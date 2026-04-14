use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdmissibilityResult {
    Applies,
    Inapplicable,
    Underdetermined,
    ConflictingBindings,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdmissibilityRecord {
    pub record_id: String,
    pub agent_id: String,
    pub target_ref: String,
    pub result: AdmissibilityResult,
    pub reasons: Vec<String>,
    pub missing_requirements: Vec<String>,
    pub binding_candidates: Vec<String>,
    pub emitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentCallStatus {
    Success,
    NoOp,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentCallRecord {
    pub call_id: String,
    pub agent_id: String,
    pub state_ref: String,
    pub selected_targets: Vec<String>,
    pub admissibility_result: AdmissibilityResult,
    pub inputs_used: Vec<String>,
    pub outputs_produced: Vec<String>,
    pub proof_fragment: String,
    pub status: AgentCallStatus,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    Applied,
    Partial,
    Rejected,
    NoOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MergeRecord {
    pub merge_id: String,
    pub pre_state_ref: String,
    pub post_state_ref: String,
    pub call_ref: String,
    pub added_facts: Vec<String>,
    pub added_contradictions: Vec<String>,
    pub rejected_outputs: Vec<String>,
    pub merge_status: MergeStatus,
}
