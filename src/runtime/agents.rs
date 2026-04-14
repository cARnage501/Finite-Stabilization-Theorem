use std::collections::{BTreeMap, BTreeSet};

use crate::spec::{
    AdmissibilityRecord, AdmissibilityResult, AgentCallRecord, AgentCallStatus,
    ContradictionRecord, ContradictionStatus, DerivedPositiveFact, FactSource, Polarity,
    PositiveAtom, PositiveRule, ProblemState, SignedGroundAtom,
};

pub(crate) const FORWARD_CHAINING_AGENT_ID: &str = "forward_chaining";
pub(crate) const BACKWARD_CHAINING_AGENT_ID: &str = "backward_chaining";
pub(crate) const UNIFICATION_BINDING_CHECK_AGENT_ID: &str = "unification_binding_check";
pub(crate) const CONTRADICTION_DETECTION_AGENT_ID: &str = "contradiction_detection";
pub(crate) const MISSING_PREMISE_DETECTOR_AGENT_ID: &str = "missing_premise_detector";

pub(crate) struct AgentOutcome {
    pub call_record: AgentCallRecord,
    pub derived_facts: Vec<DerivedPositiveFact>,
    pub contradictions: Vec<ContradictionRecord>,
    pub notes: Vec<String>,
}

pub(crate) fn contradiction_detection_admissibility(
    state: &ProblemState,
    record_id: String,
) -> AdmissibilityRecord {
    let contested_targets = find_contested_targets(state);
    let result = if contested_targets.is_empty() {
        AdmissibilityResult::Inapplicable
    } else {
        AdmissibilityResult::Applies
    };

    AdmissibilityRecord {
        record_id: record_id.clone(),
        agent_id: CONTRADICTION_DETECTION_AGENT_ID.to_string(),
        target_ref: state.state_id.clone(),
        result,
        reasons: if contested_targets.is_empty() {
            vec!["no contested targets are active".to_string()]
        } else {
            vec!["support and opposition both exist for one or more targets".to_string()]
        },
        missing_requirements: Vec::new(),
        binding_candidates: contested_targets,
        emitted_at: format!("{record_id}::emitted"),
    }
}

pub(crate) fn run_contradiction_detection(
    state: &ProblemState,
    call_id: String,
) -> AgentOutcome {
    let contradictions = find_contested_targets(state)
        .into_iter()
        .filter_map(|target| contradiction_for_target(state, &call_id, &target))
        .collect::<Vec<_>>();
    let outputs = contradictions
        .iter()
        .map(|record| record.contradiction_id.clone())
        .collect::<Vec<_>>();

    AgentOutcome {
        call_record: AgentCallRecord {
            call_id: call_id.clone(),
            agent_id: CONTRADICTION_DETECTION_AGENT_ID.to_string(),
            state_ref: state.state_id.clone(),
            selected_targets: outputs.clone(),
            admissibility_result: if outputs.is_empty() {
                AdmissibilityResult::Inapplicable
            } else {
                AdmissibilityResult::Applies
            },
            inputs_used: outputs.clone(),
            outputs_produced: outputs,
            proof_fragment: "registered active support/opposition collisions".to_string(),
            status: if contradictions.is_empty() {
                AgentCallStatus::NoOp
            } else {
                AgentCallStatus::Success
            },
            failure_code: None,
        },
        derived_facts: Vec::new(),
        contradictions,
        notes: Vec::new(),
    }
}

pub(crate) fn forward_chaining_admissibility(
    state: &ProblemState,
    record_id: String,
) -> AdmissibilityRecord {
    let derivable = candidate_forward_derivations(state);
    let result = if derivable.is_empty() {
        AdmissibilityResult::Inapplicable
    } else {
        AdmissibilityResult::Applies
    };

    AdmissibilityRecord {
        record_id: record_id.clone(),
        agent_id: FORWARD_CHAINING_AGENT_ID.to_string(),
        target_ref: render_signed_atom(&state.query),
        result,
        reasons: if derivable.is_empty() {
            vec!["no rule has a fully supported antecedent set".to_string()]
        } else {
            vec!["at least one rule can derive an immediate positive consequence".to_string()]
        },
        missing_requirements: Vec::new(),
        binding_candidates: derivable
            .iter()
            .map(|fact| fact.derivation_id.clone())
            .collect(),
        emitted_at: format!("{record_id}::emitted"),
    }
}

