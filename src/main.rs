use crate::errors::ShowMeErrors;
use crate::token::{Token, get_usable_token};
use crate::trees::journeys::{AuthenticationTreeList, ReactFlowEdge, ReactFlowNode, Tree};
use crate::trees::nodes::{NodeConfig, NodeData};
use actix_web::http::header::ContentType;
use actix_web::rt::time::sleep;
use actix_web::web::Query;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, mime, post, rt, web};
use actix_ws::{AggregatedMessage, Session};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt as _, TryFutureExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, LockResult, Mutex, mpsc};
use std::time::Duration;

mod errors;
mod token;
mod trees;

struct AppMutState {
  transaction_id: Mutex<String>,
  authentication_tree: AuthenticationTreeList,
  token: Token,
  // ToDo read the doc to see if this is silly to have a
  token_str: Mutex<String>,
  payload: Mutex<token::Payload>,
  sec: String,
  key: String,
  log: String,
  ws: Mutex<Vec<Session>>,
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
struct NodeOutcomeInfo {
  node_extra_logging: Option<serde_json::Map<String, serde_json::Value>>,
  node_id: String,
  node_outcome: String,
  display_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct NodeOutcome {
  info: NodeOutcomeInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PingPayload {
  context: Option<String>,
  level: Level,
  entries: Option<Vec<NodeOutcome>>,
  logger: Option<String>,
  message: Option<String>,
  transaction_id: String,
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
    ("source", "am-everything,idm-everything"),
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
  script_id: Option<String>,
  node_id: Option<String>,
}

#[get("/logs/{fr_id}")]
async fn logs(
  fr_id: web::Path<String>,
  query: Query<LogsRequest>,
) -> Result<web::Json<Logs>, ShowMeErrors> {
  let id = fr_id.into_inner();

  let script_filter = query.script_id.clone().map(|script_id| {
    format!(
      "/payload/logger sw \"scripts.AUTHENTICATION_TREE_DECISION_NODE.{}\"",
      script_id
    )
  });

  let node_filter = query
    .node_id
    .clone()
    .map(|node_id| format!("/payload/entries/info/nodeId eq \"{}\"", node_id));

  let defined_filters: Vec<String> = vec![node_filter, script_filter]
    .iter()
    .filter(|maybe_filter| maybe_filter.is_some())
    .map(|filter| filter.as_ref().cloned().unwrap())
    .collect();

  let query_filter = defined_filters.clone().join(" or ");

  match get_logs(&Client::new(), &id, Some(query_filter.as_str())).await {
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

#[derive(Debug)]
struct NodeOutcomeEdge {
  name: String,
  outcome: String,
}

async fn get_node_outcomes(transaction_id: &str) -> Result<Vec<NodeOutcomeEdge>, ShowMeErrors> {
  match get_logs(
    &Client::new(),
    &transaction_id,
    Some("/payload/entries/info/nodeOutcome pr"),
  )
  .await
  {
    Ok(ll) => Ok(
      ll.result
        .iter()
        .map(|log| {
          let payload = log.payload.entries.clone().unwrap_or(vec![])[0]
            .info
            .clone();

          let outcome = payload.node_outcome;
          let name = payload.display_name;

          NodeOutcomeEdge { name, outcome }
        })
        .collect(),
    ),
    Err(err) => {
      println!("{}", err);
      Err(ShowMeErrors::NoLogsFound(transaction_id.to_string()))
    }
  }
}

#[derive(Deserialize)]
struct JourneyFlowQuery {
  transaction_id: Option<String>,
}

#[get("/journey/{name}/flow")]
async fn journey_flow(
  name: web::Path<String>,
  query: Query<JourneyFlowQuery>,
  data: web::Data<AppMutState>,
) -> Result<web::Json<FlowPayload>, ShowMeErrors> {
  let transaction_id = &query.transaction_id;
  let tree = data.authentication_tree.get_tree(&name);

  let node_outcomes = (match transaction_id {
    Some(id) => get_node_outcomes(id).await,
    None => Ok(vec![]),
  })
  .unwrap_or(vec![]);

  match tree {
    None => Err(ShowMeErrors::NoLogsFound(
      "ToDo: Make a real error".to_string(),
    )),
    Some(tree_jouney) => Ok(web::Json(FlowPayload {
      nodes: tree_jouney.generate_nodes(),
      edges: tree_jouney.generate_edges(&node_outcomes),
    })),
  }
}

#[derive(Serialize)]
struct JourneyTransaction {
  transaction_id: String,
  timestamp: DateTime<Utc>,
}

#[get("/journey/{name}/transactions")]
async fn get_journey_transactions(
  journey_name: web::Path<String>,
) -> Result<web::Json<Vec<JourneyTransaction>>, ShowMeErrors> {
  let journey_transactions = get_logs(
    &Client::new(),
    "", // Blank transaction ID effectively runs a * search.
    Some(
      format!(
        "/payload/entries/info/treeName eq \"{}\" and /payload/entries/info/nodeOutcome pr and /payload/eventName eq \"AM-NODE-LOGIN-COMPLETED\"",
        journey_name
      )
      .as_str(),
    ),
  )
  .await;

  match journey_transactions {
    Ok(transactions) => Ok(web::Json(
      transactions
        .result
        .iter()
        .map(|log| JourneyTransaction {
          timestamp: log.timestamp,
          transaction_id: log.payload.transaction_id.clone(),
        })
        .collect(),
    )),
    Err(err) => Err(err),
  }
}

#[get("/journey/{name}/scripts")]
async fn journey_script(
  name: web::Path<String>,
  data: web::Data<AppMutState>,
) -> Result<web::Json<HashMap<String, (NodeConfig, NodeData)>>, ShowMeErrors> {
  let (token_str, payload) = get_usable_token(&data.token, &data.payload, &data.token_str).await?;

  let dom = &data.token.dom;
  let tree = match data.authentication_tree.get_tree(&name.into_inner()) {
    None => HashMap::new(),
    Some(data) => data.get_node_info(dom, &token_str).await?,
  };

  Ok(web::Json(tree))
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
async fn echo(
  req: HttpRequest,
  stream: web::Payload,
  data: web::Data<AppMutState>,
) -> Result<HttpResponse, ShowMeErrors> {
  let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

  match data.ws.lock() {
    Ok(mut ws) => ws.push(session.clone()),
    Err(_) => {
      println!("Failed to lock ws")
    }
  };

  let mut s2 = session.clone();
  let mut stream = stream
    .aggregate_continuations()
    // aggregate continuation frames up to 1MiB
    .max_continuation_size(2_usize.pow(20));

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
  let (token, payload_init) = Token::new().await?;
  let payload_mux_init = Mutex::new(payload_init);

  let client = Client::new();
  let token_mux = Mutex::new("".to_string());

  // ToDo - If this was a Arc<Mutex> it would not need the be &mut
  let (token_str, payload_up) = get_usable_token(&token, &payload_mux_init, &token_mux).await?;

  let authentication_tree: AuthenticationTreeList = serde_json::from_slice(&client.get(format!("{}/am/json/realms/root/realms/alpha/realm-config/authentication/authenticationtrees/trees?_queryFilter=true", token.dom, )).header("authorization", format!("Bearer {}", token_str)).send().await?.bytes().await?)?;

  let url = std::env::var("SANDBOX")?;
  let key = std::env::var("PING_KEY")?;
  let sec = std::env::var("PING_SEC")?;
  let state = web::Data::new(AppMutState {
    transaction_id: Mutex::new(String::new()),
    authentication_tree,
    token,
    token_str: token_mux,
    payload: Mutex::new(payload_up),
    sec,
    key,
    log: url,
    ws: Mutex::new(vec![]),
  });

  let watcher_state = state.clone();

  rt::spawn(async move {
    loop {
      while watcher_state.ws.lock().unwrap().is_empty() {
        println!("ws is still none");
        dbg!(watcher_state.ws.lock().unwrap().is_empty());
        sleep(Duration::from_secs(4)).await;
      }
      let s2 = (*watcher_state.ws.lock().unwrap()).clone();
      loop {
        match s2
          .clone()
          .iter()
          .cloned()
          .enumerate()
          .collect::<Vec<(usize, Session)>>()
          .first()
        {
          Some((i, x)) => x
            .clone()
            .text(format!("booo {i}   {}", Utc::now().timestamp()))
            .await
            .map_err(ShowMeErrors::ActixWsClosed)?,
          None => println!("session vec is empty."),
        }
        sleep(Duration::from_secs(2)).await;
      }
    }

    Ok::<(), ShowMeErrors>(())
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
          .service(set_watch)
          .service(journey_flow)
          .service(journey_script)
          .service(get_journey)
          .service(get_journey_transactions)
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
