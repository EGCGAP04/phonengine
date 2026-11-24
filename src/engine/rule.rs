//! Phonological rule AST, natural classes, contexts, and validation invariants.

use std::collections::BTreeSet;
use std::fmt;

use crate::phonology::{FeatureMutation, FeaturePattern, Segment};

use super::error::RuleError;

/// Named set of segment symbols forming a phonological natural class.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NaturalClass {
    /// Class identifier (e.g., `"V"` for vowels).
    pub name: String,
    /// Deterministic ordered set of member segment symbols.
    pub members: BTreeSet<String>,
}

impl NaturalClass {
    /// Creates a [`NaturalClass`] from a name and an iterable of member symbols.
    #[must_use]
    pub fn new<I, S>(name: impl Into<String>, members: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            members: members.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns `true` if `segment` belongs to this natural class.
    #[must_use]
    pub fn matches(&self, segment: &Segment) -> bool {
        self.members.contains(&segment.symbol)
    }
}

/// Segment pattern usable inside rule targets and local contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// Exact match on `segment.symbol`.
    Literal(String),
    /// Feature-based match on `segment.bundle`.
    Features(FeaturePattern),
    /// Membership check in a [`NaturalClass`].
    Class(NaturalClass),
}

impl Pattern {
    /// Returns `true` if `segment` matches this pattern.
    #[must_use]
    pub fn matches(&self, segment: &Segment) -> bool {
        match self {
            Self::Literal(symbol) => segment.symbol == *symbol,
            Self::Features(pattern) => pattern.matches(&segment.bundle),
            Self::Class(class) => class.matches(segment),
        }
    }
}

/// Single output element produced when a rule match is rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementItem {
    /// Emits an unstressed segment resolved by literal symbol from the inventory.
    Literal(String),
    /// Mutates the matched input segment at `source_index` while preserving its stress.
    Mutate {
        /// Zero-based index into the matched `target` slice.
        source_index: usize,
        /// Feature mutation applied to the source segment's [`FeatureBundle`](crate::phonology::FeatureBundle).
        mutation: FeatureMutation,
    },
}

/// Element of a rule's left or right conditioning environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextElement {
    /// Word boundary (`#`).
    WordBoundary,
    /// Single segment matching `Pattern`.
    Segment(Pattern),
    /// Non-local Kleene closure (`Pattern*`), reserved for Milestone 3.
    ZeroOrMore(Pattern),
}

/// Evaluation order strategy for rule application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ApplicationMode {
    /// Evaluates all non-overlapping matches against an immutable snapshot of the input word.
    #[default]
    Simultaneous,
    /// Iterative left-to-right application (deferred to Milestone 2/3).
    LeftToRight,
    /// Iterative right-to-left application (deferred to Milestone 2/3).
    RightToLeft,
}

impl fmt::Display for ApplicationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Simultaneous => "simultaneous",
            Self::LeftToRight => "left-to-right",
            Self::RightToLeft => "right-to-left",
        };
        f.write_str(label)
    }
}

/// A validated phonological sound-change rule supporting general `N -> M` rewrites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    id: String,
    name: String,
    target: Vec<Pattern>,
    replacement: Vec<ReplacementItem>,
    left_context: Vec<ContextElement>,
    right_context: Vec<ContextElement>,
    mode: ApplicationMode,
}

