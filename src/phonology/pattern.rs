//! Explicit phonological feature patterns, mutations, prosodic stress matching,
//! and alpha-variable bindings.

use std::collections::BTreeMap;
use std::fmt;

use super::features::{Backness, FeatureBundle, Height, Manner, Place, Stress};
use super::segment::Segment;

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

    /// Returns the list of [`FeatureField`]s explicitly constrained by this pattern.
    #[must_use]
    pub fn constrained_fields(&self) -> Vec<FeatureField> {
        let mut fields = Vec::new();
        if self.manner.is_some() {
            fields.push(FeatureField::Manner);
        }
        if !matches!(self.place, FieldMatch::Any) {
            fields.push(FeatureField::Place);
        }
        if !matches!(self.height, FieldMatch::Any) {
            fields.push(FeatureField::Height);
        }
        if !matches!(self.backness, FieldMatch::Any) {
            fields.push(FeatureField::Backness);
        }
        for (field, flag) in [
            (FeatureField::Voiced, self.voiced.is_some()),
            (FeatureField::Aspirated, self.aspirated.is_some()),
            (FeatureField::Palatalized, self.palatalized.is_some()),
            (FeatureField::Labialized, self.labialized.is_some()),
            (FeatureField::Rounded, self.rounded.is_some()),
            (FeatureField::Syllabic, self.syllabic.is_some()),
            (FeatureField::Long, self.long.is_some()),
        ] {
            if flag {
                fields.push(field);
            }
        }
        fields
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

    /// Returns the list of [`FeatureField`]s explicitly updated by this mutation.
    #[must_use]
    pub fn updated_fields(&self) -> Vec<FeatureField> {
        let mut fields = Vec::new();
        if self.manner.is_some() {
            fields.push(FeatureField::Manner);
        }
        if !matches!(self.place, FieldUpdate::Keep) {
            fields.push(FeatureField::Place);
        }
        if !matches!(self.height, FieldUpdate::Keep) {
            fields.push(FeatureField::Height);
        }
        if !matches!(self.backness, FieldUpdate::Keep) {
            fields.push(FeatureField::Backness);
        }
        for (field, flag) in [
            (FeatureField::Voiced, self.voiced.is_some()),
            (FeatureField::Aspirated, self.aspirated.is_some()),
            (FeatureField::Palatalized, self.palatalized.is_some()),
            (FeatureField::Labialized, self.labialized.is_some()),
            (FeatureField::Rounded, self.rounded.is_some()),
            (FeatureField::Syllabic, self.syllabic.is_some()),
            (FeatureField::Long, self.long.is_some()),
        ] {
            if flag {
                fields.push(field);
            }
        }
        fields
    }
}

/// Prosodic stress predicate evaluated against [`Segment::stress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum StressMatch {
    /// Matches any stress level.
    #[default]
    Any,
    /// Matches either [`Stress::Primary`] or [`Stress::Secondary`].
    Stressed,
    /// Matches an exact [`Stress`] level (`Unstressed`, `Primary`, or `Secondary`).
    Is(Stress),
}

impl StressMatch {
    /// Returns `true` if `stress` satisfies this stress predicate.
    #[must_use]
    pub const fn matches(&self, stress: Stress) -> bool {
        matches!(
            (self, stress),
            (Self::Any, _)
                | (Self::Stressed, Stress::Primary | Stress::Secondary)
                | (Self::Is(Stress::Unstressed), Stress::Unstressed)
                | (Self::Is(Stress::Primary), Stress::Primary)
                | (Self::Is(Stress::Secondary), Stress::Secondary)
        )
    }
}

/// Prosodic stress update applied to an existing segment's [`Stress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum StressUpdate {
    /// Leaves the source segment's stress unchanged.
    #[default]
    Keep,
    /// Replaces the segment's stress with an explicit [`Stress`] level.
    Set(Stress),
}

