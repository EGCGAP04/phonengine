//! Error types for phonological rule construction and execution.

use std::error::Error;
use std::fmt;

use crate::phonology::FeatureBundle;

use super::rule::ApplicationMode;

/// Validation error produced when constructing a [`Rule`](crate::engine::Rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// Rule identifier is empty or whitespace-only.
    EmptyId,
    /// Rule inserts segments (`target` is empty) with both `left_context` and `right_context` empty.
    UnconstrainedInsertion {
        /// Identifier of the invalid rule.
        rule_id: String,
    },
    /// A [`ReplacementItem::Mutate`](crate::engine::ReplacementItem::Mutate) references an
    /// out-of-range `source_index`.
    InvalidSourceIndex {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Out-of-bounds index referenced by the mutation.
        source_index: usize,
        /// Length of the rule's `target` pattern vector.
        target_len: usize,
    },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("rule id must not be empty"),
            Self::UnconstrainedInsertion { rule_id } => write!(
                f,
                "rule {rule_id:?} is an unconstrained insertion: empty target requires a left or right context"
            ),
            Self::InvalidSourceIndex {
                rule_id,
                source_index,
                target_len,
            } => write!(
                f,
                "rule {rule_id:?} references source_index {source_index} but target has length {target_len}"
            ),
        }
    }
}

impl Error for RuleError {}

/// Runtime error produced when applying rules to a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A feature mutation produced a [`FeatureBundle`] absent from the [`Inventory`](crate::phonology::Inventory).
    UnmappedFeatureBundle(FeatureBundle),
    /// A literal replacement symbol is not registered in the [`Inventory`](crate::phonology::Inventory).
    UnknownSymbol(String),
    /// The rule requests an [`ApplicationMode`] not yet supported in Milestone 1.
    UnsupportedMode(ApplicationMode),
    /// The rule uses a non-local context element (such as `ZeroOrMore`) reserved for Milestone 3.
    UnsupportedContext {
        /// Identifier of the rule containing the unsupported context.
        rule_id: String,
    },
    /// Defensive error if rule invariants are violated during evaluation.
    InvalidRule(RuleError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmappedFeatureBundle(bundle) => {
                write!(
                    f,
                    "mutated feature bundle {bundle} is not mapped in inventory"
                )
            }
            Self::UnknownSymbol(sym) => {
                write!(
                    f,
                    "literal replacement symbol {sym:?} is not registered in inventory"
                )
            }
            Self::UnsupportedMode(mode) => {
                write!(f, "application mode {mode} is not supported in Milestone 1")
            }
            Self::UnsupportedContext { rule_id } => {
                write!(
                    f,
                    "rule {rule_id:?} uses a non-local context unsupported in Milestone 1"
                )
            }
            Self::InvalidRule(err) => write!(f, "invalid rule: {err}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRule(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{Manner, Place};

    #[test]
    fn rule_error_variants_implement_display_and_error() {
        let errors = [
            RuleError::EmptyId,
            RuleError::UnconstrainedInsertion {
                rule_id: "R_INS".to_owned(),
            },
            RuleError::InvalidSourceIndex {
                rule_id: "R_IDX".to_owned(),
                source_index: 2,
                target_len: 1,
            },
        ];
        for err in errors {
            assert!(!err.to_string().is_empty());
            let as_dyn: &dyn Error = &err;
            assert!(as_dyn.source().is_none());
        }
    }

    #[test]
    fn engine_error_variants_implement_display_and_error() {
        let bundle = FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true);
        let errors = [
            EngineError::UnmappedFeatureBundle(bundle),
            EngineError::UnknownSymbol("z".to_owned()),
            EngineError::UnsupportedMode(ApplicationMode::LeftToRight),
            EngineError::UnsupportedContext {
                rule_id: "R_CTX".to_owned(),
            },
            EngineError::InvalidRule(RuleError::EmptyId),
        ];
        for err in &errors {
            assert!(!err.to_string().is_empty());
        }
        let wrapped = &errors[4];
        let as_dyn: &dyn Error = wrapped;
        assert!(as_dyn.source().is_some());
    }
}
