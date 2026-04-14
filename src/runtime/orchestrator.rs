use crate::runtime::agents::{
    backward_chaining_admissibility, contradiction_detection_admissibility,
    forward_chaining_admissibility, missing_premise_detector_admissibility,
    run_backward_chaining, run_contradiction_detection, run_forward_chaining,
    run_missing_premise_detector, unification_binding_check_admissibility,
    AgentOutcome, BACKWARD_CHAINING_AGENT_ID, CONTRADICTION_DETECTION_AGENT_ID,
    FORWARD_CHAINING_AGENT_ID, MISSING_PREMISE_DETECTOR_AGENT_ID,
    UNIFICATION_BINDING_CHECK_AGENT_ID, query_path_forward_progress_available,
};
use crate::runtime::kernel::{adjudicate, opposite_query, terminal_failure_locus_for_state};
use crate::runtime::merge::apply_merge;
use crate::runtime::readouts::ReadoutEngine;
use crate::spec::{
    AdmissibilityRecord, AgentCallRecord, BudgetSnapshot, CaseSpec, DecisionStatus,
    EvidenceEntry, EvidenceKind, FactInstance, FactSource, FinalLabel, GateRecord, GateResult,
    GateType, KernelDecision, LoopPhase, MergeRecord, Polarity, ProblemState,
    ProceduralHypothesis, ProceduralHypothesisType, ReplayBundle, RuntimeStatus, TerminalFailureLocus,
    TerminalPacket, ValidationError,
};

const SCHEMA_VERSION: &str = "mvp-1";

pub struct ExecutionArtifacts {
    pub terminal_packet: TerminalPacket,
    pub replay_bundle: ReplayBundle,
    pub kernel_decision: Option<KernelDecision>,
    pub final_state: ProblemState,
    pub admissibility_records: Vec<AdmissibilityRecord>,
    pub merge_records: Vec<MergeRecord>,
}

pub struct RuntimeRunner;