impl StressUpdate {
    /// Returns the updated stress value derived from `stress`.
    #[must_use]
    pub const fn apply(&self, stress: Stress) -> Stress {
        match self {
            Self::Keep => stress,
            Self::Set(new_stress) => *new_stress,
        }
    }
}

/// Prosodic stress directive for literal replacement items.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LiteralStress {
    /// Inherits the stress of the matched target segment at the same output position
    /// (falling back to [`Stress::Unstressed`] if unmapped or already copied by `Mutate`).
    #[default]
    Inherit,
    /// Emits the segment with an explicit [`Stress`] level.
    Set(Stress),
    /// Emits the segment with the [`Stress`] bound to the named alpha variable.
    Var(String),
}

/// Compatibility domain for phonological alpha variables (`FeatureVar`).
///
/// All boolean features share [`FeatureDomain::Bool`], allowing cross-feature correlations
/// such as `[+stop, αvoiced] > [αaspirated]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureDomain {
    /// [`Manner`] enum domain.
    Manner,
    /// [`Place`] enum domain.
    Place,
    /// [`Height`] enum domain.
    Height,
    /// [`Backness`] enum domain.
    Backness,
    /// Shared binary phonological feature domain (`voiced`, `aspirated`, etc.).
    Bool,
    /// Prosodic [`Stress`] domain.
    Stress,
}

impl fmt::Display for FeatureDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Manner => "manner",
            Self::Place => "place",
            Self::Height => "height",
            Self::Backness => "backness",
            Self::Bool => "bool",
            Self::Stress => "stress",
        };
        f.write_str(label)
    }
}

/// Segment feature or prosodic dimension addressable by an alpha variable or conflict check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureField {
    /// Primary manner of articulation.
    Manner,
    /// Place of articulation (`Option<Place>` on bundles).
    Place,
    /// Vowel height (`Option<Height>` on bundles).
    Height,
    /// Vowel backness (`Option<Backness>` on bundles).
    Backness,
    /// Voicing boolean flag.
    Voiced,
    /// Aspiration boolean flag.
    Aspirated,
    /// Palatalization boolean flag.
    Palatalized,
    /// Labialization boolean flag.
    Labialized,
    /// Rounding boolean flag.
    Rounded,
    /// Syllabicity boolean flag.
    Syllabic,
    /// Phonological length boolean flag.
    Long,
    /// Prosodic stress stored on `Segment`.
    Stress,
}

impl FeatureField {
    /// Returns the compatibility [`FeatureDomain`] of this field.
    #[must_use]
    pub const fn domain(self) -> FeatureDomain {
        match self {
            Self::Manner => FeatureDomain::Manner,
            Self::Place => FeatureDomain::Place,
            Self::Height => FeatureDomain::Height,
            Self::Backness => FeatureDomain::Backness,
            Self::Voiced
            | Self::Aspirated
            | Self::Palatalized
            | Self::Labialized
            | Self::Rounded
            | Self::Syllabic
            | Self::Long => FeatureDomain::Bool,
            Self::Stress => FeatureDomain::Stress,
        }
    }

    /// Returns `true` if this field belongs to [`FeatureDomain::Bool`].
    #[must_use]
    pub const fn is_bool(self) -> bool {
        matches!(self.domain(), FeatureDomain::Bool)
    }

    /// Returns the canonical lowercase field identifier used in DSL diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manner => "manner",
            Self::Place => "place",
            Self::Height => "height",
            Self::Backness => "backness",
            Self::Voiced => "voiced",
            Self::Aspirated => "aspirated",
            Self::Palatalized => "palatalized",
            Self::Labialized => "labialized",
            Self::Rounded => "rounded",
            Self::Syllabic => "syllabic",
            Self::Long => "long",
            Self::Stress => "stress",
        }
    }
}

