use actix_web::web;
use crate::ping_logs::service::{get_watch, logs, script_logs, set_watch};

// this function could be located in a different module
pub fn log_api(cfg: &mut web::ServiceConfig) {
  cfg.service(
    web::scope("/logs")
      .service(script_logs)
      .service(logs)
      .service(get_watch)
      .service(logs)
      .service(set_watch),
  );
}