impl RuntimeRunner {
    pub fn run_case(case: &CaseSpec) -> ExecutionArtifacts {
        let mut recorder = RunRecorder::new(case.case_id.clone());
        let validation_errors = case.validate().err().unwrap_or_default();
        if !validation_errors.is_empty() {
            return finalize_contract_invalid(case, validation_errors, recorder);
        }

        let mut state = initial_problem_state(case);
        recorder.record_state(&state);

        let mut step_count = 0u32;
        let mut iteration_count = 0u32;
        let mut agent_call_count = 0u32;
        let mut admissibility_records = Vec::new();
        let mut merge_records = Vec::new();
        let mut kernel_decision = None;

        while iteration_count < case.budget.max_iterations
            && step_count < case.budget.max_steps
            && agent_call_count <= case.budget.max_agent_calls
        {
            iteration_count += 1;
            step_count += 1;

            state.status.loop_phase = LoopPhase::Measure;
            state.state_id = recorder.next_state_id("measure");
            state.lattice_readouts = Some(ReadoutEngine::evaluate(&state));
            state.evidence_ledger.push(EvidenceEntry {
                evidence_id: format!("{}::readouts", state.state_id),
                kind: EvidenceKind::ReadoutSignal,
                content_ref: state
                    .lattice_readouts
                    .as_ref()
                    .map(|packet| packet.packet_id.clone())
                    .unwrap_or_default(),
                provenance_refs: vec![format!("{}::query", case.case_id)],
                timestamp: format!("{}::readouts", state.state_id),
            });
            recorder.record_state(&state);

            let structural_gate = structural_readiness_gate(&state, recorder.next_gate_id());
            state.gate_history.push(structural_gate.clone());
            recorder.record_gate(&structural_gate);

            if matches!(structural_gate.result, GateResult::Fail) {
                state.status.loop_phase = LoopPhase::Stop;
                state.status.decision_status = DecisionStatus::Final;
                state.status.final_label = Some(FinalLabel::Undetermined);
                state.status.terminal_failure_locus =
                    Some(TerminalFailureLocus::MeasurementMalformedStructure);
                recorder.record_state(&state);
                break;
            }

            state.status.loop_phase = LoopPhase::Hypothesize;
            let hypothesis = procedural_hypothesis(&state, &mut recorder);
            state.procedural_hypotheses.push(hypothesis.clone());
            state.state_id = recorder.next_state_id("hypothesize");
            recorder.record_state(&state);

            if matches!(hypothesis.hypothesis_type, ProceduralHypothesisType::ResolveNow) {
                let evidence_gate = evidence_sufficiency_gate(&state, recorder.next_gate_id());
                state.gate_history.push(evidence_gate.clone());
                recorder.record_gate(&evidence_gate);
                kernel_decision = Some(adjudicate(&state, recorder.next_kernel_id()));
                state.status.loop_phase = LoopPhase::Resolve;
                state.status.decision_status = DecisionStatus::Final;
                state.status.final_label = kernel_decision.as_ref().map(|decision| decision.final_label.clone());
                state.status.terminal_failure_locus = terminal_failure_locus_for_state(&state);
                state.state_id = recorder.next_state_id("resolve");
                recorder.record_state(&state);
                break;
            }

            state.status.loop_phase = LoopPhase::SelectAgent;
            state.state_id = recorder.next_state_id("select_agent");
            recorder.record_state(&state);

            let agent_id = select_agent(&hypothesis);
            let admissibility = run_admissibility(&state, agent_id, recorder.next_admissibility_id());
            let admissibility_gate =
                admissibility_gate(&state, &admissibility, recorder.next_gate_id());
            state.gate_history.push(admissibility_gate.clone());
            recorder.record_gate(&admissibility_gate);
            state.evidence_ledger.push(EvidenceEntry {
                evidence_id: format!("{}::evidence", admissibility.record_id),
                kind: EvidenceKind::AdmissibilityRecord,
                content_ref: admissibility.record_id.clone(),
                provenance_refs: vec![admissibility.target_ref.clone()],
                timestamp: format!("{}::evidence", admissibility.record_id),
            });
            admissibility_records.push(admissibility.clone());

            if matches!(admissibility.result, crate::spec::AdmissibilityResult::Inapplicable) {
                if matches!(hypothesis.hypothesis_type, ProceduralHypothesisType::MissingSupport) {
                    kernel_decision = Some(adjudicate(&state, recorder.next_kernel_id()));
                    state.status.loop_phase = LoopPhase::Resolve;
                    state.status.decision_status = DecisionStatus::Final;
                    state.status.final_label =
                        kernel_decision.as_ref().map(|decision| decision.final_label.clone());
                    state.status.terminal_failure_locus =
                        Some(TerminalFailureLocus::MissingPremise);
                    state.state_id = recorder.next_state_id("resolve");
                    recorder.record_state(&state);
                    break;
                }

                let stop_gate = stop_gate(
                    &state,
                    recorder.next_gate_id(),
                    "no applicable agent remained for the selected procedural hypothesis",
                    GateResult::Pass,
                );
                state.gate_history.push(stop_gate.clone());
                recorder.record_gate(&stop_gate);
                kernel_decision = Some(adjudicate(&state, recorder.next_kernel_id()));
                state.status.loop_phase = LoopPhase::Resolve;
                state.status.decision_status = DecisionStatus::Final;
                state.status.final_label =
                    kernel_decision.as_ref().map(|decision| decision.final_label.clone());
                state.status.terminal_failure_locus = terminal_failure_locus_for_state(&state)
                    .or(Some(TerminalFailureLocus::AdmissibilityNoApplicableAgent));
                state.state_id = recorder.next_state_id("resolve");
                recorder.record_state(&state);
                break;
            }

            if agent_call_count >= case.budget.max_agent_calls {
                break;
            }
            agent_call_count += 1;
            step_count += 1;

            state.status.loop_phase = LoopPhase::ApplyAgent;
            let outcome = run_agent(&state, agent_id, recorder.next_call_id());
            state.agent_history.push(outcome.call_record.clone());
            recorder.record_agent(&outcome.call_record);

            state.status.loop_phase = LoopPhase::Merge;
            let merge_state_ref = recorder.next_state_id("merge");
            let (next_state, merge_record) = apply_merge(
                &state,
                merge_state_ref,
                outcome.call_record.call_id.clone(),
                outcome.derived_facts,
                outcome.contradictions,
            );
            merge_records.push(merge_record);
            state = next_state;
            recorder.record_state(&state);

            let evidence_gate = evidence_sufficiency_gate(&state, recorder.next_gate_id());
            state.gate_history.push(evidence_gate.clone());
            recorder.record_gate(&evidence_gate);

            if matches!(evidence_gate.result, GateResult::Pass) {
                kernel_decision = Some(adjudicate(&state, recorder.next_kernel_id()));
                state.status.loop_phase = LoopPhase::Resolve;
                state.status.decision_status = DecisionStatus::Final;
                state.status.final_label =
                    kernel_decision.as_ref().map(|decision| decision.final_label.clone());
                state.status.terminal_failure_locus = terminal_failure_locus_for_state(&state);
                state.state_id = recorder.next_state_id("resolve");
                recorder.record_state(&state);
                break;
            }

            if !outcome.notes.is_empty() && matches!(hypothesis.hypothesis_type, ProceduralHypothesisType::MissingSupport) {
                kernel_decision = Some(adjudicate(&state, recorder.next_kernel_id()));
                state.status.loop_phase = LoopPhase::Resolve;
                state.status.decision_status = DecisionStatus::Final;
                state.status.final_label =
                    kernel_decision.as_ref().map(|decision| decision.final_label.clone());
                state.status.terminal_failure_locus = Some(TerminalFailureLocus::MissingPremise);
                state.state_id = recorder.next_state_id("resolve");
                recorder.record_state(&state);
                break;
            }
        }

        let terminal_packet = if let Some(decision) = &kernel_decision {
            build_terminal_packet(
                case,
                &state,
                Some(decision),
                state.status.terminal_failure_locus.clone(),
                &recorder,
            )
        } else {
            state.status.loop_phase = LoopPhase::Stop;
            state.status.decision_status = DecisionStatus::Final;
            state.status.final_label = Some(FinalLabel::ResourceBounded);
            state.status.terminal_failure_locus = Some(TerminalFailureLocus::BudgetExhaustion);
            state.state_id = recorder.next_state_id("stop");
            recorder.record_state(&state);
            build_terminal_packet(case, &state, None, Some(TerminalFailureLocus::BudgetExhaustion), &recorder)
        };

        let replay_bundle = recorder.build_replay_bundle(
            case.case_id.clone(),
            kernel_decision.as_ref().map(|decision| decision.decision_id.clone()),
        );

        ExecutionArtifacts {
            terminal_packet,
            replay_bundle,
            kernel_decision,
            final_state: state,
            admissibility_records,
            merge_records,
        }
    }
}

