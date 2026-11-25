//! Error types for phonological rule construction, execution, and dialect trees.

use std::error::Error;
use std::fmt;

use crate::phonology::FeatureBundle;

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

/// Validation error produced when constructing a [`DialectTree`](crate::engine::DialectTree).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialectError {
    /// Dialect tree contains no stages.
    EmptyTree,
    /// A dialect stage has an empty or whitespace-only identifier.
    EmptyStageId,
    /// Two dialect stages share the same identifier.
    DuplicateStageId(String),
    /// A dialect stage references a parent identifier that does not exist in the tree.
    UnknownParent {
        /// Stage whose `parent` reference is invalid.
        stage_id: String,
        /// Missing parent stage identifier.
        parent_id: String,
    },
    /// No root stage (`parent: None`) exists in the dialect tree.
    MissingRoot,
    /// More than one root stage (`parent: None`) exists in the dialect tree.
    MultipleRoots(Vec<String>),
    /// Parent links form a cycle that cannot be reached from the root stage.
    CycleDetected(Vec<String>),
}

impl fmt::Display for DialectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTree => f.write_str("dialect tree must contain at least one stage"),
            Self::EmptyStageId => f.write_str("dialect stage id must not be empty"),
            Self::DuplicateStageId(id) => write!(f, "duplicate dialect stage id {id:?}"),
            Self::UnknownParent {
                stage_id,
                parent_id,
            } => write!(
                f,
                "dialect stage {stage_id:?} references unknown parent stage {parent_id:?}"
            ),
            Self::MissingRoot => f.write_str("dialect tree has no root stage (parent: None)"),
            Self::MultipleRoots(roots) => {
                write!(
                    f,
                    "dialect tree has multiple root stages: {}",
                    roots.join(", ")
                )
            }
            Self::CycleDetected(cycle) => {
                write!(
                    f,
                    "dialect tree contains a parent cycle: {}",
                    cycle.join(" -> ")
                )
            }
        }
    }
}

impl Error for DialectError {}

/// Runtime error produced when applying rules or deriving across a dialect tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A feature mutation produced a [`FeatureBundle`] absent from the [`Inventory`](crate::phonology::Inventory).
    UnmappedFeatureBundle(FeatureBundle),
    /// A literal replacement symbol is not registered in the [`Inventory`](crate::phonology::Inventory).
    UnknownSymbol(String),
    /// Requested stage identifier does not exist in the [`DialectTree`](crate::engine::DialectTree).
    UnknownStage(String),
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
            Self::UnknownStage(stage) => write!(f, "unknown dialect stage {stage:?}"),
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
    fn dialect_error_variants_implement_display_and_error() {
        let errors = [
            DialectError::EmptyTree,
            DialectError::EmptyStageId,
            DialectError::DuplicateStageId("pie".to_owned()),
            DialectError::UnknownParent {
                stage_id: "attic".to_owned(),
                parent_id: "proto_greek".to_owned(),
            },
            DialectError::MissingRoot,
            DialectError::MultipleRoots(vec!["a".to_owned(), "b".to_owned()]),
            DialectError::CycleDetected(vec!["a".to_owned(), "b".to_owned(), "a".to_owned()]),
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
            EngineError::UnknownStage("dorian".to_owned()),
            EngineError::InvalidRule(RuleError::EmptyId),
        ];
        for err in &errors {
            assert!(!err.to_string().is_empty());
        }
        let wrapped = &errors[3];
        let as_dyn: &dyn Error = wrapped;
        assert!(as_dyn.source().is_some());
    }
}
