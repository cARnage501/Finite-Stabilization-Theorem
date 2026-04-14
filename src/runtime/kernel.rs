use crate::spec::{
    FinalLabel, KernelDecision, Polarity, ProblemState, SignedGroundAtom, TerminalFailureLocus,
};

pub(crate) fn adjudicate(state: &ProblemState, decision_id: String) -> KernelDecision {
    let query = &state.query;
    let opposite = opposite_query(query);
    let supports_query = state.supports(query);
    let supports_opposite = state.supports(&opposite);

    let (final_label, rationale_summary, decisive_refs, blocking_refs) =
        if supports_query && supports_opposite {
            (
                FinalLabel::InconsistentBase,
                "both the query and its opposite have support under the current state".to_string(),
                support_refs(state, query),
                support_refs(state, &opposite),
            )
        } else if supports_query {
            (
                FinalLabel::Entailed,
                "the query has support and its opposite does not".to_string(),
                support_refs(state, query),
                Vec::new(),
            )
        } else if supports_opposite {
            (
                FinalLabel::Refuted,
                "the opposite signed query has support".to_string(),
                support_refs(state, &opposite),
                support_refs(state, query),
            )
        } else {
            (
                FinalLabel::Undetermined,
                "neither the query nor its opposite has support".to_string(),
                Vec::new(),
                state
                    .contradictions
                    .iter()
                    .map(|record| record.contradiction_id.clone())
                    .collect(),
            )
        };

    KernelDecision {
        decision_id: decision_id.clone(),
        state_ref: state.state_id.clone(),
        semantics_profile: state.semantics_profile.clone(),
        final_label,
        rationale_summary,
        decisive_refs,
        blocking_refs,
        emitted_at: format!("{decision_id}::emitted"),
    }
}

pub(crate) fn terminal_failure_locus_for_state(state: &ProblemState) -> Option<TerminalFailureLocus> {
    let query = &state.query;
    let opposite = opposite_query(query);
    let supports_query = state.supports(query);
    let supports_opposite = state.supports(&opposite);

    if supports_query && supports_opposite {
        Some(TerminalFailureLocus::ContradictionBlock)
    } else if !supports_query
        && !supports_opposite
        && state
            .lattice_readouts
            .as_ref()
            .map(|packet| packet.coverage_gap.threshold_exceeded)
            .unwrap_or(false)
    {
        Some(TerminalFailureLocus::MissingPremise)
    } else {
        None
    }
}

pub(crate) fn opposite_query(query: &SignedGroundAtom) -> SignedGroundAtom {
    SignedGroundAtom {
        predicate: query.predicate.clone(),
        args: query.args.clone(),
        polarity: match query.polarity {
            Polarity::Pos => Polarity::Neg,
            Polarity::Neg => Polarity::Pos,
        },
    }
}

fn support_refs(state: &ProblemState, query: &SignedGroundAtom) -> Vec<String> {
    let mut refs = state
        .known_facts
        .iter()
        .filter(|fact| fact.atom == *query)
        .flat_map(|fact| fact.support_refs.clone())
        .collect::<Vec<_>>();

    if query.polarity == Polarity::Pos {
        refs.extend(
            state
                .derived_facts
                .iter()
                .filter(|fact| fact.atom.predicate == query.predicate && fact.atom.args == query.args)
                .map(|fact| fact.derivation_id.clone()),
        );
    }

    refs.sort();
    refs.dedup();
    refs
}