fn finalize_contract_invalid(
    case: &CaseSpec,
    errors: Vec<ValidationError>,
    mut recorder: RunRecorder,
) -> ExecutionArtifacts {
    let blocking_refs = errors
        .iter()
        .map(|error| format!("validation::{error}"))
        .collect::<Vec<_>>();
    let state = ProblemState {
        state_id: recorder.next_state_id("contract_invalid"),
        case_id: case.case_id.clone(),
        semantics_profile: case.semantics_profile.clone(),
        closed_world: case.closed_world,
        domain_constants: case.domain_constants.clone(),
        budget: case.budget.clone(),
        known_facts: Vec::new(),
        active_rules: case.rules.clone(),
        query: case.query.clone(),
        derived_facts: Vec::new(),
        contradictions: Vec::new(),
        lattice_readouts: None,
        agent_history: Vec::new(),
        gate_history: Vec::new(),
        evidence_ledger: Vec::new(),
        procedural_hypotheses: Vec::new(),
        status: RuntimeStatus {
            loop_phase: LoopPhase::Stop,
            decision_status: DecisionStatus::Final,
            final_label: Some(FinalLabel::ContractInvalid),
            terminal_failure_locus: Some(TerminalFailureLocus::ContractValidation),
        },
        provenance_root: format!("{}::provenance", case.case_id),
    };
    recorder.record_state(&state);

    let terminal_packet = TerminalPacket {
        schema_version: SCHEMA_VERSION.to_string(),
        case_id: case.case_id.clone(),
        final_label: FinalLabel::ContractInvalid,
        rationale_summary: format!(
            "case failed semantic contract validation with {} error(s)",
            blocking_refs.len()
        ),
        terminal_failure_locus: Some(TerminalFailureLocus::ContractValidation),
        decisive_refs: Vec::new(),
        blocking_refs,
        semantics_profile: case.semantics_profile.clone(),
        budget_snapshot: BudgetSnapshot::from(&case.budget),
        replay_bundle_ref: format!("{}::replay", case.case_id),
        terminal_state_ref: state.state_id.clone(),
        emitted_at: format!("{}::terminal", case.case_id),
    };

    let replay_bundle = recorder.build_replay_bundle(case.case_id.clone(), None);

    ExecutionArtifacts {
        terminal_packet,
        replay_bundle,
        kernel_decision: None,
        final_state: state,
        admissibility_records: Vec::new(),
        merge_records: Vec::new(),
    }
}

