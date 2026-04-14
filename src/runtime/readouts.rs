use std::collections::{BTreeMap, BTreeSet};

use crate::spec::{
    AggregateFlags, LatticeReadoutPacket, Polarity, PositiveAtom, PositiveRule, ProblemState,
    Readout, SignedGroundAtom,
};
use serde_json::json;

pub struct ReadoutEngine;

impl ReadoutEngine {
    pub fn evaluate(state: &ProblemState) -> LatticeReadoutPacket {
        let coverage_gap = compute_coverage_gap(state);
        let collision_pressure = compute_collision_pressure(state);
        let trigger_illegality = compute_trigger_illegality(state);
        let role_confusion = compute_role_confusion(state);
        let structurally_incomplete = coverage_gap.threshold_exceeded;
        let structurally_contested = collision_pressure.threshold_exceeded;
        let structurally_malformed =
            trigger_illegality.threshold_exceeded || role_confusion.threshold_exceeded;

        LatticeReadoutPacket {
            packet_id: format!("{}::lattice-readouts", state.state_id),
            state_ref: state.state_id.clone(),
            coverage_gap,
            collision_pressure,
            trigger_illegality,
            role_confusion,
            aggregate_flags: AggregateFlags {
                structurally_incomplete,
                structurally_contested,
                structurally_malformed,
            },
            emitted_at: format!("{}::readouts", state.state_id),
        }
    }
}

fn compute_coverage_gap(state: &ProblemState) -> Readout {
    let domain = domain_set(state);
    let mut candidate_rules = Vec::new();
    let mut missing_atoms = Vec::<String>::new();
    let mut supporting_refs = Vec::new();

    if direct_query_support(state, &state.query) {
        supporting_refs.extend(positive_support_refs_for_query(state, &state.query));
    } else if matches!(state.query.polarity, Polarity::Pos) {
        for rule in &state.active_rules {
            if let Some(bindings) = unify_positive_atom(&rule.consequent, &state.query, &domain) {
                candidate_rules.push(rule.rule_id.clone());
                let mut branch_missing = Vec::new();
                if antecedents_have_support(rule, 0, &bindings, state, &domain, &mut branch_missing)
                {
                    supporting_refs.push(rule.rule_id.clone());
                    break;
                }
                if missing_atoms.is_empty() {
                    missing_atoms = branch_missing;
                }
            }
        }

        if missing_atoms.is_empty() {
            missing_atoms.push(render_signed_atom(&state.query));
        }
    } else {
        missing_atoms.push(render_signed_atom(&state.query));
    }

    let supported = !supporting_refs.is_empty() || direct_query_support(state, &state.query);
    let score = if supported { 0.0 } else { 1.0 };

    Readout {
        readout_id: format!("{}::coverage_gap", state.state_id),
        name: "coverage_gap".to_string(),
        input_refs: input_refs_for_coverage(state, &candidate_rules, &supporting_refs),
        computation_summary: if supported {
            "query has complete structural support".to_string()
        } else {
            "query is missing one or more structural prerequisites".to_string()
        },
        numeric_score: score,
        threshold_exceeded: score > 0.5,
        trace_fragment: if supported {
            "coverage gap remained below threshold".to_string()
        } else {
            "coverage gap exceeded threshold".to_string()
        },
        details: json!({
            "candidate_rules": candidate_rules,
            "missing_atoms": missing_atoms,
            "supported": supported,
        }),
    }
}

