use crate::token::Token;
use actix_web::http::StatusCode;
use actix_web::http::header::ContentType;
use actix_web::rt::time::sleep;
use actix_web::web::Query;
use actix_web::{
  App, HttpRequest, HttpResponse, HttpServer, Responder, error, get, mime, post, rt, web,
};
use actix_ws::AggregatedMessage;
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use thiserror::Error;
use crate::trees::journeys::{AuthenticationTreeList, ReactFlowEdge, ReactFlowNode};

mod token;
mod trees;

#[derive(Error, Debug)]
enum ShowMeErrors {
  #[error("invalid configuration")]
  Config(#[from] VarError),
  #[error("ping api error")]
  PingApiError(#[from] reqwest::Error),
  #[error("parsing error, check the struct")]
  ParsingUiPath,
  #[error("parsing error, check the struct")]
  Parsing(#[from] serde_json::Error),
  #[error("failed to lock shared state [{0}].")]
  SharedLocking(String),
  #[error("There are no logs for id: [{0}].")]
  NoLogsFound(String),
  #[error("Creation of the api token failed: [{0}].")]
  TokenDefault(String),
  #[error("Failed to create openssl rand but")]
  TokenOpenSsl(#[from] openssl::error::ErrorStack),
  #[error("Failed to create openssl rand but")]
  TokenReadKey(#[from] std::io::Error),
  #[error("Failed to create and encode the token")]
  TokenCreateToken(#[from] jsonwebkey::Error),
  #[error("Failed to create and encode the token")]
  TokenCreateKey(#[from] jsonwebtoken::errors::Error),
  #[error("Actix Web Error")]
  ActixWs(#[from] actix_web::Error),
}

impl error::ResponseError for ShowMeErrors {
  fn status_code(&self) -> StatusCode {
    match *self {
      ShowMeErrors::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
      ShowMeErrors::PingApiError(_) => StatusCode::BAD_GATEWAY,
      ShowMeErrors::ParsingUiPath => StatusCode::BAD_REQUEST,
      ShowMeErrors::Parsing(_) => StatusCode::INTERNAL_SERVER_ERROR,
      ShowMeErrors::SharedLocking(_) => StatusCode::INTERNAL_SERVER_ERROR,
      ShowMeErrors::NoLogsFound(_) => StatusCode::NOT_FOUND,
      ShowMeErrors::TokenDefault(_) => StatusCode::INTERNAL_SERVER_ERROR,
      _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

struct AppMutState {
  transaction_id: Mutex<String>,
  authentication_tree: AuthenticationTreeList,
  token: Token,
  sec: String,
  key: String,
  log: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum Level {
  Debug,
  Warning,
  Warn,
  Info,
  Error,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PingPayload {
  context: Option<String>,
  level: Level,
  logger: Option<String>,
  message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ResultingLog {
  payload: PingPayload,
  timestamp: DateTime<Utc>,
  #[serde(rename = "type")]
  data_type: String,
  source: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Logs {
  result: Vec<ResultingLog>,
  paged_result_cooke: Option<String>,
  total_paged_results_policy: String,
  total_paged_results: i16,
  remaining_paged_results: i16,
}

impl Logs {
  pub fn filter_logs(self, level: Level) -> Logs {
    let result = self
      .result
      .iter()
      .filter(|t| t.payload.level == level)
      .cloned()
      .collect::<Vec<_>>()
      .clone();
    Logs { result, ..self }
  }
}

async fn get_logs(
  client: &Client,
  transaction_id: &str,
  query_filter: Option<&str>,
) -> Result<Logs, ShowMeErrors> {
  let params = [
    ("source", "am-everything"),
    ("transactionId", transaction_id),
    (
      "_queryFilter",
      match query_filter {
        Some(filter_string) => filter_string,
        None => "",
      },
    ),
  ];

  let url = std::env::var("SANDBOX")?;
  let key = std::env::var("PING_KEY")?;
  let sec = std::env::var("PING_SEC")?;
  match client
    .get(url)
    .query(&params)
    .header("x-api-key", key)
    .header("x-api-secret", sec)
    .send()
    .await
  {
    Ok(res) => match res.bytes().await {
      Ok(bty) => Ok(serde_json::from_slice(&bty)?),
      Err(e) => {
        Err(ShowMeErrors::PingApiError(/* reqwest::Error */ e))
      }
    },
    Err(e) => {
      Err(ShowMeErrors::PingApiError(/* reqwest::Error */ e))
    }
  }
}

#[derive(Debug, Deserialize, Clone)]
enum Filters {
  All,
  Warn,
  Error,
  Debug,
  Default,
}

#[derive(Debug, Deserialize, Clone)]
struct LogsRequest {
  filters: Option<Filters>,
}

#[get("/logs/{fr_id}")]
async fn logs(
  fr_id: web::Path<String>,
  query: Query<LogsRequest>,
) -> Result<web::Json<Logs>, ShowMeErrors> {
  let id = fr_id.into_inner();
  match get_logs(&Client::new(), &id, None).await {
    Ok(ll) => Ok(match query.filters.clone() {
      None => web::Json(ll),
      Some(filter) => match filter {
        Filters::Warn => web::Json(ll.filter_logs(Level::Warning)),
        Filters::Error => web::Json(ll.filter_logs(Level::Error)),
        Filters::Debug => web::Json(ll.filter_logs(Level::Debug)),
        _ => web::Json(ll),
      },
    }),
    Err(err) => {
      println!("{}", err);
      Err(ShowMeErrors::NoLogsFound(id))
    }
  }
}

#[derive(Deserialize)]
struct ScriptLogs {
  fr_id: String,
  script_id: String,
}

#[get("/logs/{fr_id}/script/{script_id}")]
async fn script_logs(
  path: web::Path<ScriptLogs>,
  query: Query<LogsRequest>,
) -> Result<web::Json<Logs>, ShowMeErrors> {
  let formatted_query = format!(
    "/payload/logger sw \"scripts.AUTHENTICATION_TREE_DECISION_NODE.{}\"",
    path.script_id
  );

  let query_filter = Some(formatted_query.as_str());

  println!("{:?} {:?}", formatted_query, path.fr_id);
  match get_logs(&Client::new(), &path.fr_id, query_filter).await {
    Ok(ll) => Ok(match query.filters.clone() {
      None => web::Json(ll),
      Some(filter) => match filter {
        Filters::Warn => web::Json(ll.filter_logs(Level::Warning)),
        Filters::Error => web::Json(ll.filter_logs(Level::Error)),
        Filters::Debug => web::Json(ll.filter_logs(Level::Debug)),
        _ => web::Json(ll),
      },
    }),
    Err(err) => {
      println!("{}", err);
      Err(ShowMeErrors::NoLogsFound(path.fr_id.clone()))
    }
  }
}

#[derive(Debug, Deserialize, Clone)]
struct WatchFr {
  fr_id: String,
}

#[derive(Debug, Deserialize, Clone)]
struct JourneyFilter {
  starts_with: Option<String>,
  ends_with: Option<String>,
  contains: Option<String>,
}

#[get("/journey")]
async fn get_journey(
  data: web::Data<AppMutState>,
  query: Query<JourneyFilter>,
) -> Result<web::Json<Vec<String>>, ShowMeErrors> {
  let sw = query.starts_with.clone().unwrap_or_default();
  let ew = query.ends_with.clone().unwrap_or_default();
  let cont = query.contains.clone().unwrap_or_default();

  let tree_list = data
    .authentication_tree
    .get_tree_list()
    .iter()
    .filter(|t| {
      if query.contains.is_some() {
        t.contains(&cont)
      } else {
        true
      }
    })
    .filter(|t| {
      if query.ends_with.is_some() {
        t.ends_with(&ew)
      } else {
        true
      }
    })
    .filter(|t| {
      if query.starts_with.is_some() {
        t.starts_with(&sw)
      } else {
        true
      }
    })
    .map(|t| t.clone())
    .collect();

  Ok(web::Json(tree_list))
}

#[derive(Serialize)]
struct FlowPayload {
  nodes: Vec<ReactFlowNode>,
  edges: Vec<ReactFlowEdge>,
}

#[get("/journey/{name}/flow")]
async fn journey_flow(
  name: web::Path<String>,
  data: web::Data<AppMutState>,
) -> Result<web::Json<FlowPayload>, ShowMeErrors> {
  let tree = data.authentication_tree.get_tree(&name.into_inner());

  match tree {
    None => Err(ShowMeErrors::NoLogsFound(
      "ToDo: Make a real error".to_string(),
    )),
    Some(tree_jouney) => Ok(web::Json(FlowPayload {
      nodes: tree_jouney.generate_nodes(),
      edges: tree_jouney.generate_edges(),
    })),
  }
}

#[get("/journey/{name}/scripts")]
async fn journey_script(
  name: web::Path<String>,
  data: web::Data<AppMutState>,
) -> Result<web::Json<FlowPayload>, ShowMeErrors> {
  let client = Client::new();

  let url = format!(
    "{}/am/json/realms/root/realms/alpha/realm-config/authentication/authenticationtrees/nodes/ScriptedDecisionDone/{}",
    &data.token.dom, name
  );

  let token = format!("Bearer {}",  &data.token.token_string.lock().await.deref());

  let scripts  = &client.get(url).header("authorization", token).send().await?.bytes().await?;

  Ok(web::Json(FlowPayload {
    nodes: vec![],
    edges: vec![],
  }))
}

#[get("/logs/watch")]
async fn get_watch(
  data: web::Data<AppMutState>,
  query: Query<LogsRequest>,
) -> Result<web::Json<Logs>, ShowMeErrors> {
  Ok(match data.transaction_id.lock() {
    Ok(id) => match get_logs(&Client::new(), &*id, None).await {
      Ok(ll) => match query.filters.clone() {
        None => web::Json(ll),
        Some(filter) => match filter {
          Filters::Warn => web::Json(ll.filter_logs(Level::Warning)),
          Filters::Error => web::Json(ll.filter_logs(Level::Error)),
          Filters::Debug => web::Json(ll.filter_logs(Level::Debug)),
          _ => web::Json(ll),
        },
      },
      Err(err) => {
        println!("{}", err);
        web::Json(Logs::default())
      }
    },
    _ => web::Json(Logs::default()),
  })
}

#[post("/logs/watch")]
async fn set_watch(
  data: web::Data<AppMutState>,
  payload: web::Json<WatchFr>,
) -> Result<impl Responder, ShowMeErrors> {
  let mut id = data
    .transaction_id
    .lock()
    .map_err(|_| ShowMeErrors::SharedLocking("transaction_id".into()))?;
  *id = payload.fr_id.clone();

  drop(id);

  Ok("success")
}

// this could be done with rust embed
async fn index(req: HttpRequest) -> Result<HttpResponse, ShowMeErrors> {
  let path: PathBuf = req
    .match_info()
    .query("filename")
    .parse()
    .map_err(|_| ShowMeErrors::ParsingUiPath)?;
  Ok(match path.to_str() {
    Some(pp) => {
      println!("{}", pp);
      if pp.eq("") || pp.eq("/") {
        HttpResponse::Ok()
          .content_type(ContentType::html())
          .body(include_str!(concat!("..", "/", "ui/dist/index.html")))
      } else if pp.ends_with("js") {
        HttpResponse::Ok()
          .content_type(mime::APPLICATION_JAVASCRIPT)
          .body(include_str!(concat!("..", "/", env!("JS"))))
      } else if pp.ends_with("css") {
        HttpResponse::Ok()
          .content_type("text/css")
          .body(include_str!(concat!("..", "/", env!("CSS"))))
      } else if pp.ends_with("vite.svg") {
        HttpResponse::Ok()
          .content_type(ContentType::octet_stream())
          .body(include_str!(concat!("..", "/", "ui/dist/vite.svg")))
      } else {
        HttpResponse::NotFound().body("")
      }
    }
    _ => HttpResponse::NotFound().body(""),
  })
}
async fn echo(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, ShowMeErrors> {
  let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

  let mut s2 = session.clone();
  let mut stream = stream
    .aggregate_continuations()
    // aggregate continuation frames up to 1MiB
    .max_continuation_size(2_usize.pow(20));
  rt::spawn(async move {
    loop {
      s2.text(format!("booo    {}", chrono::Utc::now().timestamp())).await.unwrap();
      sleep(Duration::from_secs(2)).await
    }
  });

  // start task but don't wait for it
  rt::spawn(async move {
    // receive messages from websocket
    while let Some(msg) = stream.next().await {
      match msg {
        Ok(AggregatedMessage::Text(text)) => {
          // echo text message
          session.text(text).await.unwrap();
        }

        Ok(AggregatedMessage::Binary(bin)) => {
          // echo binary message
          session.binary(bin).await.unwrap();
        }

        Ok(AggregatedMessage::Ping(msg)) => {
          // respond to PING frame with PONG frame
          session.pong(&msg).await.unwrap();
        }

        _ => {}
      }
    }
  });

  // respond immediately with response connected to WS session
  Ok(res)
}

#[get("/idm")]
async fn idm(data: web::Data<AppMutState>) -> Result<String, ShowMeErrors> {
  let client = Client::new();
  let metrics = &*client
    .get(format!("{}/monitoring/prometheus/idm", &data.token.dom,))
    .header("x-api-key", &data.key)
    .header("x-api-secret", &data.sec)
    .send()
    .await?
    .text()
    .await?;

  Ok(metrics.to_string())
}

#[get("/am")]
async fn am(data: web::Data<AppMutState>) -> Result<String, ShowMeErrors> {
  let client = Client::new();
  let metrics = client
    .get(format!("{}/monitoring/prometheus/am", &data.token.dom,))
    .header("x-api-key", &data.key)
    .header("x-api-secret", &data.sec)
    .send()
    .await?
    .text()
    .await?;

  Ok(metrics.to_string())
}

#[actix_web::main]
async fn main() -> Result<(), ShowMeErrors> {
  let mut token = Token::new().await?;

  let client = Client::new();

  // ToDo - If this was a Arc<Mutex> it would not need the be &mut
  let token_str = token.get_usable_token().await;

  let authentication_tree: AuthenticationTreeList = serde_json::from_slice(&client.get(format!("{}/am/json/realms/root/realms/alpha/realm-config/authentication/authenticationtrees/trees?_queryFilter=true", token.dom, )).header("authorization", format!("Bearer {}", token_str)).send().await?.bytes().await?)?;

  let url = std::env::var("SANDBOX")?;
  let key = std::env::var("PING_KEY")?;
  let sec = std::env::var("PING_SEC")?;
  let state = web::Data::new(AppMutState {
    transaction_id: Mutex::new(String::new()),
    authentication_tree,
    token,
    sec,
    key,
    log: url,
  });

  HttpServer::new(move || {
    let cors = actix_cors::Cors::permissive().allow_any_header();
    App::new()
      .app_data(state.clone())
      .wrap(cors)
      .service(
        web::scope("/api")
          .service(get_watch)
          .service(logs)
          .service(script_logs)
          .service(set_watch)
          .service(journey_flow)
          .service(get_journey)
          .route("/echo", web::get().to(echo))
          .service(web::scope("/monitoring").service(am).service(idm)),
      )
      .route("/{filename:.*}", web::get().to(index))
  })
    .bind(("0.0.0.0", 8081))?
    .run()
    .await?;
  Ok(())
}