fn initial_problem_state(case: &CaseSpec) -> ProblemState {
    ProblemState {
        state_id: format!("{}::state::000::init", case.case_id),
        case_id: case.case_id.clone(),
        semantics_profile: case.semantics_profile.clone(),
        closed_world: case.closed_world,
        domain_constants: case.domain_constants.clone(),
        budget: case.budget.clone(),
        known_facts: case
            .facts
            .iter()
            .map(|fact| FactInstance {
                atom: crate::spec::SignedGroundAtom {
                    predicate: fact.predicate.clone(),
                    args: fact.args.clone(),
                    polarity: fact.polarity.clone(),
                },
                source: FactSource::Input,
                support_refs: vec![fact.fact_id.clone()],
                version: 1,
            })
            .collect(),
        active_rules: case.rules.clone(),
        query: case.query.clone(),
        derived_facts: Vec::new(),
        contradictions: Vec::new(),
        lattice_readouts: None,
        agent_history: Vec::new(),
        gate_history: Vec::new(),
        evidence_ledger: case
            .facts
            .iter()
            .map(|fact| EvidenceEntry {
                evidence_id: format!("{}::evidence", fact.fact_id),
                kind: EvidenceKind::InputFact,
                content_ref: fact.fact_id.clone(),
                provenance_refs: vec![fact.fact_id.clone()],
                timestamp: format!("{}::evidence", fact.fact_id),
            })
            .collect(),
        procedural_hypotheses: Vec::new(),
        status: RuntimeStatus {
            loop_phase: LoopPhase::Init,
            decision_status: DecisionStatus::Open,
            final_label: None,
            terminal_failure_locus: None,
        },
        provenance_root: format!("{}::provenance", case.case_id),
    }
}