pub(crate) fn query_path_forward_progress_available(state: &ProblemState) -> bool {
    let relevant_predicates = query_support_predicates(state);
    candidate_forward_derivations(state)
        .into_iter()
        .any(|fact| relevant_predicates.contains(&fact.atom.predicate))
}

pub(crate) fn run_forward_chaining(
    state: &ProblemState,
    call_id: String,
) -> AgentOutcome {
    let derived_facts = candidate_forward_derivations(state);
    let outputs = derived_facts
        .iter()
        .map(|fact| fact.derivation_id.clone())
        .collect::<Vec<_>>();

    AgentOutcome {
        call_record: AgentCallRecord {
            call_id,
            agent_id: FORWARD_CHAINING_AGENT_ID.to_string(),
            state_ref: state.state_id.clone(),
            selected_targets: vec![render_signed_atom(&state.query)],
            admissibility_result: if outputs.is_empty() {
                AdmissibilityResult::Inapplicable
            } else {
                AdmissibilityResult::Applies
            },
            inputs_used: derivation_inputs(&derived_facts),
            outputs_produced: outputs,
            proof_fragment: "derived immediate positive consequences from supported rules".to_string(),
            status: if derived_facts.is_empty() {
                AgentCallStatus::NoOp
            } else {
                AgentCallStatus::Success
            },
            failure_code: None,
        },
        derived_facts,
        contradictions: Vec::new(),
        notes: Vec::new(),
    }
}

pub(crate) fn backward_chaining_admissibility(
    state: &ProblemState,
    record_id: String,
) -> AdmissibilityRecord {
    let candidate_rules = query_matching_rules(state);
    let result = if candidate_rules.is_empty() {
        AdmissibilityResult::Inapplicable
    } else {
        AdmissibilityResult::Applies
    };

    AdmissibilityRecord {
        record_id: record_id.clone(),
        agent_id: BACKWARD_CHAINING_AGENT_ID.to_string(),
        target_ref: render_signed_atom(&state.query),
        result,
        reasons: if matches!(state.query.polarity, Polarity::Neg) {
            vec!["backward chaining is only used for positive query support".to_string()]
        } else if candidate_rules.is_empty() {
            vec!["no rule consequent matches the query".to_string()]
        } else {
            vec!["one or more rules can be reduced against the query".to_string()]
        },
        missing_requirements: Vec::new(),
        binding_candidates: candidate_rules,
        emitted_at: format!("{record_id}::emitted"),
    }
}

pub(crate) fn run_backward_chaining(
    state: &ProblemState,
    call_id: String,
) -> AgentOutcome {
    let selected_targets = query_matching_rules(state);
    let outputs = selected_targets
        .iter()
        .map(|rule_id| format!("{call_id}::plan::{rule_id}"))
        .collect::<Vec<_>>();

    AgentOutcome {
        call_record: AgentCallRecord {
            call_id,
            agent_id: BACKWARD_CHAINING_AGENT_ID.to_string(),
            state_ref: state.state_id.clone(),
            selected_targets,
            admissibility_result: if outputs.is_empty() {
                AdmissibilityResult::Inapplicable
            } else {
                AdmissibilityResult::Applies
            },
            inputs_used: vec![render_signed_atom(&state.query)],
            outputs_produced: outputs,
            proof_fragment: "reduced query to candidate support rules".to_string(),
            status: if state.query.polarity == Polarity::Neg {
                AgentCallStatus::NoOp
            } else if query_matching_rules(state).is_empty() {
                AgentCallStatus::NoOp
            } else {
                AgentCallStatus::Success
            },
            failure_code: None,
        },
        derived_facts: Vec::new(),
        contradictions: Vec::new(),
        notes: Vec::new(),
    }
}

pub(crate) fn unification_binding_check_admissibility(
    state: &ProblemState,
    record_id: String,
) -> AdmissibilityRecord {
    let candidates = binding_candidates_for_query(state);
    let result = if candidates.is_empty() {
        AdmissibilityResult::Inapplicable
    } else {
        AdmissibilityResult::Applies
    };

    AdmissibilityRecord {
        record_id: record_id.clone(),
        agent_id: UNIFICATION_BINDING_CHECK_AGENT_ID.to_string(),
        target_ref: render_signed_atom(&state.query),
        result,
        reasons: if candidates.is_empty() {
            vec!["no valid consequent-to-query bindings were found".to_string()]
        } else {
            vec!["candidate query bindings are available".to_string()]
        },
        missing_requirements: Vec::new(),
        binding_candidates: candidates,
        emitted_at: format!("{record_id}::emitted"),
    }
}

