use actix_web::http::StatusCode;
use actix_web::http::header::ContentType;
use actix_web::web::Query;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, error, get, mime, post, web,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env::VarError;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use thiserror::Error;

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
        }
    }
}

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

async fn get_logs(client: &Client, transaction_id: &str) -> Result<Logs, ShowMeErrors> {
    let params = [
        ("source", "am-everything"),
        ("transactionId", transaction_id),
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
    match get_logs(&Client::new(), &id).await {
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

#[derive(Debug, Deserialize, Clone)]
struct WatchFr {
    fr_id: String,
}

#[get("/journey/{name}")]
async fn get_journey(
    name: web::Path<String>,
    data: web::Data<AppMutState>,
) -> Result<web::Json<Value>, ShowMeErrors> {
    let token = data
        .token
        .lock()
        .map_err(|_| ShowMeErrors::SharedLocking("Failed to lock token".into()))?
        .clone();

    let client = Client::new();
    let _ = client.get(format!("https://{}/am/json/realms/roon/realms/alpha/realm-config/authentication/authenticationtrees/trees/{}", std::env::var("PING_DOMAIN")?, name.into_inner())).header("authorization", format!("Bearer {}", token));

    Ok(web::Json(Value::from_str("5")?))
}

#[get("/logs/watch")]
async fn get_watch(
    data: web::Data<AppMutState>,
    query: Query<LogsRequest>,
) -> Result<web::Json<Logs>, ShowMeErrors> {
    Ok(match data.transaction_id.lock() {
        Ok(id) => match get_logs(&Client::new(), &*id).await {
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
