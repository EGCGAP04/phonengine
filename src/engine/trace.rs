//! Immutable derivation trace structures excluding vacuous matches.

use crate::phonology::Word;

/// A single effective rule firing where `word_after != word_before`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationStep {
    rule_id: String,
    rule_name: String,
    word_before: Word,
    word_after: Word,
}

impl DerivationStep {
    /// Creates a new [`DerivationStep`].
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        rule_name: impl Into<String>,
        word_before: Word,
        word_after: Word,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            rule_name: rule_name.into(),
            word_before,
            word_after,
        }
    }

    /// Returns the identifier of the rule that produced this change.
    #[must_use]
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the name of the rule that produced this change.
    #[must_use]
    pub fn rule_name(&self) -> &str {
        &self.rule_name
    }

    /// Returns the word state immediately before this step.
    #[must_use]
    pub fn word_before(&self) -> &Word {
        &self.word_before
    }

    /// Returns the word state immediately after this step.
    #[must_use]
    pub fn word_after(&self) -> &Word {
        &self.word_after
    }
}

/// Immutable record of a word's evolution through an ordered list of rules.
///
/// Only rules that actually modify the word (`word_after != word_before`) appear in
/// [`steps`](Self::steps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationTrace {
    initial: Word,
    steps: Vec<DerivationStep>,
    final_word: Word,
}

impl DerivationTrace {
    /// Creates a new [`DerivationTrace`].
    #[must_use]
    pub fn new(initial: Word, steps: Vec<DerivationStep>, final_word: Word) -> Self {
        Self {
            initial,
            steps,
            final_word,
        }
    }

    /// Returns the initial input word before any rule ran.
    #[must_use]
    pub fn initial(&self) -> &Word {
        &self.initial
    }

    /// Returns the ordered slice of non-vacuous derivation steps.
    #[must_use]
    pub fn steps(&self) -> &[DerivationStep] {
        &self.steps
    }

    /// Returns the final word after all rules were evaluated.
    #[must_use]
    pub fn final_word(&self) -> &Word {
        &self.final_word
    }

    /// Returns the ordered list of rule IDs that produced a change.
    #[must_use]
    pub fn fired_rule_ids(&self) -> Vec<&str> {
        self.steps.iter().map(DerivationStep::rule_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{Backness, FeatureBundle, Height, Segment};

    #[test]
    fn derivation_trace_exposes_getters_and_fired_rule_ids() {
        let a = Segment::unstressed(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        );
        let o = Segment::unstressed(
            "o",
            FeatureBundle::vowel(Height::MidHigh, Backness::Back, true),
        );
        let w0 = Word::new(vec![a]);
        let w1 = Word::new(vec![o]);
        let step = DerivationStep::new("R1", "Backing", w0.clone(), w1.clone());
        let trace = DerivationTrace::new(w0.clone(), vec![step], w1.clone());

        assert_eq!(trace.initial(), &w0);
        assert_eq!(trace.final_word(), &w1);
        assert_eq!(trace.steps().len(), 1);
        assert_eq!(trace.steps()[0].rule_name(), "Backing");
        assert_eq!(trace.steps()[0].word_before(), &w0);
        assert_eq!(trace.steps()[0].word_after(), &w1);
        assert_eq!(trace.fired_rule_ids(), vec!["R1"]);
    }
}