fn procedural_hypothesis(state: &ProblemState, recorder: &mut RunRecorder) -> ProceduralHypothesis {
    let opposite = opposite_query(&state.query);
    let forward_progress_available = query_path_forward_progress_available(state);
    let (hypothesis_type, rationale, triggered_by_refs) = if state.supports(&state.query) {
        (
            ProceduralHypothesisType::ResolveNow,
            "the query already has support".to_string(),
            vec![render_signed_atom(&state.query)],
        )
    } else if state.supports(&opposite) {
        (
            ProceduralHypothesisType::ResolveNow,
            "the opposite signed query already has support".to_string(),
            vec![render_signed_atom(&opposite)],
        )
    } else if state
        .lattice_readouts
        .as_ref()
        .map(|packet| packet.role_confusion.threshold_exceeded)
        .unwrap_or(false)
    {
        (
            ProceduralHypothesisType::MalformedStructure,
            "role confusion exceeded the runtime threshold".to_string(),
            vec![
                state
                    .lattice_readouts
                    .as_ref()
                    .map(|packet| packet.role_confusion.readout_id.clone())
                    .unwrap_or_default(),
            ],
        )
    } else if state
        .lattice_readouts
        .as_ref()
        .map(|packet| packet.collision_pressure.threshold_exceeded)
        .unwrap_or(false)
        && !state
            .agent_history
            .iter()
            .any(|call| call.agent_id == CONTRADICTION_DETECTION_AGENT_ID)
    {
        (
            ProceduralHypothesisType::Contradiction,
            "collision pressure indicates active structural tension".to_string(),
            vec![
                state
                    .lattice_readouts
                    .as_ref()
                    .map(|packet| packet.collision_pressure.readout_id.clone())
                    .unwrap_or_default(),
            ],
        )
    } else if forward_progress_available {
        (
            ProceduralHypothesisType::DirectDerivation,
            "an immediate forward derivation step is admissible".to_string(),
            vec![render_signed_atom(&state.query)],
        )
    } else if state
        .lattice_readouts
        .as_ref()
        .map(|packet| packet.coverage_gap.threshold_exceeded)
        .unwrap_or(false)
    {
        (
            ProceduralHypothesisType::MissingSupport,
            "coverage gap indicates a missing antecedent path".to_string(),
            vec![
                state
                    .lattice_readouts
                    .as_ref()
                    .map(|packet| packet.coverage_gap.readout_id.clone())
                    .unwrap_or_default(),
            ],
        )
    } else {
        (
            ProceduralHypothesisType::DirectDerivation,
            "the state is structurally ready for direct forward derivation".to_string(),
            vec![
                state
                    .lattice_readouts
                    .as_ref()
                    .map(|packet| packet.packet_id.clone())
                    .unwrap_or_default(),
            ],
        )
    };

    ProceduralHypothesis {
        hypothesis_id: recorder.next_hypothesis_id(),
        hypothesis_type,
        triggered_by_refs,
        rationale,
    }
}

fn select_agent(hypothesis: &ProceduralHypothesis) -> &'static str {
    match hypothesis.hypothesis_type {
        ProceduralHypothesisType::Contradiction => CONTRADICTION_DETECTION_AGENT_ID,
        ProceduralHypothesisType::MissingSupport => MISSING_PREMISE_DETECTOR_AGENT_ID,
        ProceduralHypothesisType::DirectDerivation => FORWARD_CHAINING_AGENT_ID,
        ProceduralHypothesisType::MalformedCase | ProceduralHypothesisType::MalformedStructure => {
            UNIFICATION_BINDING_CHECK_AGENT_ID
        }
        ProceduralHypothesisType::ResolveNow => BACKWARD_CHAINING_AGENT_ID,
    }
}

fn run_admissibility(
    state: &ProblemState,
    agent_id: &str,
    record_id: String,
) -> AdmissibilityRecord {
    match agent_id {
        CONTRADICTION_DETECTION_AGENT_ID => contradiction_detection_admissibility(state, record_id),
        MISSING_PREMISE_DETECTOR_AGENT_ID => missing_premise_detector_admissibility(state, record_id),
        FORWARD_CHAINING_AGENT_ID => forward_chaining_admissibility(state, record_id),
        BACKWARD_CHAINING_AGENT_ID => backward_chaining_admissibility(state, record_id),
        UNIFICATION_BINDING_CHECK_AGENT_ID => {
            unification_binding_check_admissibility(state, record_id)
        }
        _ => unreachable!("unknown agent id"),
    }
}