pub(crate) fn missing_premise_detector_admissibility(
    state: &ProblemState,
    record_id: String,
) -> AdmissibilityRecord {
    let missing_atoms = missing_premise_candidates(state);
    let result = if missing_atoms.is_empty() {
        AdmissibilityResult::Inapplicable
    } else {
        AdmissibilityResult::Applies
    };

    AdmissibilityRecord {
        record_id: record_id.clone(),
        agent_id: MISSING_PREMISE_DETECTOR_AGENT_ID.to_string(),
        target_ref: render_signed_atom(&state.query),
        result,
        reasons: if missing_atoms.is_empty() {
            vec!["no near-miss support path exists for the query".to_string()]
        } else {
            vec!["query support is blocked by a missing antecedent".to_string()]
        },
        missing_requirements: missing_atoms,
        binding_candidates: query_matching_rules(state),
        emitted_at: format!("{record_id}::emitted"),
    }
}

pub(crate) fn run_missing_premise_detector(
    state: &ProblemState,
    call_id: String,
) -> AgentOutcome {
    let missing_atoms = missing_premise_candidates(state);
    let outputs = missing_atoms
        .iter()
        .map(|atom| format!("{call_id}::missing::{atom}"))
        .collect::<Vec<_>>();

    AgentOutcome {
        call_record: AgentCallRecord {
            call_id,
            agent_id: MISSING_PREMISE_DETECTOR_AGENT_ID.to_string(),
            state_ref: state.state_id.clone(),
            selected_targets: vec![render_signed_atom(&state.query)],
            admissibility_result: if outputs.is_empty() {
                AdmissibilityResult::Inapplicable
            } else {
                AdmissibilityResult::Applies
            },
            inputs_used: query_matching_rules(state),
            outputs_produced: outputs,
            proof_fragment: "diagnosed the minimal missing antecedent set for the query".to_string(),
            status: if missing_atoms.is_empty() {
                AgentCallStatus::NoOp
            } else {
                AgentCallStatus::Success
            },
            failure_code: None,
        },
        derived_facts: Vec::new(),
        contradictions: Vec::new(),
        notes: missing_atoms,
    }
}

fn candidate_forward_derivations(state: &ProblemState) -> Vec<DerivedPositiveFact> {
    let domain = domain_set(state);
    let mut derived = Vec::new();

    for rule in &state.active_rules {
        let Some(variables) = rule_variables(rule, &domain) else {
            continue;
        };

        let substitutions = enumerate_substitutions(&variables, &state.domain_constants);
        for bindings in substitutions {
            let grounded_antecedents = rule
                .antecedents
                .iter()
                .map(|atom| ground_positive_atom(atom, &bindings, &domain))
                .collect::<Option<Vec<_>>>();

            let Some(grounded_antecedents) = grounded_antecedents else {
                continue;
            };

            if !grounded_antecedents
                .iter()
                .all(|atom| positive_support_available(state, atom))
            {
                continue;
            }

            if state.semantics_profile.quarantines_query_touching_contradiction()
                && grounded_antecedents
                    .iter()
                    .any(|atom| positive_atom_is_contested(state, atom))
            {
                continue;
            }

            let Some(consequent) = ground_positive_atom(&rule.consequent, &bindings, &domain) else {
                continue;
            };

            if positive_support_available(state, &consequent) {
                continue;
            }

            let input_refs = grounded_antecedents
                .iter()
                .flat_map(|atom| positive_support_refs(state, atom))
                .collect::<Vec<_>>();
            if input_refs.is_empty() {
                continue;
            }

            derived.push(DerivedPositiveFact {
                atom: consequent.clone(),
                produced_by_agent: FORWARD_CHAINING_AGENT_ID.to_string(),
                input_refs,
                proof_fragment: format!(
                    "{} => {}",
                    render_positive_atoms(&grounded_antecedents),
                    render_positive_atom(&consequent)
                ),
                derivation_id: format!(
                    "{}::derive::{}::{}",
                    state.state_id,
                    rule.rule_id,
                    render_positive_atom(&consequent)
                ),
            });
        }
    }

    derived.sort_by(|left, right| left.derivation_id.cmp(&right.derivation_id));
    derived.dedup_by(|left, right| left.atom == right.atom);
    derived
}

fn query_matching_rules(state: &ProblemState) -> Vec<String> {
    if state.query.polarity == Polarity::Neg {
        return Vec::new();
    }

    binding_candidates_for_query(state)
        .into_iter()
        .filter_map(|candidate| candidate.split("::").next().map(str::to_string))
        .collect()
}

