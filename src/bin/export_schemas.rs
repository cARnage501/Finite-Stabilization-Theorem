use std::fs;
use std::path::Path;

use governed_reasoning_mvp::spec::{
    AdmissibilityRecord, AgentCallRecord, CaseSpec, GateRecord, HandoffEnvelope,
    KernelDecision, LatticeReadoutPacket, MergeRecord, PositiveRule, ProblemState,
    ReplayBundle, SignedGroundFact, TerminalPacket, WorkItem,
};
use schemars::{schema_for, JsonSchema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    export_schema::<CaseSpec>("CaseSpec")?;
    export_schema::<SignedGroundFact>("SignedGroundFact")?;
    export_schema::<PositiveRule>("PositiveRule")?;
    export_schema::<ProblemState>("ProblemState")?;
    export_schema::<LatticeReadoutPacket>("LatticeReadoutPacket")?;
    export_schema::<WorkItem>("WorkItem")?;
    export_schema::<HandoffEnvelope>("HandoffEnvelope")?;
    export_schema::<AdmissibilityRecord>("AdmissibilityRecord")?;
    export_schema::<AgentCallRecord>("AgentCallRecord")?;
    export_schema::<MergeRecord>("MergeRecord")?;
    export_schema::<GateRecord>("GateRecord")?;
    export_schema::<KernelDecision>("KernelDecision")?;
    export_schema::<TerminalPacket>("TerminalPacket")?;
    export_schema::<ReplayBundle>("ReplayBundle")?;
    Ok(())
}

fn export_schema<T: JsonSchema>(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let schema = schema_for!(T);
    let schema_dir = Path::new("specs");
    fs::create_dir_all(schema_dir)?;
    let path = schema_dir.join(format!("{name}.schema.json"));
    fs::write(path, serde_json::to_string_pretty(&schema)?)?;
    Ok(())
}
