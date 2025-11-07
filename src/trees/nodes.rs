use crate::errors::ShowMeErrors;
use crate::trees::journeys::NodeType;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;

#[derive(Deserialize, Serialize, Clone)]
struct NodeConfigType {
  #[serde(rename = "_id")]
  id: String,
  name: String,
  collection: bool,
  version: String,
}

#[derive(Deserialize, Serialize, Clone)]
struct ScriptConfig {
  #[serde(rename = "_id")]
  id: String,
  #[serde(rename = "_rev")]
  rev: String,
  script: String,
  outcomes: Vec<String>,
  outputs: Vec<String>,
  inputs: Vec<String>,
  #[serde(rename = "_type")]
  node_type: NodeConfigType,
  #[serde(rename = "_outcomes")]
  outcome_map: Vec<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum NodeConfig {
  ScriptConfig(ScriptConfig),
  None,
}

#[derive(Deserialize, Serialize, Clone)]
struct Script {
  #[serde(rename = "_id")]
  id: String,
  name: String,
  description: String,
  script: String
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum NodeData {
  Scirpt(Script),
  None,
}

pub async fn node_id_to_script_config(
  node_type: &NodeType,
  node_id: &str,
  dom: &str,
  token_str: &str,
) -> Result<(NodeConfig, NodeData), ShowMeErrors> {
  let client = Client::new();

  let config_txt = &client.get(format!("{dom}/am/json/realms/root/realms/alpha/realm-config/authentication/authenticationtrees/nodes/{node_type}/{node_id}")).header("authorization", format!("Bearer {}", token_str)).send().await?.bytes()
    .await.map_err(|e| {
    dbg!(e);
   ShowMeErrors::NoLogsFound("".to_string())
  })?;

  let script_config: ScriptConfig = serde_json::from_slice(config_txt)?;

  let script_id = &script_config.script;

  let script_txt = &client
    .get(format!("{dom}/am/json/alpha/scripts/{script_id}"))
    .header("authorization", format!("Bearer {}", token_str))
    .send()
    .await?
    .bytes()
    .await?;


  let script_data: Script = serde_json::from_slice(script_txt)?;


  Ok((
    NodeConfig::ScriptConfig(script_config),
    NodeData::Scirpt(script_data),
  ))
}
