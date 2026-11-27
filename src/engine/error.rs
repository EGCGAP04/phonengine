//! Error types for phonological rule construction, execution, and dialect trees.

use std::error::Error;
use std::fmt;

use crate::phonology::{FeatureBundle, FeatureDomain, FeatureField};

/// Validation error produced when constructing a [`Rule`](crate::engine::Rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// Rule identifier is empty or whitespace-only.
    EmptyId,
    /// Rule inserts segments (`target` is empty) without an anchoring context: neither
    /// `left_context` nor `right_context` contains a `WordBoundary` or a `Segment` element
    /// (contexts made only of `ZeroOrMore` match at every gap).
    UnconstrainedInsertion {
        /// Identifier of the invalid rule.
        rule_id: String,
    },
    /// A `WordBoundary` (`#`) is not the outermost element of its context: it must be the first
    /// element of `left_context` or the last element of `right_context`, at most once per side.
    MisplacedWordBoundary {
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
    /// An alpha variable has an empty or whitespace-only name.
    InvalidVariableName {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Invalid variable name string.
        var: String,
    },
    /// Negated variable polarity (`-α`) used on a non-boolean phonological field.
    InvalidVariablePolarity {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Alpha variable identifier.
        var: String,
        /// Non-boolean field where opposite polarity was attempted.
        field: FeatureField,
    },
    /// Same alpha variable name used across incompatible [`FeatureDomain`]s within one rule.
    IncompatibleVariableDomain {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Alpha variable identifier.
        var: String,
        /// First domain encountered for `var`.
        first: FeatureDomain,
        /// Conflicting domain encountered later for `var`.
        second: FeatureDomain,
    },
    /// Same feature or stress field constrained or mutated more than once on one segment spec.
    ConflictingFeatureConstraint {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Conflicting phonological or prosodic field.
        field: FeatureField,
    },
    /// Replacement references an alpha variable that is not guaranteed to be bound before
    /// replacement evaluation (must appear in `target` or as a fixed `ContextElement::Segment`
    /// in every branch of `environments`).
    UnboundVariable {
        /// Identifier of the invalid rule.
        rule_id: String,
        /// Unbound alpha variable identifier.
        var: String,
    },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("rule id must not be empty"),
            Self::UnconstrainedInsertion { rule_id } => write!(
                f,
                "rule {rule_id:?} is an unconstrained insertion: empty target requires a context with a '#' or a segment"
            ),
            Self::MisplacedWordBoundary { rule_id } => write!(
                f,
                "rule {rule_id:?} has a misplaced word boundary: '#' must be the first element of the left context or the last of the right context"
            ),
            Self::InvalidSourceIndex {
                rule_id,
                source_index,
                target_len,
            } => write!(
                f,
                "rule {rule_id:?} references source_index {source_index} but target has length {target_len}"
            ),
            Self::InvalidVariableName { rule_id, var } => {
                write!(
                    f,
                    "rule {rule_id:?} has an invalid variable name {var:?}: variable names must not be empty"
                )
            }
            Self::InvalidVariablePolarity {
                rule_id,
                var,
                field,
            } => write!(
                f,
                "rule {rule_id:?} negates non-boolean field {field:?} using variable '-{var}'"
            ),
            Self::IncompatibleVariableDomain {
                rule_id,
                var,
                first,
                second,
            } => write!(
                f,
                "rule {rule_id:?} uses variable {var:?} in incompatible domains ({first} vs {second})"
            ),
            Self::ConflictingFeatureConstraint { rule_id, field } => write!(
                f,
                "rule {rule_id:?} constrains or mutates feature {field:?} more than once on the same segment"
            ),
            Self::UnboundVariable { rule_id, var } => write!(
                f,
                "rule {rule_id:?} replacement references variable {var:?}, which is not guaranteed to be bound by target or every context branch"
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
    /// Two rules share the same identifier within one dialect stage.
    DuplicateRuleId {
        /// Stage containing the duplicated rule identifier.
        stage_id: String,
        /// Duplicated rule identifier.
        rule_id: String,
    },
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
            Self::DuplicateRuleId { stage_id, rule_id } => write!(
                f,
                "duplicate rule id {rule_id:?} in dialect stage {stage_id:?}"
            ),
        }
    }
}

impl Error for DialectError {}

