use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::case::{BudgetContract, SemanticsProfile};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FinalLabel {
    Entailed,
    Refuted,
    Undetermined,
    InconsistentBase,
    ContractInvalid,
    ResourceBounded,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminalFailureLocus {
    ContractValidation,
    MeasurementMalformedStructure,
    AdmissibilityNoApplicableAgent,
    MissingPremise,
    ContradictionBlock,
    BudgetExhaustion,
    KernelUnresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BudgetSnapshot {
    pub max_steps: u32,
    pub max_agent_calls: u32,
    pub max_iterations: u32,
}

impl From<&BudgetContract> for BudgetSnapshot {
    fn from(value: &BudgetContract) -> Self {
        Self {
            max_steps: value.max_steps,
            max_agent_calls: value.max_agent_calls,
            max_iterations: value.max_iterations,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KernelDecision {
    pub decision_id: String,
    pub state_ref: String,
    pub semantics_profile: SemanticsProfile,
    pub final_label: FinalLabel,
    pub rationale_summary: String,
    pub decisive_refs: Vec<String>,
    pub blocking_refs: Vec<String>,
    pub emitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalPacket {
    pub schema_version: String,
    pub case_id: String,
    pub final_label: FinalLabel,
    pub rationale_summary: String,
    pub terminal_failure_locus: Option<TerminalFailureLocus>,
    pub decisive_refs: Vec<String>,
    pub blocking_refs: Vec<String>,
    pub semantics_profile: SemanticsProfile,
    pub budget_snapshot: BudgetSnapshot,
    pub replay_bundle_ref: String,
    pub terminal_state_ref: String,
    pub emitted_at: String,
}