fn run_agent(state: &ProblemState, agent_id: &str, call_id: String) -> AgentOutcome {
    match agent_id {
        CONTRADICTION_DETECTION_AGENT_ID => run_contradiction_detection(state, call_id),
        MISSING_PREMISE_DETECTOR_AGENT_ID => run_missing_premise_detector(state, call_id),
        FORWARD_CHAINING_AGENT_ID => run_forward_chaining(state, call_id),
        BACKWARD_CHAINING_AGENT_ID => run_backward_chaining(state, call_id),
        UNIFICATION_BINDING_CHECK_AGENT_ID => AgentOutcome {
            call_record: AgentCallRecord {
                call_id,
                agent_id: UNIFICATION_BINDING_CHECK_AGENT_ID.to_string(),
                state_ref: state.state_id.clone(),
                selected_targets: vec![render_signed_atom(&state.query)],
                admissibility_result: crate::spec::AdmissibilityResult::Applies,
                inputs_used: vec![render_signed_atom(&state.query)],
                outputs_produced: Vec::new(),
                proof_fragment: "validated available query bindings".to_string(),
                status: crate::spec::AgentCallStatus::NoOp,
                failure_code: None,
            },
            derived_facts: Vec::new(),
            contradictions: Vec::new(),
            notes: Vec::new(),
        },
        _ => unreachable!("unknown agent id"),
    }
}

fn structural_readiness_gate(state: &ProblemState, gate_id: String) -> GateRecord {
    let malformed = state
        .lattice_readouts
        .as_ref()
        .map(|packet| packet.aggregate_flags.structurally_malformed)
        .unwrap_or(false);

    GateRecord {
        gate_id: gate_id.clone(),
        gate_type: GateType::StructuralReadiness,
        state_ref: state.state_id.clone(),
        inputs: state
            .lattice_readouts
            .as_ref()
            .map(|packet| vec![packet.packet_id.clone()])
            .unwrap_or_default(),
        result: if malformed {
            GateResult::Fail
        } else {
            GateResult::Pass
        },
        rationale: if malformed {
            "runtime state is structurally malformed".to_string()
        } else {
            "runtime state is structurally ready".to_string()
        },
        emitted_at: format!("{gate_id}::emitted"),
    }
}

fn admissibility_gate(
    state: &ProblemState,
    record: &AdmissibilityRecord,
    gate_id: String,
) -> GateRecord {
    GateRecord {
        gate_id: gate_id.clone(),
        gate_type: GateType::Admissibility,
        state_ref: state.state_id.clone(),
        inputs: vec![record.record_id.clone()],
        result: if matches!(record.result, crate::spec::AdmissibilityResult::Applies) {
            GateResult::Pass
        } else {
            GateResult::Fail
        },
        rationale: if matches!(record.result, crate::spec::AdmissibilityResult::Applies) {
            "selected agent passed admissibility".to_string()
        } else {
            "selected agent failed admissibility".to_string()
        },
        emitted_at: format!("{gate_id}::emitted"),
    }
}

fn evidence_sufficiency_gate(state: &ProblemState, gate_id: String) -> GateRecord {
    let opposite = opposite_query(&state.query);
    let supports_query = state.supports(&state.query);
    let supports_opposite = state.supports(&opposite);
    let enough = supports_query
        || supports_opposite
        || (state.query.polarity == Polarity::Pos
            && state
                .lattice_readouts
                .as_ref()
                .map(|packet| packet.coverage_gap.threshold_exceeded)
                .unwrap_or(false)
            && state
                .agent_history
                .iter()
                .any(|call| call.agent_id == MISSING_PREMISE_DETECTOR_AGENT_ID));

    GateRecord {
        gate_id: gate_id.clone(),
        gate_type: GateType::EvidenceSufficiency,
        state_ref: state.state_id.clone(),
        inputs: vec![render_signed_atom(&state.query), render_signed_atom(&opposite)],
        result: if enough {
            GateResult::Pass
        } else {
            GateResult::Defer
        },
        rationale: if enough {
            "state contains enough evidence for semantic adjudication".to_string()
        } else {
            "state still needs another deterministic reasoning step".to_string()
        },
        emitted_at: format!("{gate_id}::emitted"),
    }
}

