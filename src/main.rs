use crate::errors::ShowMeErrors;
use crate::ping_logs::service::log_api;
use crate::token::{Token, get_usable_token};
use crate::trees::journeys::AuthenticationTreeList;
use crate::trees::service::trees_api;
use crate::workers::scripts::{RichScript, ScriptConfig,  list_scripts, get_rich_script};
use actix_web::http::header::ContentType;
use actix_web::rt::time::sleep;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, mime, rt, web};
use std::sync::mpsc::{Sender};
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

mod errors;
mod ping_logs;
mod token;
mod trees;
mod workers;

struct AppMutState {
  transaction_id: Mutex<String>,
  authentication_tree: AuthenticationTreeList,
  token: Token,
  token_str: Mutex<String>,
  payload: Mutex<token::Payload>,
  sec: String,
  key: String,
  log: String,
  script_config: Mutex<HashMap<String, ScriptConfig>>,
  txs: Arc<Mutex<Vec<Sender<String>>>>,
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

#[derive(Debug)]
struct NodeOutcomeEdge {
  name: String,
  outcome: String,
}

async fn echo(req: HttpRequest, stream: web::Payload, data: web::Data<AppMutState>) -> Result<HttpResponse, ShowMeErrors> {
  let (res, mut session, _) = actix_ws::handle(&req, stream)?;

  rt::spawn(async move { 
    let (tx, rx) = mpsc::channel();

    {
      let mutex = data.txs.clone();
      let mut senders = mutex.lock().unwrap();
      senders.push(tx.clone());
    }

    let binding = req.connection_info();
    let host = binding.host();
    
    println!("{} connected.", host);

    // forward messages back
    loop {
      if let Ok(msg) = rx.recv() {
        let send_result = session.text(msg.to_owned()).await;
        if send_result.is_err() {
          println!("{} disconnected.", host);
          return;
        }
      }
    }
  });

  // respond immediately with response connected to WS session
  Ok(res)
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
    script_config: Mutex::new(HashMap::new()),
    txs: Arc::new(Mutex::new(Vec::new())),
  });

  let data = state.clone();
  rt::spawn(async move {
    let client = Client::new();
    loop {
      let (token_str, payload) =
        get_usable_token(&data.token, &data.payload, &data.token_str).await?;
      let scripts = list_scripts(&client, &data.token.dom, &token_str).await?;

      let mut sct = data
        .script_config
        .lock()
        .map_err(|_| ShowMeErrors::SharedLocking("script list".into()))?;

      *sct = scripts;

      // Otherwise this lock would only go out of scope when the sleep endds.
      drop(sct);

      sleep(Duration::from_secs(30)).await;
    }
    Ok::<(), ShowMeErrors>(())
  });

  // multiple sources
  let (tx, rx) = mpsc::channel();

  {
    let tx2 = tx.clone();
    thread::spawn(move || {
      loop {
        tx2.send(String::from("test")).unwrap();
        thread::sleep(Duration::from_millis(80))
      }
    });

    let tx2 = tx.clone();
    thread::spawn(move || {
      loop {
        tx2.send(String::from("src 2")).unwrap();
        thread::sleep(Duration::from_millis(160))
      }
    });
  }

  // broadcast service
  let senders_mutex = state.txs.clone();
  thread::spawn(move || {
    loop {
      while let Ok(msg) = rx.recv() {
        // TODO: do transformations here

        let mut senders = senders_mutex.lock().unwrap();
        senders.retain(|tx| tx.send(msg.to_owned()).is_ok());
      }
    }
  });

  HttpServer::new(move || {
    let cors = actix_cors::Cors::permissive().allow_any_header();
    App::new()
      .app_data(state.clone())
      .wrap(cors)
      .service(
        web::scope("/api")
          .configure(trees_api)
          .configure(log_api)
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
