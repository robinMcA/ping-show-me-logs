use crate::errors::ShowMeErrors;
use crate::ping_logs::logs::{get_logs, ResultingLog};
use crate::ping_logs::service::log_api;
use crate::token::{get_usable_token, Token};
use crate::trees::journeys::{AuthenticationTreeList, ReactFlowEdge, ReactFlowNode};
use crate::trees::nodes::{NodeConfig, NodeData};
use actix_web::http::header::ContentType;
use actix_web::rt::time::sleep;
use actix_web::web::Query;
use actix_web::{get, mime, rt, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_ws::AggregatedMessage;
use chrono::{DateTime, Utc};
use futures::future;
use futures_util::StreamExt as _;
use futures_util::TryFutureExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

mod errors;
mod token;
mod trees;
mod ping_logs;

struct AppMutState {
  transaction_id: Mutex<String>,
  authentication_tree: AuthenticationTreeList,
  token: Token,
  token_str: Mutex<String>,
  payload: Mutex<token::Payload>,
  sec: String,
  key: String,
  log: String,
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
  let client = &Client::new();

  // Get latest node outcomes with tracking IDs
  let most_recent_transaction_logs = get_logs(
    client,
    &transaction_id,
    Some("/payload/entries/info/nodeOutcome pr"),
  )
  .await
  .map_or(vec![], |some_logs| some_logs.result);

  let mut tracking_ids: Vec<String> = most_recent_transaction_logs
    .clone()
    .iter()
    .flat_map(|log| log.payload.tracking_ids.clone())
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  tracking_ids.sort();

  let async_logs = future::join_all(if !tracking_ids.is_empty() {
    tracking_ids
      .into_iter() // takes ownership of each String
      .map(|tracking_id| async move {
        println!(
          "Getting logs for additional tracking ID [{:?}]...",
          tracking_id
        );

        let some_logs = get_logs(
          client,
          "",
          Some(&format!(
            "/payload/trackingIds eq \"{}\" and /payload/entries/info/nodeOutcome pr",
            tracking_id
          )),
        )
        .await;

        let final_logs = some_logs.map_or(vec![], |some_logs| some_logs.result);

        println!(
          "Got  [{:?}] logs for tracking ID [{:?}].",
          final_logs.len(),
          tracking_id,
        );

        final_logs
      })
      .collect()
  } else {
    vec![]
  })
  .await;

  // Perform query for all other node outcomes in the journey with the tracking ID
  let all_transaction_logs: Vec<&ResultingLog> =
    async_logs.iter().flat_map(|results| results).collect();

  Ok(
    all_transaction_logs
      .iter()
      .map(|log| {
        let thing = &log.payload.entries.clone().unwrap_or(vec![])[0];

        NodeOutcomeEdge {
          name: thing.info.display_name.clone(),
          outcome: thing.info.node_outcome.clone(),
        }
      })
      .collect(),
  )
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
      s2.text(format!("booo    {}", chrono::Utc::now().timestamp()))
        .await
        .unwrap();
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
  });

  HttpServer::new(move || {
    let cors = actix_cors::Cors::permissive().allow_any_header();
    App::new()
      .app_data(state.clone())
      .wrap(cors)
      .service(
        web::scope("/api")
          .configure(log_api)
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
