//! Explicit phonological feature patterns and mutations.

use super::features::{Backness, FeatureBundle, Height, Manner, Place};

/// Matcher for optional feature dimensions (`Option<T>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum FieldMatch<T> {
    /// Matches regardless of whether the field is `Some` or `None`.
    #[default]
    Any,
    /// Matches only when the field equals `Some(value)`.
    Is(T),
    /// Matches only when the field is `None`.
    IsNone,
}

impl<T: PartialEq> FieldMatch<T> {
    /// Evaluates whether `value` satisfies this matcher.
    #[must_use]
    pub fn matches(&self, value: Option<T>) -> bool {
        match self {
            Self::Any => true,
            Self::Is(expected) => value.as_ref() == Some(expected),
            Self::IsNone => value.is_none(),
        }
    }
}

/// Update operation for optional feature dimensions (`Option<T>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum FieldUpdate<T> {
    /// Leaves the original field value unchanged.
    #[default]
    Keep,
    /// Sets the field to `Some(value)`.
    Set(T),
    /// Clears the field to `None`.
    Clear,
}

impl<T: Copy> FieldUpdate<T> {
    /// Produces the updated field value from `value`.
    #[must_use]
    pub const fn apply(&self, value: Option<T>) -> Option<T> {
        match self {
            Self::Keep => value,
            Self::Set(new_value) => Some(*new_value),
            Self::Clear => None,
        }
    }
}

/// Pattern matching segments by phonological feature constraints.
///
/// Unspecified fields (`None` for booleans/manner, [`FieldMatch::Any`] for optional
/// dimensions) act as wildcards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct FeaturePattern {
    /// Optional exact manner constraint.
    pub manner: Option<Manner>,
    /// Place of articulation constraint.
    pub place: FieldMatch<Place>,
    /// Vowel height constraint.
    pub height: FieldMatch<Height>,
    /// Vowel backness constraint.
    pub backness: FieldMatch<Backness>,
    /// Optional voicing constraint.
    pub voiced: Option<bool>,
    /// Optional aspiration constraint.
    pub aspirated: Option<bool>,
    /// Optional palatalization constraint.
    pub palatalized: Option<bool>,
    /// Optional labialization constraint.
    pub labialized: Option<bool>,
    /// Optional rounding constraint.
    pub rounded: Option<bool>,
    /// Optional syllabicity constraint.
    pub syllabic: Option<bool>,
    /// Optional length constraint.
    pub long: Option<bool>,
}

impl FeaturePattern {
    /// Creates an unconstrained pattern that matches any [`FeatureBundle`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            manner: None,
            place: FieldMatch::Any,
            height: FieldMatch::Any,
            backness: FieldMatch::Any,
            voiced: None,
            aspirated: None,
            palatalized: None,
            labialized: None,
            rounded: None,
            syllabic: None,
            long: None,
        }
    }

    /// Constrains the pattern to a specific [`Manner`].
    #[must_use]
    pub const fn with_manner(mut self, manner: Manner) -> Self {
        self.manner = Some(manner);
        self
    }

    /// Constrains the pattern's [`Place`] matcher.
    #[must_use]
    pub const fn with_place(mut self, place: FieldMatch<Place>) -> Self {
        self.place = place;
        self
    }

    /// Constrains the pattern's [`Height`] matcher.
    #[must_use]
    pub const fn with_height(mut self, height: FieldMatch<Height>) -> Self {
        self.height = height;
        self
    }

    /// Constrains the pattern's [`Backness`] matcher.
    #[must_use]
    pub const fn with_backness(mut self, backness: FieldMatch<Backness>) -> Self {
        self.backness = backness;
        self
    }

    /// Constrains `voiced`.
    #[must_use]
    pub const fn with_voiced(mut self, voiced: bool) -> Self {
        self.voiced = Some(voiced);
        self
    }

    /// Constrains `aspirated`.
    #[must_use]
    pub const fn with_aspirated(mut self, aspirated: bool) -> Self {
        self.aspirated = Some(aspirated);
        self
    }

    /// Constrains `palatalized`.
    #[must_use]
    pub const fn with_palatalized(mut self, palatalized: bool) -> Self {
        self.palatalized = Some(palatalized);
        self
    }

    /// Constrains `labialized`.
    #[must_use]
    pub const fn with_labialized(mut self, labialized: bool) -> Self {
        self.labialized = Some(labialized);
        self
    }

    /// Constrains `rounded`.
    #[must_use]
    pub const fn with_rounded(mut self, rounded: bool) -> Self {
        self.rounded = Some(rounded);
        self
    }

    /// Constrains `syllabic`.
    #[must_use]
    pub const fn with_syllabic(mut self, syllabic: bool) -> Self {
        self.syllabic = Some(syllabic);
        self
    }

    /// Constrains `long`.
    #[must_use]
    pub const fn with_long(mut self, long: bool) -> Self {
        self.long = Some(long);
        self
    }

    /// Returns `true` if `bundle` satisfies every specified constraint in `self`.
    #[must_use]
    pub fn matches(&self, bundle: &FeatureBundle) -> bool {
        self.manner.is_none_or(|m| m == bundle.manner)
            && self.place.matches(bundle.place)
            && self.height.matches(bundle.height)
            && self.backness.matches(bundle.backness)
            && self.voiced.is_none_or(|v| v == bundle.voiced)
            && self.aspirated.is_none_or(|v| v == bundle.aspirated)
            && self.palatalized.is_none_or(|v| v == bundle.palatalized)
            && self.labialized.is_none_or(|v| v == bundle.labialized)
            && self.rounded.is_none_or(|v| v == bundle.rounded)
            && self.syllabic.is_none_or(|v| v == bundle.syllabic)
            && self.long.is_none_or(|v| v == bundle.long)
    }
}

