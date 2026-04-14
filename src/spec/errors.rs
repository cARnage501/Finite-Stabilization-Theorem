use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("duplicate domain constant `{0}`")]
    DuplicateDomainConstant(String),
    #[error("invalid budget: all budget values must be positive")]
    InvalidBudget,
    #[error("empty predicate is not allowed")]
    EmptyPredicate,
    #[error("empty identifier is not allowed")]
    EmptyIdentifier,
    #[error("symbol `{symbol}` references constant `{constant}` outside the declared universe")]
    ConstantOutsideUniverse { symbol: String, constant: String },
    #[error("predicate `{predicate}` has inconsistent arity: expected {expected}, found {found}")]
    InconsistentArity {
        predicate: String,
        expected: usize,
        found: usize,
    },
    #[error("rule `{0}` must contain at least one antecedent")]
    EmptyAntecedents(String),
    #[error("negative support may only come from signed input facts")]
    NegativeSupportMustComeFromInput,
}
