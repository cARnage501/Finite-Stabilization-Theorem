use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::agent::AgentCallRecord;
use crate::spec::case::{
    BudgetContract, Polarity, PositiveAtom, PositiveRule, SemanticsProfile, SignedGroundAtom,
};
use crate::spec::errors::ValidationError;
use crate::spec::gate::GateRecord;
use crate::spec::readout::LatticeReadoutPacket;
use crate::spec::terminal::{FinalLabel, TerminalFailureLocus};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    Input,
    ExternalIntervention,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FactInstance {
    pub atom: SignedGroundAtom,
    pub source: FactSource,
    pub support_refs: Vec<String>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DerivedPositiveFact {
    pub atom: PositiveAtom,
    pub produced_by_agent: String,
    pub input_refs: Vec<String>,
    pub proof_fragment: String,
    pub derivation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionStatus {
    Active,
    Quarantined,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContradictionRecord {
    pub contradiction_id: String,
    pub positive_ref: String,
    pub negative_ref: String,
    pub scope: String,
    pub status: ContradictionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    InputFact,
    DerivedFact,
    ContradictionRecord,
    ReadoutSignal,
    AdmissibilityRecord,
    Intervention,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceEntry {
    pub evidence_id: String,
    pub kind: EvidenceKind,
    pub content_ref: String,
    pub provenance_refs: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProceduralHypothesisType {
    MalformedCase,
    MalformedStructure,
    ResolveNow,
    Contradiction,
    MissingSupport,
    DirectDerivation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProceduralHypothesis {
    pub hypothesis_id: String,
    pub hypothesis_type: ProceduralHypothesisType,
    pub triggered_by_refs: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoopPhase {
    Init,
    Measure,
    Hypothesize,
    SelectAgent,
    CheckAdmissibility,
    ApplyAgent,
    Merge,
    Appraise,
    Escalate,
    Stop,
    Resolve,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Open,
    Provisional,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeStatus {
    pub loop_phase: LoopPhase,
    pub decision_status: DecisionStatus,
    pub final_label: Option<FinalLabel>,
    pub terminal_failure_locus: Option<TerminalFailureLocus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemType {
    Measurement,
    HypothesisGeneration,
    TheoremAgentCall,
    AdmissibilityCheck,
    Escalation,
    KernelAdjudication,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectMode {
    ReadOnly,
    StateUpdateAllowed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    Created,
    Assigned,
    InProgress,
    Blocked,
    Completed,
    Failed,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkItemConstraints {
    pub max_steps: u32,
    pub side_effect_mode: SideEffectMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkItem {
    pub work_item_id: String,
    pub item_type: WorkItemType,
    pub intent: String,
    pub target_refs: Vec<String>,
    pub success_criteria: Vec<String>,
    pub constraints: WorkItemConstraints,
    pub created_at: String,
    pub status: WorkItemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProblemState {
    pub state_id: String,
    pub case_id: String,
    pub semantics_profile: SemanticsProfile,
    pub closed_world: bool,
    pub domain_constants: Vec<String>,
    pub budget: BudgetContract,
    pub known_facts: Vec<FactInstance>,
    pub active_rules: Vec<PositiveRule>,
    pub query: SignedGroundAtom,
    pub derived_facts: Vec<DerivedPositiveFact>,
    pub contradictions: Vec<ContradictionRecord>,
    pub lattice_readouts: Option<LatticeReadoutPacket>,
    pub agent_history: Vec<AgentCallRecord>,
    pub gate_history: Vec<GateRecord>,
    pub evidence_ledger: Vec<EvidenceEntry>,
    pub procedural_hypotheses: Vec<ProceduralHypothesis>,
    pub status: RuntimeStatus,
    pub provenance_root: String,
}

impl ProblemState {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        for fact in &self.known_facts {
            if fact.atom.polarity == Polarity::Neg && fact.source != FactSource::Input {
                errors.push(ValidationError::NegativeSupportMustComeFromInput);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn supports(&self, query: &SignedGroundAtom) -> bool {
        match query.polarity {
            Polarity::Pos => {
                self.known_facts.iter().any(|fact| {
                    fact.atom == *query && !fact.support_refs.is_empty()
                }) || self.derived_facts.iter().any(|fact| {
                    fact.atom.predicate == query.predicate
                        && fact.atom.args == query.args
                        && !fact.input_refs.is_empty()
                        && !fact.proof_fragment.trim().is_empty()
                })
            }
            Polarity::Neg => self.known_facts.iter().any(|fact| {
                fact.source == FactSource::Input
                    && fact.atom == *query
                    && !fact.support_refs.is_empty()
            }),
        }
    }
}