impl fmt::Display for FeatureField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Polarity of a [`FeatureVar`] binding or lookup (`α` vs `-α`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum VarPolarity {
    /// Direct value equality (`α`).
    #[default]
    Same,
    /// Boolean negation (`-α`); valid only for [`FeatureDomain::Bool`] fields.
    Opposite,
}

/// Alpha variable constraint or assignment targeting a [`FeatureField`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeatureVar {
    /// Target phonological or prosodic field.
    pub field: FeatureField,
    /// Alpha variable identifier (e.g., `"α"`, `"p"`).
    pub name: String,
    /// Polarity relative to the bound variable value.
    pub polarity: VarPolarity,
}

impl FeatureVar {
    /// Creates a positive (`α`) variable constraint/assignment on `field`.
    #[must_use]
    pub fn same(field: FeatureField, name: impl Into<String>) -> Self {
        Self {
            field,
            name: name.into(),
            polarity: VarPolarity::Same,
        }
    }

    /// Creates a negated (`-α`) variable constraint/assignment on `field`.
    #[must_use]
    pub fn opposite(field: FeatureField, name: impl Into<String>) -> Self {
        Self {
            field,
            name: name.into(),
            polarity: VarPolarity::Opposite,
        }
    }
}

/// Concrete feature or stress value stored in [`VarBindings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureVarValue {
    /// Bound [`Manner`] value.
    Manner(Manner),
    /// Bound [`Place`] value.
    Place(Place),
    /// Bound [`Height`] value.
    Height(Height),
    /// Bound [`Backness`] value.
    Backness(Backness),
    /// Bound boolean feature value.
    Bool(bool),
    /// Bound [`Stress`] value.
    Stress(Stress),
}

impl FeatureVarValue {
    /// Returns the [`FeatureDomain`] corresponding to this value.
    #[must_use]
    pub const fn domain(self) -> FeatureDomain {
        match self {
            Self::Manner(_) => FeatureDomain::Manner,
            Self::Place(_) => FeatureDomain::Place,
            Self::Height(_) => FeatureDomain::Height,
            Self::Backness(_) => FeatureDomain::Backness,
            Self::Bool(_) => FeatureDomain::Bool,
            Self::Stress(_) => FeatureDomain::Stress,
        }
    }

    /// Negates a boolean value (`Self::Bool(v)`), returning `None` for non-boolean values.
    #[must_use]
    pub const fn negate(self) -> Option<Self> {
        match self {
            Self::Bool(flag) => Some(Self::Bool(!flag)),
            _ => None,
        }
    }

    /// Extracts the value of `field` from `segment`.
    ///
    /// Optional dimensions (`Place`, `Height`, `Backness`) yield `None` when absent on the bundle,
    /// so segments lacking that dimension never bind or satisfy a variable on it.
    #[must_use]
    pub const fn extract(segment: &Segment, field: FeatureField) -> Option<Self> {
        let bundle = &segment.bundle;
        match field {
            FeatureField::Manner => Some(Self::Manner(bundle.manner)),
            FeatureField::Place => match bundle.place {
                Some(place) => Some(Self::Place(place)),
                None => None,
            },
            FeatureField::Height => match bundle.height {
                Some(height) => Some(Self::Height(height)),
                None => None,
            },
            FeatureField::Backness => match bundle.backness {
                Some(backness) => Some(Self::Backness(backness)),
                None => None,
            },
            FeatureField::Voiced => Some(Self::Bool(bundle.voiced)),
            FeatureField::Aspirated => Some(Self::Bool(bundle.aspirated)),
            FeatureField::Palatalized => Some(Self::Bool(bundle.palatalized)),
            FeatureField::Labialized => Some(Self::Bool(bundle.labialized)),
            FeatureField::Rounded => Some(Self::Bool(bundle.rounded)),
            FeatureField::Syllabic => Some(Self::Bool(bundle.syllabic)),
            FeatureField::Long => Some(Self::Bool(bundle.long)),
            FeatureField::Stress => Some(Self::Stress(segment.stress)),
        }
    }

