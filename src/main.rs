use actix_web::http::header::ContentType;
use actix_web::web::Query;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, mime, post, web};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{LockResult, Mutex};

#[derive(Debug)]
struct AppMutState {
    transaction_id: Mutex<String>,
    token: Mutex<String>,
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

async fn get_logs(client: Client, transaction_id: &str) -> serde_json::Result<Logs> {
    let params = [
        ("source", "am-everything"),
        ("transactionId", transaction_id),
    ];

    let url = std::env::var("SANDBOX").unwrap();
    let key = std::env::var("PING_KEY").unwrap();
    let sec = std::env::var("PING_SEC").unwrap();
    serde_json::from_str(
        &client
            .get(url)
            .query(&params)
            .header("x-api-key", key)
            .header("x-api-secret", sec)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
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
async fn logs(fr_id: web::Path<String>, query: Query<LogsRequest>) -> web::Json<Logs> {
    match get_logs(Client::new(), &fr_id.into_inner()).await {
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
    }
}

#[derive(Debug, Deserialize, Clone)]
struct WatchFr {
    fr_id: String,
}

#[get("/journey/{name}")]
async fn get_journey(name: web::Path<String>, data: web::Data<AppMutState>) -> web::Json<Value> {
    let token = data.token.lock().unwrap().clone();

    let client = Client::new();
    let _ = client.get(format!("https://{}/am/json/realms/roon/realms/alpha/realm-config/authentication/authenticationtrees/trees/{}", std::env::var("PING_DOMAIN").unwrap(), name.into_inner())).header("authorization", format!("Bearer {}", token));

    web::Json(Value::from_str("5").unwrap())
}

#[get("/logs/watch")]
async fn get_watch(data: web::Data<AppMutState>, query: Query<LogsRequest>) -> web::Json<Logs> {
    match data.transaction_id.lock() {
        Ok(id) => match get_logs(Client::new(), &*id).await {
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
    }
}

#[post("/logs/watch")]
async fn set_watch(data: web::Data<AppMutState>, payload: web::Json<WatchFr>) -> HttpResponse {
    let mut id = data.transaction_id.lock().unwrap();
    *id = payload.fr_id.clone();

    drop(id);

    HttpResponse::Ok().body("success")
}

// this could be done with rust embed
async fn index(req: HttpRequest) -> HttpResponse {
    let path: PathBuf = req.match_info().query("filename").parse().unwrap();
    match path.to_str() {
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
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = web::Data::new(AppMutState {
        transaction_id: Mutex::new(String::new()),
        token: Mutex::new(String::new()),
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
                    .service(get_journey),
            )
            .route("/{filename:.*}", web::get().to(index))
    })
    .bind(("0.0.0.0", 8081))?
    .run()
    .await
}
