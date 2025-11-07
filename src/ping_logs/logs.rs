use reqwest::Client;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::errors::ShowMeErrors;


#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
  Debug,
  Warning,
  Warn,
  Info,
  Error,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeOutcomeInfo {
  node_extra_logging: Option<serde_json::Map<String, serde_json::Value>>,
  node_id: String,
  pub(crate) node_outcome: String,
  pub(crate) display_name: String,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeOutcome {
  pub(crate) info: NodeOutcomeInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PingPayload {
  context: Option<String>,
  level: Level,
  pub(crate) entries: Option<Vec<NodeOutcome>>,
  logger: Option<String>,
  message: Option<String>,
  pub(crate) transaction_id: String,
  pub(crate) tracking_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResultingLog {
  pub(crate) payload: PingPayload,
  pub(crate) timestamp: DateTime<Utc>,
  #[serde(rename = "type")]
  data_type: String,
  source: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Logs {
  pub(crate) result: Vec<ResultingLog>,
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

pub(crate) async fn get_logs(
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
