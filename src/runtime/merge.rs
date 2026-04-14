use crate::spec::{
    ContradictionRecord, DerivedPositiveFact, EvidenceEntry, EvidenceKind, MergeRecord,
    MergeStatus, ProblemState,
};

pub(crate) fn apply_merge(
    state: &ProblemState,
    post_state_ref: String,
    call_ref: String,
    derived_facts: Vec<DerivedPositiveFact>,
    contradictions: Vec<ContradictionRecord>,
) -> (ProblemState, MergeRecord) {
    let mut next_state = state.clone();
    next_state.state_id = post_state_ref.clone();

    let mut added_facts = Vec::new();
    let mut added_contradictions = Vec::new();
    let mut rejected_outputs = Vec::new();

    for fact in derived_facts {
        if fact.input_refs.is_empty()
            || fact.proof_fragment.trim().is_empty()
            || fact.produced_by_agent.trim().is_empty()
        {
            rejected_outputs.push(fact.derivation_id.clone());
            continue;
        }
        if next_state
            .derived_facts
            .iter()
            .any(|existing| existing.atom == fact.atom)
        {
            rejected_outputs.push(fact.derivation_id.clone());
            continue;
        }

        added_facts.push(fact.derivation_id.clone());
        next_state.evidence_ledger.push(EvidenceEntry {
            evidence_id: format!("{}::evidence", fact.derivation_id),
            kind: EvidenceKind::DerivedFact,
            content_ref: fact.derivation_id.clone(),
            provenance_refs: fact.input_refs.clone(),
            timestamp: format!("{}::evidence", fact.derivation_id),
        });
        next_state.derived_facts.push(fact);
    }

    for contradiction in contradictions {
        if contradiction.positive_ref.trim().is_empty()
            || contradiction.negative_ref.trim().is_empty()
            || contradiction.positive_ref == contradiction.negative_ref
        {
            rejected_outputs.push(contradiction.contradiction_id.clone());
            continue;
        }
        if next_state.contradictions.iter().any(|existing| {
            existing.positive_ref == contradiction.positive_ref
                && existing.negative_ref == contradiction.negative_ref
        }) {
            rejected_outputs.push(contradiction.contradiction_id.clone());
            continue;
        }

        added_contradictions.push(contradiction.contradiction_id.clone());
        next_state.evidence_ledger.push(EvidenceEntry {
            evidence_id: format!("{}::evidence", contradiction.contradiction_id),
            kind: EvidenceKind::ContradictionRecord,
            content_ref: contradiction.contradiction_id.clone(),
            provenance_refs: vec![
                contradiction.positive_ref.clone(),
                contradiction.negative_ref.clone(),
            ],
            timestamp: format!("{}::evidence", contradiction.contradiction_id),
        });
        next_state.contradictions.push(contradiction);
    }

    let merge_status = if !added_facts.is_empty() || !added_contradictions.is_empty() {
        if rejected_outputs.is_empty() {
            MergeStatus::Applied
        } else {
            MergeStatus::Partial
        }
    } else {
        MergeStatus::NoOp
    };

    (
        next_state,
        MergeRecord {
            merge_id: format!("{call_ref}::merge"),
            pre_state_ref: state.state_id.clone(),
            post_state_ref,
            call_ref,
            added_facts,
            added_contradictions,
            rejected_outputs,
            merge_status,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::runtime::merge::apply_merge;
    use crate::spec::{
        BudgetContract, ContradictionRecord, ContradictionStatus, DecisionStatus,
        DerivedPositiveFact, LoopPhase, Polarity, PositiveAtom, ProblemState, RuntimeStatus,
        SemanticsProfile, SignedGroundAtom,
    };

    fn base_state() -> ProblemState {
        ProblemState {
            state_id: "state-1".to_string(),
            case_id: "case-1".to_string(),
            semantics_profile: SemanticsProfile::Classical,
            closed_world: false,
            domain_constants: vec!["a".to_string()],
            budget: BudgetContract {
                max_steps: 4,
                max_agent_calls: 2,
                max_iterations: 2,
            },
            known_facts: Vec::new(),
            active_rules: Vec::new(),
            query: SignedGroundAtom {
                predicate: "q".to_string(),
                args: vec!["a".to_string()],
                polarity: Polarity::Pos,
            },
            derived_facts: Vec::new(),
            contradictions: Vec::new(),
            lattice_readouts: None,
            agent_history: Vec::new(),
            gate_history: Vec::new(),
            evidence_ledger: Vec::new(),
            procedural_hypotheses: Vec::new(),
            status: RuntimeStatus {
                loop_phase: LoopPhase::Init,
                decision_status: DecisionStatus::Open,
                final_label: None,
                terminal_failure_locus: None,
            },
            provenance_root: "root".to_string(),
        }
    }

    #[test]
    fn merge_rejects_malformed_outputs() {
        let state = base_state();
        let malformed_fact = DerivedPositiveFact {
            atom: PositiveAtom {
                predicate: "q".to_string(),
                args: vec!["a".to_string()],
            },
            produced_by_agent: String::new(),
            input_refs: Vec::new(),
            proof_fragment: String::new(),
            derivation_id: "bad-derive".to_string(),
        };
        let malformed_contradiction = ContradictionRecord {
            contradiction_id: "bad-contradiction".to_string(),
            positive_ref: "same".to_string(),
            negative_ref: "same".to_string(),
            scope: "q(a)".to_string(),
            status: ContradictionStatus::Active,
        };

        let (next_state, merge_record) = apply_merge(
            &state,
            "state-2".to_string(),
            "call-1".to_string(),
            vec![malformed_fact],
            vec![malformed_contradiction],
        );

        assert!(next_state.derived_facts.is_empty());
        assert!(next_state.contradictions.is_empty());
        assert_eq!(merge_record.rejected_outputs.len(), 2);
    }
}
