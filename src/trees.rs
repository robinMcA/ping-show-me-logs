use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
pub enum NodeType {
    AccountLockoutNode,
    AgentDataStoreDecisionNode,
    AttributeCollectorNode,
    AttributePresentDecisionNode,
    ConfigProviderNode,
    CookiePresenceDecisionNode,
    CreateObjectNode,
    DataStoreDecisionNode,
    DebugNode,
    DeviceMatchNode,
    DeviceProfileCollectorNode,
    DeviceSaveNode,
    DisplayUserNameNode,
    EmailSuspendNode,
    IdentifyExistingUserNode,
    IdentityStoreDecisionNode,
    IncrementLoginCountNode,
    InnerTreeEvaluatorNode,
    LoginCountDecisionNode,
    MessageNode,
    OneTimePasswordGeneratorNode,
    PageNode,
    PatchObjectNode,
    PollingWaitNode,
    QueryFilterDecisionNode,
    RetryLimitDecisionNode,
    ScriptedDecisionNode,
    SelectIdPNode,
    SessionDataNode,
    SetCustomCookieNode,
    SetFailureUrlNode,
    SetStateNode,
    SetSuccessDetailsNode,
    SetSuccessUrlNode,
    SocialProviderHandlerNodeV2,
    UsernameCollectorNode,
    ZeroPageLoginNode,
    #[serde(rename = "product-PingOneProtectEvaluationNode")]
    ProductPingOneProtectEvaluationNode,
    #[serde(rename = "product-PingOneProtectInitializeNode")]
    ProductPingOneProtectInitializeNode,
    #[serde(rename = "product-PingOneProtectResultNode")]
    ProductPingOneProtectResultNode,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    connections: HashMap<String, String>,
    display_name: String,
    node_type: NodeType,
    x: Option<f32>,
    y: Option<f32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StaticNode {
    x: f32,
    y: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    #[serde(rename = "_id")]
    id: String,
    #[serde(rename = "_rev")]
    rev: String,
    identity_resource: Option<String>,
    entry_node_id: String,
    inner_tree_only: bool,
    no_session: bool,
    must_run: bool,
    enabled: bool,
    transaction_only: Option<bool>,
    ui_config: HashMap<String, String>,
    nodes: HashMap<String, Node>,
    static_nodes: HashMap<String, StaticNode>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationTreeList {
    result: Vec<Tree>,
    result_count: i32,
    paged_results_cookie: Option<String>,
    total_paged_results_policy: String,
    total_paged_results: i16,
    remaining_paged_results: i16,
}

impl AuthenticationTreeList {
    pub fn get_tree_list(&self) -> Vec<String> {
        self.result.iter().map(|t| t.id.to_owned()).collect()
    }
    pub fn get_tree(&self, name: &str) -> Option<Tree> {
        self.result.iter().find(|t| t.id.eq(name)).cloned()
    }
}

#[derive(Serialize)]
enum EdgeType {
    Normal,
    Error,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactFlowEdge {
    id: String,
    #[serde(rename = "type")]
    edge_type: EdgeType,
    source: String,
    target: String,
    source_handle: String,
}

#[derive(Serialize)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Serialize)]
pub struct ReactFlowNode {
    id: String,
    position: Position,
    data: HashMap<String, String>,
}

impl Tree {
    pub fn generate_edges(&self) -> Vec<ReactFlowEdge> {
        let start_edge = ReactFlowEdge {
            id: format!("{}/{}", "start", "ok"),
            edge_type: EdgeType::Normal,
            source: "start".to_string(),
            target: self.entry_node_id.clone(),
            source_handle: "ok".to_string(),
        };
        let mut rest = self
            .nodes
            .iter()
            .flat_map(|t| {
                t.1.connections.iter().map(|v| ReactFlowEdge {
                    id: format!("{}/{}", t.0.to_owned(), v.0.to_owned()),
                    edge_type: if v.0.starts_with("error") {
                        EdgeType::Error
                    } else {
                        EdgeType::Normal
                    },
                    source: t.0.to_owned(),
                    target: v.1.to_string(),
                    source_handle: v.0.to_string(),
                })
            })
            .collect::<Vec<ReactFlowEdge>>();

        rest.push(start_edge);
        rest
    }
    pub fn generate_nodes(&self) -> Vec<ReactFlowNode> {
        let static_nodes = self.static_nodes.iter().map(|t| ReactFlowNode {
            id: t.0.to_owned(),
            position: Position { x: t.1.x, y: t.1.y },
            data: Default::default(),
        });
        let other_nodes = self.nodes.iter().map(|t| ReactFlowNode {
            id: t.0.to_owned(),
            position: Position {
                x: t.1.x.unwrap_or(0.0),
                y: t.1.y.unwrap_or(0.0),
            },
            data: Default::default(),
        });
        static_nodes.chain(other_nodes).collect()
    }
}
