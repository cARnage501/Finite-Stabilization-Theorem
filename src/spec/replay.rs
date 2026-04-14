use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskMode {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffConstraints {
    pub cost_bound: u32,
    pub time_bound_ms: u64,
    pub risk_mode: RiskMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffPermissions {
    pub may_read: Vec<String>,
    pub may_write: Vec<String>,
    pub may_adjudicate_semantics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffEnvelope {
    pub envelope_id: String,
    pub iteration_id: String,
    pub work_item_id: String,
    pub stage: String,
    pub intent: String,
    pub success_criteria: Vec<String>,
    pub constraints: HandoffConstraints,
    pub artifact_refs: Vec<String>,
    pub permissions: HandoffPermissions,
    pub idempotency_key: String,
    pub return_channel: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplayBundle {
    pub bundle_id: String,
    pub case_id: String,
    pub ordered_state_refs: Vec<String>,
    pub ordered_agent_calls: Vec<String>,
    pub ordered_gate_records: Vec<String>,
    pub kernel_decision_ref: Option<String>,
    pub checksum: String,
}