fn stop_gate(state: &ProblemState, gate_id: String, rationale: &str, result: GateResult) -> GateRecord {
    GateRecord {
        gate_id: gate_id.clone(),
        gate_type: GateType::Stop,
        state_ref: state.state_id.clone(),
        inputs: vec![state.state_id.clone()],
        result,
        rationale: rationale.to_string(),
        emitted_at: format!("{gate_id}::emitted"),
    }
}

fn build_terminal_packet(
    case: &CaseSpec,
    state: &ProblemState,
    kernel_decision: Option<&KernelDecision>,
    terminal_failure_locus: Option<TerminalFailureLocus>,
    recorder: &RunRecorder,
) -> TerminalPacket {
    let (final_label, rationale_summary, decisive_refs, blocking_refs) = match kernel_decision {
        Some(decision) => (
            decision.final_label.clone(),
            decision.rationale_summary.clone(),
            decision.decisive_refs.clone(),
            decision.blocking_refs.clone(),
        ),
        None => match state.status.final_label.clone() {
            Some(FinalLabel::ResourceBounded) | None => (
                FinalLabel::ResourceBounded,
                "the published budget expired before a terminal semantic label was reached"
                    .to_string(),
                Vec::new(),
                vec![format!(
                    "budget::{}/{}/{}",
                    state.budget.max_steps,
                    state.budget.max_agent_calls,
                    state.budget.max_iterations
                )],
            ),
            Some(final_label) => (
                final_label,
                match terminal_failure_locus {
                    Some(TerminalFailureLocus::MeasurementMalformedStructure) => {
                        "runtime halted because the structural readiness gate failed".to_string()
                    }
                    Some(TerminalFailureLocus::AdmissibilityNoApplicableAgent) => {
                        "runtime halted because no admissible next step remained".to_string()
                    }
                    Some(TerminalFailureLocus::MissingPremise) => {
                        "runtime halted after diagnosing a missing premise".to_string()
                    }
                    Some(TerminalFailureLocus::BudgetExhaustion) => {
                        "the published budget expired before a terminal semantic label was reached"
                            .to_string()
                    }
                    Some(TerminalFailureLocus::ContradictionBlock) => {
                        "runtime halted because contradiction touched the query state".to_string()
                    }
                    Some(TerminalFailureLocus::ContractValidation) => {
                        "runtime halted because the case contract was invalid".to_string()
                    }
                    Some(TerminalFailureLocus::KernelUnresolved) | None => {
                        "runtime halted without kernel adjudication".to_string()
                    }
                },
                Vec::new(),
                Vec::new(),
            ),
        },
    };

    TerminalPacket {
        schema_version: SCHEMA_VERSION.to_string(),
        case_id: case.case_id.clone(),
        final_label,
        rationale_summary,
        terminal_failure_locus,
        decisive_refs,
        blocking_refs,
        semantics_profile: case.semantics_profile.clone(),
        budget_snapshot: BudgetSnapshot::from(&case.budget),
        replay_bundle_ref: recorder.replay_bundle_ref(),
        terminal_state_ref: state.state_id.clone(),
        emitted_at: format!("{}::terminal", case.case_id),
    }
}

fn render_signed_atom(atom: &crate::spec::SignedGroundAtom) -> String {
    let core = format!("{}({})", atom.predicate, atom.args.join(","));
    if atom.polarity == Polarity::Neg {
        format!("not {core}")
    } else {
        core
    }
}