impl Rule {
    /// Constructs and validates a phonological [`Rule`].
    ///
    /// # Errors
    ///
    /// - [`RuleError::EmptyId`] if `id` is empty or whitespace-only.
    /// - [`RuleError::UnconstrainedInsertion`] if `target`, `left_context`, and `right_context` are all empty.
    /// - [`RuleError::InvalidSourceIndex`] if any [`ReplacementItem::Mutate`] has `source_index >= target.len()`.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        target: Vec<Pattern>,
        replacement: Vec<ReplacementItem>,
        left_context: Vec<ContextElement>,
        right_context: Vec<ContextElement>,
        mode: ApplicationMode,
    ) -> Result<Self, RuleError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(RuleError::EmptyId);
        }
        if target.is_empty() && left_context.is_empty() && right_context.is_empty() {
            return Err(RuleError::UnconstrainedInsertion { rule_id: id });
        }
        for item in &replacement {
            if let ReplacementItem::Mutate { source_index, .. } = item
                && *source_index >= target.len()
            {
                return Err(RuleError::InvalidSourceIndex {
                    rule_id: id,
                    source_index: *source_index,
                    target_len: target.len(),
                });
            }
        }

        Ok(Self {
            id,
            name: name.into(),
            target,
            replacement,
            left_context,
            right_context,
            mode,
        })
    }

    /// Returns the rule's unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the human-readable rule name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the target pattern sequence.
    #[must_use]
    pub fn target(&self) -> &[Pattern] {
        &self.target
    }

    /// Returns the replacement item sequence.
    #[must_use]
    pub fn replacement(&self) -> &[ReplacementItem] {
        &self.replacement
    }

    /// Returns the left conditioning context (ordered left-to-right).
    #[must_use]
    pub fn left_context(&self) -> &[ContextElement] {
        &self.left_context
    }

    /// Returns the right conditioning context (ordered left-to-right).
    #[must_use]
    pub fn right_context(&self) -> &[ContextElement] {
        &self.right_context
    }

    /// Returns the rule's [`ApplicationMode`].
    #[must_use]
    pub const fn mode(&self) -> ApplicationMode {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{Backness, FeatureBundle, Height, Manner, Place};

    #[test]
    fn rule_rejects_empty_id() {
        let err = Rule::new(
            "   ",
            "Empty ID",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(err, Err(RuleError::EmptyId));
    }

    #[test]
    fn rule_rejects_unconstrained_insertion() {
        let err = Rule::new(
            "R_INS",
            "Unconstrained epenthesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err,
            Err(RuleError::UnconstrainedInsertion {
                rule_id: "R_INS".to_owned()
            })
        );
    }

    #[test]
    fn rule_allows_constrained_insertion_with_either_context() -> Result<(), RuleError> {
        let left_only = Rule::new(
            "R_INS_L",
            "Post-rhotic epenthesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(left_only.id(), "R_INS_L");

        let right_only = Rule::new(
            "R_INS_R",
            "Prothesis",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("s".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(right_only.id(), "R_INS_R");
        Ok(())
    }

    #[test]
    fn rule_rejects_invalid_source_index() {
        let err = Rule::new(
            "R_BAD_IDX",
            "Out-of-bounds mutation",
            vec![Pattern::Literal("p".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 1,
                mutation: FeatureMutation::new().set_voiced(true),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err,
            Err(RuleError::InvalidSourceIndex {
                rule_id: "R_BAD_IDX".to_owned(),
                source_index: 1,
                target_len: 1,
            })
        );

        let err_empty_target = Rule::new(
            "R_BAD_INS",
            "Mutation on empty target",
            vec![],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new(),
            }],
            vec![ContextElement::WordBoundary],
            vec![],
            ApplicationMode::Simultaneous,
        );
        assert_eq!(
            err_empty_target,
            Err(RuleError::InvalidSourceIndex {
                rule_id: "R_BAD_INS".to_owned(),
                source_index: 0,
                target_len: 0,
            })
        );
    }

    #[test]
    fn pattern_matches_literal_features_and_natural_class() {
        let a = Segment::unstressed(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        );
        let p = Segment::unstressed(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        );
        let vowels = NaturalClass::new("V", ["a", "e", "i", "o", "u"]);

        assert!(Pattern::Literal("a".to_owned()).matches(&a));
        assert!(!Pattern::Literal("a".to_owned()).matches(&p));

        let stop = Pattern::Features(FeaturePattern::new().with_manner(Manner::Stop));
        assert!(stop.matches(&p));
        assert!(!stop.matches(&a));

        let vowel_class = Pattern::Class(vowels);
        assert!(vowel_class.matches(&a));
        assert!(!vowel_class.matches(&p));
    }
}