    /// Writes this value into `field` of `bundle` or `stress`, returning `false` if the domain
    /// does not match `field`.
    pub const fn apply_to(
        self,
        field: FeatureField,
        bundle: &mut FeatureBundle,
        stress: &mut Stress,
    ) -> bool {
        match (field, self) {
            (FeatureField::Manner, Self::Manner(m)) => bundle.manner = m,
            (FeatureField::Place, Self::Place(p)) => bundle.place = Some(p),
            (FeatureField::Height, Self::Height(h)) => bundle.height = Some(h),
            (FeatureField::Backness, Self::Backness(b)) => bundle.backness = Some(b),
            (FeatureField::Voiced, Self::Bool(v)) => bundle.voiced = v,
            (FeatureField::Aspirated, Self::Bool(v)) => bundle.aspirated = v,
            (FeatureField::Palatalized, Self::Bool(v)) => bundle.palatalized = v,
            (FeatureField::Labialized, Self::Bool(v)) => bundle.labialized = v,
            (FeatureField::Rounded, Self::Bool(v)) => bundle.rounded = v,
            (FeatureField::Syllabic, Self::Bool(v)) => bundle.syllabic = v,
            (FeatureField::Long, Self::Bool(v)) => bundle.long = v,
            (FeatureField::Stress, Self::Stress(s)) => *stress = s,
            _ => return false,
        }
        true
    }
}

/// Deterministic map of bound alpha variable names to concrete feature or stress values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct VarBindings {
    values: BTreeMap<String, FeatureVarValue>,
}

