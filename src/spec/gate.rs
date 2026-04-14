use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    StructuralReadiness,
    Admissibility,
    EvidenceSufficiency,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GateResult {
    Pass,
    Fail,
    Defer,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GateRecord {
    pub gate_id: String,
    pub gate_type: GateType,
    pub state_ref: String,
    pub inputs: Vec<String>,
    pub result: GateResult,
    pub rationale: String,
    pub emitted_at: String,
}