fn compute_collision_pressure(state: &ProblemState) -> Readout {
    let mut polarity_map: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();

    for fact in &state.known_facts {
        let entry = polarity_map.entry(base_key(&fact.atom.predicate, &fact.atom.args)).or_default();
        match fact.atom.polarity {
            Polarity::Pos => entry.0.extend(if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }),
            Polarity::Neg => entry.1.extend(if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }),
        }
    }

    for fact in &state.derived_facts {
        let entry = polarity_map.entry(base_key(&fact.atom.predicate, &fact.atom.args)).or_default();
        entry.0.push(fact.derivation_id.clone());
    }

    let contested_targets: Vec<String> = polarity_map
        .iter()
        .filter(|(_, (positive_refs, negative_refs))| {
            !positive_refs.is_empty() && !negative_refs.is_empty()
        })
        .map(|(atom, _)| atom.clone())
        .collect();

    let opposing_path_refs: Vec<String> = polarity_map
        .iter()
        .filter(|(_, (positive_refs, negative_refs))| {
            !positive_refs.is_empty() && !negative_refs.is_empty()
        })
        .flat_map(|(_, (positive_refs, negative_refs))| {
            positive_refs
                .iter()
                .cloned()
                .chain(negative_refs.iter().cloned())
        })
        .collect();

    let score = if contested_targets.is_empty() { 0.0 } else { 1.0 };

    Readout {
        readout_id: format!("{}::collision_pressure", state.state_id),
        name: "collision_pressure".to_string(),
        input_refs: opposing_path_refs.clone(),
        computation_summary: if contested_targets.is_empty() {
            "no active support/opposition collision detected".to_string()
        } else {
            "support and opposition are active for the same structural target".to_string()
        },
        numeric_score: score,
        threshold_exceeded: score > 0.5,
        trace_fragment: if contested_targets.is_empty() {
            "collision pressure remained below threshold".to_string()
        } else {
            "collision pressure exceeded threshold".to_string()
        },
        details: json!({
            "contested_targets": contested_targets,
            "opposing_path_refs": opposing_path_refs,
        }),
    }
}

fn compute_trigger_illegality(state: &ProblemState) -> Readout {
    let domain = domain_set(state);
    let mut illegal_terms = Vec::new();
    let mut blocked_agent_candidates = Vec::new();
    let mut underbound_variables = Vec::new();
    let mut input_refs = Vec::new();

    for fact in &state.known_facts {
        for arg in &fact.atom.args {
            if !domain.contains(arg) {
                illegal_terms.push(render_signed_atom(&fact.atom));
                input_refs.extend(if fact.support_refs.is_empty() {
                    vec![render_signed_atom(&fact.atom)]
                } else {
                    fact.support_refs.clone()
                });
                break;
            }
        }
    }

    if state
        .query
        .args
        .iter()
        .any(|arg| !domain.contains(arg))
    {
        illegal_terms.push(render_signed_atom(&state.query));
        input_refs.push(format!("{}::query", state.case_id));
    }

    for rule in &state.active_rules {
        let antecedent_variables = variables_in_rule_antecedents(rule, &domain);
        let consequent_variables = variables_in_rule_consequent(rule, &domain);
        let underbound: Vec<String> = consequent_variables
            .difference(&antecedent_variables)
            .cloned()
            .collect();

        if !underbound.is_empty() {
            blocked_agent_candidates.push(rule.rule_id.clone());
            underbound_variables.extend(underbound);
            input_refs.push(rule.rule_id.clone());
        }
    }

    let score = if illegal_terms.is_empty() && blocked_agent_candidates.is_empty() {
        0.0
    } else {
        1.0
    };

    Readout {
        readout_id: format!("{}::trigger_illegality", state.state_id),
        name: "trigger_illegality".to_string(),
        input_refs,
        computation_summary: if score == 0.0 {
            "no structural trigger illegality detected".to_string()
        } else {
            "candidate actions appear underbound or malformed".to_string()
        },
        numeric_score: score,
        threshold_exceeded: score > 0.5,
        trace_fragment: if score == 0.0 {
            "trigger illegality remained below threshold".to_string()
        } else {
            "trigger illegality exceeded threshold".to_string()
        },
        details: json!({
            "illegal_terms": illegal_terms,
            "blocked_agent_candidates": blocked_agent_candidates,
            "underbound_variables": underbound_variables,
        }),
    }
}