fn query_support_predicates(state: &ProblemState) -> BTreeSet<String> {
    let mut predicates = BTreeSet::from([state.query.predicate.clone()]);
    let mut changed = true;

    while changed {
        changed = false;
        for rule in &state.active_rules {
            if predicates.contains(&rule.consequent.predicate) {
                for antecedent in &rule.antecedents {
                    if predicates.insert(antecedent.predicate.clone()) {
                        changed = true;
                    }
                }
            }
        }
    }

    predicates
}

fn binding_candidates_for_query(state: &ProblemState) -> Vec<String> {
    if state.query.polarity == Polarity::Neg {
        return Vec::new();
    }

    let domain = domain_set(state);
    let mut candidates = Vec::new();
    for rule in &state.active_rules {
        if let Some(bindings) = unify_rule_consequent_to_query(rule, &state.query, &domain) {
            let binding_render = bindings
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",");
            candidates.push(format!("{}::{}", rule.rule_id, binding_render));
        }
    }

    candidates.sort();
    candidates
}

fn missing_premise_candidates(state: &ProblemState) -> Vec<String> {
    if state.query.polarity == Polarity::Neg {
        return Vec::new();
    }

    let domain = domain_set(state);
    let mut missing_atoms = BTreeSet::new();

    for rule in &state.active_rules {
        let Some(bindings) = unify_rule_consequent_to_query(rule, &state.query, &domain) else {
            continue;
        };

        for antecedent in &rule.antecedents {
            let Some(ground) = ground_positive_atom(antecedent, &bindings, &domain) else {
                continue;
            };
            if !positive_support_available(state, &ground) {
                missing_atoms.insert(render_positive_atom(&ground));
            }
        }
    }

    missing_atoms.into_iter().collect()
}

fn find_contested_targets(state: &ProblemState) -> Vec<String> {
    let mut positive = BTreeSet::new();
    let mut negative = BTreeSet::new();

    for fact in &state.known_facts {
        let key = base_key(&fact.atom.predicate, &fact.atom.args);
        match fact.atom.polarity {
            Polarity::Pos => {
                positive.insert(key);
            }
            Polarity::Neg => {
                negative.insert(key);
            }
        }
    }

    for fact in &state.derived_facts {
        positive.insert(base_key(&fact.atom.predicate, &fact.atom.args));
    }

    positive
        .intersection(&negative)
        .cloned()
        .collect::<Vec<_>>()
}

fn contradiction_for_target(
    state: &ProblemState,
    call_id: &str,
    target: &str,
) -> Option<ContradictionRecord> {
    let positive_ref = positive_refs_for_target(state, target).into_iter().next()?;
    let negative_ref = negative_refs_for_target(state, target).into_iter().next()?;

    Some(ContradictionRecord {
        contradiction_id: format!("{call_id}::contradiction::{target}"),
        positive_ref,
        negative_ref,
        scope: target.to_string(),
        status: ContradictionStatus::Active,
    })
}

fn positive_refs_for_target(state: &ProblemState, target: &str) -> Vec<String> {
    let mut refs = state
        .known_facts
        .iter()
        .filter(|fact| {
            fact.atom.polarity == Polarity::Pos
                && base_key(&fact.atom.predicate, &fact.atom.args) == target
        })
        .flat_map(|fact| {
            if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }
        })
        .collect::<Vec<_>>();

    refs.extend(
        state
            .derived_facts
            .iter()
            .filter(|fact| base_key(&fact.atom.predicate, &fact.atom.args) == target)
            .map(|fact| fact.derivation_id.clone()),
    );
    refs.sort();
    refs
}

fn negative_refs_for_target(state: &ProblemState, target: &str) -> Vec<String> {
    let mut refs = state
        .known_facts
        .iter()
        .filter(|fact| {
            fact.atom.polarity == Polarity::Neg
                && base_key(&fact.atom.predicate, &fact.atom.args) == target
        })
        .flat_map(|fact| {
            if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }
        })
        .collect::<Vec<_>>();
    refs.sort();
    refs
}

fn derivation_inputs(derived_facts: &[DerivedPositiveFact]) -> Vec<String> {
    let mut inputs = derived_facts
        .iter()
        .flat_map(|fact| fact.input_refs.clone())
        .collect::<Vec<_>>();
    inputs.sort();
    inputs.dedup();
    inputs
}

fn positive_support_available(state: &ProblemState, atom: &PositiveAtom) -> bool {
    state.known_facts.iter().any(|fact| {
        fact.atom.polarity == Polarity::Pos
            && fact.atom.predicate == atom.predicate
            && fact.atom.args == atom.args
    }) || state.derived_facts.iter().any(|fact| fact.atom == *atom)
}