struct RunRecorder {
    case_id: String,
    state_seq: u32,
    gate_seq: u32,
    admissibility_seq: u32,
    call_seq: u32,
    hypothesis_seq: u32,
    kernel_seq: u32,
    ordered_state_refs: Vec<String>,
    ordered_agent_calls: Vec<String>,
    ordered_gate_records: Vec<String>,
}

impl RunRecorder {
    fn new(case_id: String) -> Self {
        Self {
            case_id,
            state_seq: 1,
            gate_seq: 1,
            admissibility_seq: 1,
            call_seq: 1,
            hypothesis_seq: 1,
            kernel_seq: 1,
            ordered_state_refs: Vec::new(),
            ordered_agent_calls: Vec::new(),
            ordered_gate_records: Vec::new(),
        }
    }

    fn next_state_id(&mut self, stage: &str) -> String {
        let id = format!("{}::state::{:03}::{stage}", self.case_id, self.state_seq);
        self.state_seq += 1;
        id
    }

    fn next_gate_id(&mut self) -> String {
        let id = format!("{}::gate::{:03}", self.case_id, self.gate_seq);
        self.gate_seq += 1;
        id
    }

    fn next_admissibility_id(&mut self) -> String {
        let id = format!("{}::admissibility::{:03}", self.case_id, self.admissibility_seq);
        self.admissibility_seq += 1;
        id
    }

    fn next_call_id(&mut self) -> String {
        let id = format!("{}::call::{:03}", self.case_id, self.call_seq);
        self.call_seq += 1;
        id
    }

    fn next_hypothesis_id(&mut self) -> String {
        let id = format!("{}::hypothesis::{:03}", self.case_id, self.hypothesis_seq);
        self.hypothesis_seq += 1;
        id
    }

    fn next_kernel_id(&mut self) -> String {
        let id = format!("{}::kernel::{:03}", self.case_id, self.kernel_seq);
        self.kernel_seq += 1;
        id
    }

    fn record_state(&mut self, state: &ProblemState) {
        if self
            .ordered_state_refs
            .last()
            .map(|last| last != &state.state_id)
            .unwrap_or(true)
        {
            self.ordered_state_refs.push(state.state_id.clone());
        }
    }

    fn record_agent(&mut self, call: &AgentCallRecord) {
        self.ordered_agent_calls.push(call.call_id.clone());
    }

    fn record_gate(&mut self, gate: &GateRecord) {
        self.ordered_gate_records.push(gate.gate_id.clone());
    }

    fn replay_bundle_ref(&self) -> String {
        format!("{}::replay", self.case_id)
    }

    fn build_replay_bundle(
        &self,
        case_id: String,
        kernel_decision_ref: Option<String>,
    ) -> ReplayBundle {
        ReplayBundle {
            bundle_id: self.replay_bundle_ref(),
            case_id,
            ordered_state_refs: self.ordered_state_refs.clone(),
            ordered_agent_calls: self.ordered_agent_calls.clone(),
            ordered_gate_records: self.ordered_gate_records.clone(),
            kernel_decision_ref,
            checksum: checksum_for_bundle(
                &self.ordered_state_refs,
                &self.ordered_agent_calls,
                &self.ordered_gate_records,
            ),
        }
    }
}

fn checksum_for_bundle(
    ordered_state_refs: &[String],
    ordered_agent_calls: &[String],
    ordered_gate_records: &[String],
) -> String {
    let mut checksum: u64 = 0xcbf29ce484222325;
    for segment in ordered_state_refs
        .iter()
        .chain(ordered_agent_calls.iter())
        .chain(ordered_gate_records.iter())
    {
        for byte in segment.as_bytes() {
            checksum ^= *byte as u64;
            checksum = checksum.wrapping_mul(0x100000001b3);
        }
        checksum ^= 0xff;
        checksum = checksum.wrapping_mul(0x100000001b3);
    }
    format!("{checksum:016x}")
}