/// Mutation applied to an existing [`FeatureBundle`] to derive a target bundle.
///
/// Unspecified fields (`None` for booleans/manner, [`FieldUpdate::Keep`] for optional
/// dimensions) preserve the source segment's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct FeatureMutation {
    /// Optional replacement manner.
    pub manner: Option<Manner>,
    /// Place update.
    pub place: FieldUpdate<Place>,
    /// Vowel height update.
    pub height: FieldUpdate<Height>,
    /// Vowel backness update.
    pub backness: FieldUpdate<Backness>,
    /// Optional voicing update.
    pub voiced: Option<bool>,
    /// Optional aspiration update.
    pub aspirated: Option<bool>,
    /// Optional palatalization update.
    pub palatalized: Option<bool>,
    /// Optional labialization update.
    pub labialized: Option<bool>,
    /// Optional rounding update.
    pub rounded: Option<bool>,
    /// Optional syllabicity update.
    pub syllabic: Option<bool>,
    /// Optional length update.
    pub long: Option<bool>,
}

impl FeatureMutation {
    /// Creates a no-op mutation that preserves all features of the source bundle.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            manner: None,
            place: FieldUpdate::Keep,
            height: FieldUpdate::Keep,
            backness: FieldUpdate::Keep,
            voiced: None,
            aspirated: None,
            palatalized: None,
            labialized: None,
            rounded: None,
            syllabic: None,
            long: None,
        }
    }

    /// Sets a replacement [`Manner`].
    #[must_use]
    pub const fn set_manner(mut self, manner: Manner) -> Self {
        self.manner = Some(manner);
        self
    }

    /// Sets an update for [`Place`].
    #[must_use]
    pub const fn set_place(mut self, place: FieldUpdate<Place>) -> Self {
        self.place = place;
        self
    }

    /// Sets an update for [`Height`].
    #[must_use]
    pub const fn set_height(mut self, height: FieldUpdate<Height>) -> Self {
        self.height = height;
        self
    }

    /// Sets an update for [`Backness`].
    #[must_use]
    pub const fn set_backness(mut self, backness: FieldUpdate<Backness>) -> Self {
        self.backness = backness;
        self
    }

    /// Sets `voiced`.
    #[must_use]
    pub const fn set_voiced(mut self, voiced: bool) -> Self {
        self.voiced = Some(voiced);
        self
    }

    /// Sets `aspirated`.
    #[must_use]
    pub const fn set_aspirated(mut self, aspirated: bool) -> Self {
        self.aspirated = Some(aspirated);
        self
    }

    /// Sets `palatalized`.
    #[must_use]
    pub const fn set_palatalized(mut self, palatalized: bool) -> Self {
        self.palatalized = Some(palatalized);
        self
    }

    /// Sets `labialized`.
    #[must_use]
    pub const fn set_labialized(mut self, labialized: bool) -> Self {
        self.labialized = Some(labialized);
        self
    }

    /// Sets `rounded`.
    #[must_use]
    pub const fn set_rounded(mut self, rounded: bool) -> Self {
        self.rounded = Some(rounded);
        self
    }

    /// Sets `syllabic`.
    #[must_use]
    pub const fn set_syllabic(mut self, syllabic: bool) -> Self {
        self.syllabic = Some(syllabic);
        self
    }

    /// Sets `long`.
    #[must_use]
    pub const fn set_long(mut self, long: bool) -> Self {
        self.long = Some(long);
        self
    }

    /// Applies this mutation to `bundle` without modifying the input.
    #[must_use]
    pub const fn apply(&self, bundle: &FeatureBundle) -> FeatureBundle {
        FeatureBundle {
            manner: match self.manner {
                Some(m) => m,
                None => bundle.manner,
            },
            place: self.place.apply(bundle.place),
            height: self.height.apply(bundle.height),
            backness: self.backness.apply(bundle.backness),
            voiced: match self.voiced {
                Some(v) => v,
                None => bundle.voiced,
            },
            aspirated: match self.aspirated {
                Some(v) => v,
                None => bundle.aspirated,
            },
            palatalized: match self.palatalized {
                Some(v) => v,
                None => bundle.palatalized,
            },
            labialized: match self.labialized {
                Some(v) => v,
                None => bundle.labialized,
            },
            rounded: match self.rounded {
                Some(v) => v,
                None => bundle.rounded,
            },
            syllabic: match self.syllabic {
                Some(v) => v,
                None => bundle.syllabic,
            },
            long: match self.long {
                Some(v) => v,
                None => bundle.long,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_match_handles_any_is_and_is_none() {
        assert!(FieldMatch::<Place>::Any.matches(None));
        assert!(FieldMatch::Any.matches(Some(Place::Velar)));
        assert!(FieldMatch::Is(Place::Velar).matches(Some(Place::Velar)));
        assert!(!FieldMatch::Is(Place::Velar).matches(Some(Place::Alveolar)));
        assert!(!FieldMatch::Is(Place::Velar).matches(None));
        assert!(FieldMatch::<Place>::IsNone.matches(None));
        assert!(!FieldMatch::IsNone.matches(Some(Place::Bilabial)));
    }

    #[test]
    fn field_update_handles_keep_set_and_clear() {
        assert_eq!(
            FieldUpdate::Keep.apply(Some(Place::Dental)),
            Some(Place::Dental)
        );
        assert_eq!(FieldUpdate::<Place>::Keep.apply(None), None);
        assert_eq!(
            FieldUpdate::Set(Place::Palatal).apply(Some(Place::Dental)),
            Some(Place::Palatal)
        );
        assert_eq!(FieldUpdate::Clear.apply(Some(Place::Dental)), None);
    }

    #[test]
    fn feature_pattern_matches_voiceless_stops_and_distinguishes_is_none() {
        let t = FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false);
        let d = FeatureBundle::consonant(Manner::Stop, Place::Alveolar, true);
        let s = FeatureBundle::consonant(Manner::Fricative, Place::Alveolar, false);
        let a = FeatureBundle::vowel(Height::Low, Backness::Central, false);

        let voiceless_stop = FeaturePattern::new()
            .with_manner(Manner::Stop)
            .with_voiced(false)
            .with_height(FieldMatch::IsNone);

        assert!(voiceless_stop.matches(&t));
        assert!(!voiceless_stop.matches(&d));
        assert!(!voiceless_stop.matches(&s));
        assert!(!voiceless_stop.matches(&a));

        assert!(FeaturePattern::new().matches(&t));
        assert!(FeaturePattern::new().matches(&a));
    }

    #[test]
    fn feature_mutation_updates_only_targeted_fields() {
        let p = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false);
        let voice = FeatureMutation::new().set_voiced(true);
        assert_eq!(
            voice.apply(&p),
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true)
        );

        let lenition = FeatureMutation::new()
            .set_manner(Manner::Fricative)
            .set_place(FieldUpdate::Set(Place::Labiodental))
            .set_voiced(true);
        assert_eq!(
            lenition.apply(&p),
            FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true)
        );

        let i = FeatureBundle::vowel(Height::High, Backness::Front, false);
        let to_glide = FeatureMutation::new()
            .set_manner(Manner::Approximant)
            .set_place(FieldUpdate::Set(Place::Palatal))
            .set_height(FieldUpdate::Clear)
            .set_backness(FieldUpdate::Clear)
            .set_syllabic(false);
        assert_eq!(
            to_glide.apply(&i),
            FeatureBundle::consonant(Manner::Approximant, Place::Palatal, true)
        );
    }
}