fn compute_role_confusion(state: &ProblemState) -> Readout {
    let mut arities: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();

    for fact in &state.known_facts {
        arities
            .entry(fact.atom.predicate.clone())
            .or_default()
            .insert(fact.atom.args.len());
    }

    for fact in &state.derived_facts {
        arities
            .entry(fact.atom.predicate.clone())
            .or_default()
            .insert(fact.atom.args.len());
    }

    arities
        .entry(state.query.predicate.clone())
        .or_default()
        .insert(state.query.args.len());

    for rule in &state.active_rules {
        for atom in &rule.antecedents {
            arities.entry(atom.predicate.clone()).or_default().insert(atom.args.len());
        }
        arities
            .entry(rule.consequent.predicate.clone())
            .or_default()
            .insert(rule.consequent.args.len());
    }

    let problematic_symbols: Vec<String> = arities
        .iter()
        .filter(|(_, observed_arities)| observed_arities.len() > 1)
        .map(|(predicate, _)| predicate.clone())
        .collect();

    let confusion_types: Vec<String> = problematic_symbols
        .iter()
        .map(|symbol| format!("predicate_arity_mismatch:{symbol}"))
        .collect();

    let score = if problematic_symbols.is_empty() { 0.0 } else { 1.0 };

    Readout {
        readout_id: format!("{}::role_confusion", state.state_id),
        name: "role_confusion".to_string(),
        input_refs: problematic_symbols.clone(),
        computation_summary: if problematic_symbols.is_empty() {
            "no symbol role confusion detected".to_string()
        } else {
            "predicate arity is inconsistent across the structural state".to_string()
        },
        numeric_score: score,
        threshold_exceeded: score > 0.5,
        trace_fragment: if problematic_symbols.is_empty() {
            "role confusion remained below threshold".to_string()
        } else {
            "role confusion exceeded threshold".to_string()
        },
        details: json!({
            "problematic_symbols": problematic_symbols,
            "confusion_types": confusion_types,
        }),
    }
}

fn positive_support_refs_for_query(state: &ProblemState, query: &SignedGroundAtom) -> Vec<String> {
    state
        .known_facts
        .iter()
        .filter(|fact| {
            fact.atom.polarity == Polarity::Pos
                && fact.atom.predicate == query.predicate
                && fact.atom.args == query.args
        })
        .flat_map(|fact| {
            if fact.support_refs.is_empty() {
                vec![render_signed_atom(&fact.atom)]
            } else {
                fact.support_refs.clone()
            }
        })
        .chain(
            state
                .derived_facts
                .iter()
                .filter(|fact| fact.atom.predicate == query.predicate && fact.atom.args == query.args)
                .map(|fact| fact.derivation_id.clone()),
        )
        .collect()
}

fn input_refs_for_coverage(
    state: &ProblemState,
    candidate_rules: &[String],
    supporting_refs: &[String],
) -> Vec<String> {
    let mut refs = Vec::with_capacity(1 + candidate_rules.len() + supporting_refs.len());
    refs.push(format!("{}::query", state.case_id));
    refs.extend(candidate_rules.iter().cloned());
    refs.extend(supporting_refs.iter().cloned());
    refs
}

fn antecedents_have_support(
    rule: &PositiveRule,
    antecedent_index: usize,
    bindings: &BTreeMap<String, String>,
    state: &ProblemState,
    domain: &BTreeSet<String>,
    missing_out: &mut Vec<String>,
) -> bool {
    if antecedent_index == rule.antecedents.len() {
        return true;
    }

    let antecedent = &rule.antecedents[antecedent_index];
    let groundings = groundings_for_atom(antecedent, bindings, domain);

    for grounding in &groundings {
        if supports_positive_atom(state, grounding) {
            let mut next_bindings = bindings.clone();
            bind_atom_to_ground(antecedent, grounding, &mut next_bindings, domain);
            if antecedents_have_support(
                rule,
                antecedent_index + 1,
                &next_bindings,
                state,
                domain,
                missing_out,
            ) {
                return true;
            }
        }
    }

    if let Some(first) = groundings.first() {
        missing_out.push(render_positive_atom(first));
    }
    false
}

fn supports_positive_atom(state: &ProblemState, atom: &PositiveAtom) -> bool {
    state.known_facts.iter().any(|fact| {
        fact.atom.polarity == Polarity::Pos
            && fact.atom.predicate == atom.predicate
            && fact.atom.args == atom.args
    }) || state.derived_facts.iter().any(|fact| {
        fact.atom.predicate == atom.predicate && fact.atom.args == atom.args
    })
}