/// Runtime error produced when applying rules or deriving across a dialect tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A feature mutation produced a [`FeatureBundle`] absent from the [`Inventory`](crate::phonology::Inventory).
    UnmappedFeatureBundle {
        /// Identifier of the rule whose mutation failed.
        rule_id: String,
        /// Mutated bundle with no symbol in the inventory.
        bundle: FeatureBundle,
    },
    /// A literal replacement symbol is not registered in the [`Inventory`](crate::phonology::Inventory).
    UnknownSymbol {
        /// Identifier of the rule whose literal replacement failed.
        rule_id: String,
        /// Unregistered literal symbol.
        symbol: String,
    },
    /// Requested stage identifier does not exist in the [`DialectTree`](crate::engine::DialectTree).
    UnknownStage(String),
    /// Defensive error if rule invariants are violated during evaluation.
    InvalidRule(RuleError),
    /// An error raised while evaluating the rules of a dialect stage.
    StageFailed {
        /// Identifier of the stage being evaluated.
        stage_id: String,
        /// Underlying error raised by the stage's rules.
        source: Box<EngineError>,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmappedFeatureBundle { rule_id, bundle } => write!(
                f,
                "rule {rule_id:?}: mutated feature bundle {bundle} is not mapped in inventory"
            ),
            Self::UnknownSymbol { rule_id, symbol } => write!(
                f,
                "rule {rule_id:?}: literal replacement symbol {symbol:?} is not registered in inventory"
            ),
            Self::UnknownStage(stage) => write!(f, "unknown dialect stage {stage:?}"),
            Self::InvalidRule(err) => write!(f, "invalid rule: {err}"),
            Self::StageFailed { stage_id, source } => write!(f, "stage {stage_id:?}: {source}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRule(err) => Some(err),
            Self::StageFailed { source, .. } => Some(source.as_ref()),
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
            RuleError::MisplacedWordBoundary {
                rule_id: "R_HASH".to_owned(),
            },
            RuleError::InvalidSourceIndex {
                rule_id: "R_IDX".to_owned(),
                source_index: 2,
                target_len: 1,
            },
            RuleError::InvalidVariableName {
                rule_id: "R_VAR".to_owned(),
                var: String::new(),
            },
            RuleError::InvalidVariablePolarity {
                rule_id: "R_VAR".to_owned(),
                var: "α".to_owned(),
                field: FeatureField::Place,
            },
            RuleError::IncompatibleVariableDomain {
                rule_id: "R_VAR".to_owned(),
                var: "α".to_owned(),
                first: FeatureDomain::Place,
                second: FeatureDomain::Bool,
            },
            RuleError::ConflictingFeatureConstraint {
                rule_id: "R_VAR".to_owned(),
                field: FeatureField::Voiced,
            },
            RuleError::UnboundVariable {
                rule_id: "R_VAR".to_owned(),
                var: "α".to_owned(),
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
            DialectError::DuplicateRuleId {
                stage_id: "attic".to_owned(),
                rule_id: "a_raising".to_owned(),
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
            EngineError::UnmappedFeatureBundle {
                rule_id: "R_MUT".to_owned(),
                bundle,
            },
            EngineError::UnknownSymbol {
                rule_id: "R_LIT".to_owned(),
                symbol: "z".to_owned(),
            },
            EngineError::UnknownStage("dorian".to_owned()),
            EngineError::InvalidRule(RuleError::EmptyId),
        ];
        for err in &errors {
            assert!(!err.to_string().is_empty());
        }
        assert!(errors[0].to_string().starts_with("rule \"R_MUT\": "));
        assert!(errors[1].to_string().starts_with("rule \"R_LIT\": "));
        let as_dyn: &dyn Error = &errors[3];
        assert!(as_dyn.source().is_some());
    }

    #[test]
    fn stage_failed_chains_display_and_exposes_source() {
        let inner = EngineError::UnknownSymbol {
            rule_id: "a_raising".to_owned(),
            symbol: "eː".to_owned(),
        };
        let err = EngineError::StageFailed {
            stage_id: "attic".to_owned(),
            source: Box::new(inner.clone()),
        };
        assert_eq!(
            err.to_string(),
            "stage \"attic\": rule \"a_raising\": literal replacement symbol \"eː\" is not registered in inventory"
        );
        let as_dyn: &dyn Error = &err;
        assert_eq!(
            as_dyn.source().map(ToString::to_string),
            Some(inner.to_string())
        );
    }
}