fn positive_support_refs(state: &ProblemState, atom: &PositiveAtom) -> Vec<String> {
    let mut refs = state
        .known_facts
        .iter()
        .filter(|fact| {
            fact.atom.polarity == Polarity::Pos
                && fact.atom.predicate == atom.predicate
                && fact.atom.args == atom.args
        })
        .flat_map(|fact| {
            if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }
        })
        .collect::<Vec<_>>();

    refs.extend(
        state
            .derived_facts
            .iter()
            .filter(|fact| fact.atom == *atom)
            .map(|fact| fact.derivation_id.clone()),
    );
    refs.sort();
    refs
}

fn positive_atom_is_contested(state: &ProblemState, atom: &PositiveAtom) -> bool {
    state.known_facts.iter().any(|fact| {
        fact.atom.polarity == Polarity::Neg
            && fact.atom.predicate == atom.predicate
            && fact.atom.args == atom.args
            && fact.source == FactSource::Input
    })
}

fn unify_rule_consequent_to_query(
    rule: &PositiveRule,
    query: &SignedGroundAtom,
    domain: &BTreeSet<String>,
) -> Option<BTreeMap<String, String>> {
    if rule.consequent.predicate != query.predicate
        || rule.consequent.args.len() != query.args.len()
    {
        return None;
    }

    let mut bindings = BTreeMap::new();
    for (pattern_arg, query_arg) in rule.consequent.args.iter().zip(&query.args) {
        if domain.contains(pattern_arg) {
            if pattern_arg != query_arg {
                return None;
            }
        } else if let Some(existing) = bindings.get(pattern_arg) {
            if existing != query_arg {
                return None;
            }
        } else {
            bindings.insert(pattern_arg.clone(), query_arg.clone());
        }
    }
    Some(bindings)
}

fn rule_variables(rule: &PositiveRule, domain: &BTreeSet<String>) -> Option<Vec<String>> {
    let antecedent_vars = rule
        .antecedents
        .iter()
        .flat_map(|atom| atom.args.iter())
        .filter(|arg| !domain.contains(*arg))
        .cloned()
        .collect::<BTreeSet<_>>();
    let consequent_vars = rule
        .consequent
        .args
        .iter()
        .filter(|arg| !domain.contains(*arg))
        .cloned()
        .collect::<BTreeSet<_>>();

    if !consequent_vars.is_subset(&antecedent_vars) {
        return None;
    }

    Some(antecedent_vars.into_iter().collect())
}

fn enumerate_substitutions(
    variables: &[String],
    constants: &[String],
) -> Vec<BTreeMap<String, String>> {
    if variables.is_empty() {
        return vec![BTreeMap::new()];
    }

    let mut substitutions = vec![BTreeMap::new()];
    for variable in variables {
        let mut next = Vec::new();
        for existing in &substitutions {
            for constant in constants {
                let mut bound = existing.clone();
                bound.insert(variable.clone(), constant.clone());
                next.push(bound);
            }
        }
        substitutions = next;
    }
    substitutions
}

fn ground_positive_atom(
    atom: &PositiveAtom,
    bindings: &BTreeMap<String, String>,
    domain: &BTreeSet<String>,
) -> Option<PositiveAtom> {
    let mut grounded_args = Vec::with_capacity(atom.args.len());
    for arg in &atom.args {
        if domain.contains(arg) {
            grounded_args.push(arg.clone());
        } else {
            grounded_args.push(bindings.get(arg)?.clone());
        }
    }
    Some(PositiveAtom {
        predicate: atom.predicate.clone(),
        args: grounded_args,
    })
}

fn domain_set(state: &ProblemState) -> BTreeSet<String> {
    state.domain_constants.iter().cloned().collect()
}

fn base_key(predicate: &str, args: &[String]) -> String {
    format!("{predicate}({})", args.join(","))
}

fn render_positive_atoms(atoms: &[PositiveAtom]) -> String {
    atoms
        .iter()
        .map(render_positive_atom)
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn render_positive_atom(atom: &PositiveAtom) -> String {
    format!("{}({})", atom.predicate, atom.args.join(","))
}

fn render_signed_atom(atom: &SignedGroundAtom) -> String {
    let core = format!("{}({})", atom.predicate, atom.args.join(","));
    if atom.polarity == Polarity::Neg {
        format!("not {core}")
    } else {
        core
    }
}