fn unify_positive_atom(
    pattern: &PositiveAtom,
    ground: &SignedGroundAtom,
    domain: &BTreeSet<String>,
) -> Option<BTreeMap<String, String>> {
    if pattern.predicate != ground.predicate || pattern.args.len() != ground.args.len() {
        return None;
    }

    let mut bindings = BTreeMap::new();
    for (pattern_arg, ground_arg) in pattern.args.iter().zip(&ground.args) {
        if domain.contains(pattern_arg) {
            if pattern_arg != ground_arg {
                return None;
            }
        } else {
            match bindings.get(pattern_arg) {
                Some(existing) if existing != ground_arg => return None,
                _ => {
                    bindings.insert(pattern_arg.clone(), ground_arg.clone());
                }
            }
        }
    }

    Some(bindings)
}

fn groundings_for_atom(
    atom: &PositiveAtom,
    bindings: &BTreeMap<String, String>,
    domain: &BTreeSet<String>,
) -> Vec<PositiveAtom> {
    let mut outputs = Vec::new();
    let mut current = Vec::with_capacity(atom.args.len());
    build_groundings(atom, 0, bindings, domain, &mut current, &mut outputs);
    outputs
}

fn build_groundings(
    atom: &PositiveAtom,
    index: usize,
    bindings: &BTreeMap<String, String>,
    domain: &BTreeSet<String>,
    current: &mut Vec<String>,
    outputs: &mut Vec<PositiveAtom>,
) {
    if index == atom.args.len() {
        outputs.push(PositiveAtom {
            predicate: atom.predicate.clone(),
            args: current.clone(),
        });
        return;
    }

    let arg = &atom.args[index];
    if domain.contains(arg) {
        current.push(arg.clone());
        build_groundings(atom, index + 1, bindings, domain, current, outputs);
        current.pop();
        return;
    }

    if let Some(bound) = bindings.get(arg) {
        current.push(bound.clone());
        build_groundings(atom, index + 1, bindings, domain, current, outputs);
        current.pop();
        return;
    }

    for constant in domain {
        current.push(constant.clone());
        build_groundings(atom, index + 1, bindings, domain, current, outputs);
        current.pop();
    }
}

fn bind_atom_to_ground(
    pattern: &PositiveAtom,
    ground: &PositiveAtom,
    bindings: &mut BTreeMap<String, String>,
    domain: &BTreeSet<String>,
) {
    for (pattern_arg, ground_arg) in pattern.args.iter().zip(&ground.args) {
        if !domain.contains(pattern_arg) {
            bindings.insert(pattern_arg.clone(), ground_arg.clone());
        }
    }
}

fn variables_in_rule_antecedents(rule: &PositiveRule, domain: &BTreeSet<String>) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    for atom in &rule.antecedents {
        for arg in &atom.args {
            if !domain.contains(arg) {
                variables.insert(arg.clone());
            }
        }
    }
    variables
}

fn variables_in_rule_consequent(rule: &PositiveRule, domain: &BTreeSet<String>) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    for arg in &rule.consequent.args {
        if !domain.contains(arg) {
            variables.insert(arg.clone());
        }
    }
    variables
}

fn direct_query_support(state: &ProblemState, query: &SignedGroundAtom) -> bool {
    match query.polarity {
        Polarity::Pos => {
            !positive_support_refs_for_query(state, query).is_empty()
        }
        Polarity::Neg => state
            .known_facts
            .iter()
            .any(|fact| fact.atom == *query),
    }
}

fn domain_set(state: &ProblemState) -> BTreeSet<String> {
    state.domain_constants.iter().cloned().collect()
}

fn base_key(predicate: &str, args: &[String]) -> String {
    format!("{}({})", predicate, args.join(","))
}

fn render_signed_atom(atom: &SignedGroundAtom) -> String {
    match atom.polarity {
        Polarity::Pos => base_key(&atom.predicate, &atom.args),
        Polarity::Neg => format!("not {}", base_key(&atom.predicate, &atom.args)),
    }
}

fn render_positive_atom(atom: &PositiveAtom) -> String {
    base_key(&atom.predicate, &atom.args)
}
