use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Readout {
    pub readout_id: String,
    pub name: String,
    pub input_refs: Vec<String>,
    pub computation_summary: String,
    pub numeric_score: f32,
    pub threshold_exceeded: bool,
    pub trace_fragment: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AggregateFlags {
    pub structurally_incomplete: bool,
    pub structurally_contested: bool,
    pub structurally_malformed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LatticeReadoutPacket {
    pub packet_id: String,
    pub state_ref: String,
    pub coverage_gap: Readout,
    pub collision_pressure: Readout,
    pub trigger_illegality: Readout,
    pub role_confusion: Readout,
    pub aggregate_flags: AggregateFlags,
    pub emitted_at: String,
}