impl VarBindings {
    /// Creates an empty [`VarBindings`] map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Returns the concrete [`FeatureVarValue`] bound to `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<FeatureVarValue> {
        self.values.get(name).copied()
    }

    /// Returns `true` if `name` is currently bound.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Returns `true` if no variables are bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of bound variables.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Iterates deterministically over `(name, value)` pairs in lexicographical order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, FeatureVarValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Checks `segment` against `var`, binding `var.name` if unbound or verifying agreement.
    ///
    /// Returns `false` if `var.field` is absent on `segment` (e.g., `Place` on a vowel), if
    /// [`VarPolarity::Opposite`] is used on a non-boolean field, or if the implied value disagrees
    /// with an existing binding for `var.name`.
    pub fn match_or_bind(&mut self, var: &FeatureVar, segment: &Segment) -> bool {
        let Some(seg_val) = FeatureVarValue::extract(segment, var.field) else {
            return false;
        };
        let implied = match var.polarity {
            VarPolarity::Same => seg_val,
            VarPolarity::Opposite => match seg_val.negate() {
                Some(neg) => neg,
                None => return false,
            },
        };

        if let Some(&existing) = self.values.get(&var.name) {
            existing == implied
        } else {
            self.values.insert(var.name.clone(), implied);
            true
        }
    }

    /// Resolves `var` against the bound value for `var.name`, applying `var.polarity`.
    ///
    /// Returns `None` if `var.name` is unbound, has an incompatible [`FeatureDomain`], or uses
    /// [`VarPolarity::Opposite`] on a non-boolean value.
    #[must_use]
    pub fn resolve(&self, var: &FeatureVar) -> Option<FeatureVarValue> {
        let bound = self.values.get(&var.name).copied()?;
        if bound.domain() != var.field.domain() {
            return None;
        }
        match var.polarity {
            VarPolarity::Same => Some(bound),
            VarPolarity::Opposite => bound.negate(),
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

    #[test]
    fn stress_match_and_update_cover_all_stress_levels() {
        assert!(StressMatch::Any.matches(Stress::Unstressed));
        assert!(StressMatch::Any.matches(Stress::Primary));
        assert!(StressMatch::Any.matches(Stress::Secondary));

        assert!(!StressMatch::Stressed.matches(Stress::Unstressed));
        assert!(StressMatch::Stressed.matches(Stress::Primary));
        assert!(StressMatch::Stressed.matches(Stress::Secondary));

        assert!(StressMatch::Is(Stress::Unstressed).matches(Stress::Unstressed));
        assert!(!StressMatch::Is(Stress::Primary).matches(Stress::Secondary));
        assert!(StressMatch::Is(Stress::Secondary).matches(Stress::Secondary));

        assert_eq!(StressUpdate::Keep.apply(Stress::Primary), Stress::Primary);
        assert_eq!(
            StressUpdate::Set(Stress::Secondary).apply(Stress::Unstressed),
            Stress::Secondary
        );
        assert_eq!(LiteralStress::default(), LiteralStress::Inherit);
    }

    #[test]
    fn var_bindings_match_bind_negate_and_reject_absent_optional_dimensions() {
        let mut p = Segment::unstressed(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        );
        let b = Segment::unstressed(
            "b",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true),
        );
        let k = Segment::unstressed(
            "k",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false),
        );
        let a = Segment::new(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
            Stress::Primary,
        );

        let alpha_place = FeatureVar::same(FeatureField::Place, "α");
        let alpha_voiced = FeatureVar::same(FeatureField::Voiced, "v");
        let neg_voiced = FeatureVar::opposite(FeatureField::Voiced, "v");

        // Vowel has place == None: cannot bind or match a Place variable.
        let mut empty = VarBindings::new();
        assert!(!empty.match_or_bind(&alpha_place, &a));
        assert!(empty.is_empty());

        // Opposite polarity on non-bool fails safely without mutating bindings.
        let bad_neg_place = FeatureVar::opposite(FeatureField::Place, "α");
        assert!(!empty.match_or_bind(&bad_neg_place, &p));
        assert!(empty.is_empty());

        // Bind Place from `p`, verify agreement with `b` and disagreement with `k`.
        let mut bindings = VarBindings::new();
        assert!(bindings.match_or_bind(&alpha_place, &p));
        assert!(bindings.contains("α"));
        assert_eq!(
            bindings.get("α"),
            Some(FeatureVarValue::Place(Place::Bilabial))
        );
        assert!(bindings.match_or_bind(&alpha_place, &b));
        assert!(!bindings.match_or_bind(&alpha_place, &k));

        // Bind opposite polarity boolean variable `-v` from `-voiced` `p` -> `v = true`.
        assert!(bindings.match_or_bind(&neg_voiced, &p));
        assert_eq!(bindings.get("v"), Some(FeatureVarValue::Bool(true)));
        assert!(bindings.match_or_bind(&alpha_voiced, &b));
        assert!(!bindings.match_or_bind(&alpha_voiced, &p));

        // Cross-boolean domain sharing resolves `v` on another boolean field (`Aspirated`).
        let alpha_asp = FeatureVar::same(FeatureField::Aspirated, "v");
        assert_eq!(
            bindings.resolve(&alpha_asp),
            Some(FeatureVarValue::Bool(true))
        );
        assert_eq!(
            bindings.resolve(&FeatureVar::same(FeatureField::Place, "v")),
            None
        );

        // Apply resolved values to a bundle & stress.
        let mut out_bundle = k.bundle;
        let mut out_stress = k.stress;
        assert!(FeatureVarValue::Place(Place::Bilabial).apply_to(
            FeatureField::Place,
            &mut out_bundle,
            &mut out_stress
        ));
        assert_eq!(out_bundle.place, Some(Place::Bilabial));
        assert!(FeatureVarValue::Stress(Stress::Secondary).apply_to(
            FeatureField::Stress,
            &mut out_bundle,
            &mut out_stress
        ));
        assert_eq!(out_stress, Stress::Secondary);

        // Domain metadata helpers.
        assert!(FeatureField::Voiced.is_bool());
        assert!(!FeatureField::Stress.is_bool());
        assert_eq!(FeatureField::Stress.domain(), FeatureDomain::Stress);
        assert_eq!(p.stress, Stress::Unstressed);
        p.stress = Stress::Secondary;
        assert_eq!(
            FeatureVarValue::extract(&p, FeatureField::Stress),
            Some(FeatureVarValue::Stress(Stress::Secondary))
        );
    }
}
