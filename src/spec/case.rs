use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::errors::ValidationError;
use crate::spec::terminal::{FinalLabel, TerminalFailureLocus};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemanticsProfile {
    Classical,
    Paraconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Handwritten,
    Synthetic,
    Adversarial,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaseMetadata {
    pub difficulty: Difficulty,
    pub contradiction_present: bool,
    pub source_type: SourceType,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    Pos,
    Neg,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PositiveAtom {
    pub predicate: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SignedGroundAtom {
    pub predicate: String,
    pub args: Vec<String>,
    pub polarity: Polarity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SignedGroundFact {
    pub predicate: String,
    pub args: Vec<String>,
    pub polarity: Polarity,
    pub fact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PositiveRule {
    pub antecedents: Vec<PositiveAtom>,
    pub consequent: PositiveAtom,
    pub rule_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BudgetContract {
    pub max_steps: u32,
    pub max_agent_calls: u32,
    pub max_iterations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GoldCaseAnnotation {
    pub case_id: String,
    pub expected_label: FinalLabel,
    pub gold_failure_locus: Option<TerminalFailureLocus>,
    pub decisive_support_refs: Option<Vec<String>>,
    pub blocking_condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaseSpec {
    pub case_id: String,
    pub domain_version: String,
    pub semantics_profile: SemanticsProfile,
    pub closed_world: bool,
    pub domain_constants: Vec<String>,
    pub facts: Vec<SignedGroundFact>,
    pub rules: Vec<PositiveRule>,
    pub query: SignedGroundAtom,
    pub budget: BudgetContract,
    pub expected_label: FinalLabel,
    pub gold_failure_locus: Option<TerminalFailureLocus>,
    pub metadata: CaseMetadata,
}

impl CaseSpec {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        let mut domain_seen = BTreeSet::new();
        for constant in &self.domain_constants {
            if !domain_seen.insert(constant) {
                errors.push(ValidationError::DuplicateDomainConstant(constant.clone()));
            }
        }

        if self.budget.max_steps == 0
            || self.budget.max_agent_calls == 0
            || self.budget.max_iterations == 0
        {
            errors.push(ValidationError::InvalidBudget);
        }

        let mut arities = BTreeMap::new();

        for fact in &self.facts {
            validate_identifier(&fact.predicate, &fact.fact_id, &mut errors);
            validate_ground_args(&fact.predicate, &fact.args, &domain_seen, &mut errors);
            check_arity(&fact.predicate, fact.args.len(), &mut arities, &mut errors);
        }

        validate_identifier(&self.query.predicate, &self.case_id, &mut errors);
        validate_ground_args(
            &self.query.predicate,
            &self.query.args,
            &domain_seen,
            &mut errors,
        );
        check_arity(
            &self.query.predicate,
            self.query.args.len(),
            &mut arities,
            &mut errors,
        );

        for rule in &self.rules {
            if rule.rule_id.trim().is_empty() {
                errors.push(ValidationError::EmptyIdentifier);
            }
            if rule.antecedents.is_empty() {
                errors.push(ValidationError::EmptyAntecedents(rule.rule_id.clone()));
            }
            for atom in &rule.antecedents {
                validate_positive_atom(atom, &domain_seen, &mut arities, &mut errors);
            }
            validate_positive_atom(&rule.consequent, &domain_seen, &mut arities, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validate_positive_atom(
    atom: &PositiveAtom,
    domain_seen: &BTreeSet<&String>,
    arities: &mut BTreeMap<String, usize>,
    errors: &mut Vec<ValidationError>,
) {
    if atom.predicate.trim().is_empty() {
        errors.push(ValidationError::EmptyPredicate);
    }
    check_arity(&atom.predicate, atom.args.len(), arities, errors);
    for arg in &atom.args {
        if domain_seen.contains(arg) || is_rule_variable(arg) {
            continue;
        }
        errors.push(ValidationError::ConstantOutsideUniverse {
            symbol: atom.predicate.clone(),
            constant: arg.clone(),
        });
    }
}

fn validate_identifier(predicate: &str, identifier: &str, errors: &mut Vec<ValidationError>) {
    if predicate.trim().is_empty() {
        errors.push(ValidationError::EmptyPredicate);
    }
    if identifier.trim().is_empty() {
        errors.push(ValidationError::EmptyIdentifier);
    }
}

fn validate_ground_args(
    symbol: &str,
    args: &[String],
    domain_seen: &BTreeSet<&String>,
    errors: &mut Vec<ValidationError>,
) {
    for arg in args {
        if !domain_seen.contains(arg) {
            errors.push(ValidationError::ConstantOutsideUniverse {
                symbol: symbol.to_string(),
                constant: arg.clone(),
            });
        }
    }
}

fn check_arity(
    predicate: &str,
    arity: usize,
    arities: &mut BTreeMap<String, usize>,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(expected) = arities.get(predicate) {
        if *expected != arity {
            errors.push(ValidationError::InconsistentArity {
                predicate: predicate.to_string(),
                expected: *expected,
                found: arity,
            });
        }
    } else {
        arities.insert(predicate.to_string(), arity);
    }
}

fn is_rule_variable(symbol: &str) -> bool {
    matches!(symbol, "x" | "y" | "z" | "u" | "v" | "w") || symbol.starts_with('?')
}
