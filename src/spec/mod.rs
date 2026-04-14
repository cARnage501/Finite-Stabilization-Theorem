pub mod agent;
pub mod case;
pub mod errors;
pub mod gate;
pub mod kernel;
pub mod readout;
pub mod replay;
pub mod state;
pub mod terminal;

pub use agent::{AdmissibilityRecord, AdmissibilityResult, AgentCallRecord, AgentCallStatus, MergeRecord, MergeStatus};
pub use case::{
    BudgetContract, CaseMetadata, CaseSpec, Difficulty, GoldCaseAnnotation, Polarity,
    PositiveAtom, PositiveRule, SemanticsProfile, SignedGroundAtom, SignedGroundFact,
    SourceType,
};
pub use errors::ValidationError;
pub use gate::{GateRecord, GateResult, GateType};
pub use readout::{AggregateFlags, LatticeReadoutPacket, Readout};
pub use replay::{HandoffConstraints, HandoffEnvelope, HandoffPermissions, ReplayBundle};
pub use state::{
    ContradictionRecord, ContradictionStatus, DecisionStatus, DerivedPositiveFact,
    EvidenceEntry, EvidenceKind, FactInstance, FactSource, LoopPhase, ProblemState,
    ProceduralHypothesis, ProceduralHypothesisType, RuntimeStatus, SideEffectMode,
    WorkItem, WorkItemConstraints, WorkItemStatus, WorkItemType,
};
pub use terminal::{
    BudgetSnapshot, FinalLabel, KernelDecision, TerminalFailureLocus, TerminalPacket,
};
