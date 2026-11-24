//! Phonological features, feature bundles, and prosodic stress.

use std::fmt;

/// Primary manner of articulation for a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Manner {
    /// Plosive / oral stop.
    Stop,
    /// Fricative.
    Fricative,
    /// Affricate.
    Affricate,
    /// Nasal consonant.
    Nasal,
    /// Lateral approximant or fricative.
    Lateral,
    /// Rhotic consonant.
    Rhotic,
    /// Non-syllabic approximant / glide.
    Approximant,
    /// Syllabic vocoid.
    Vowel,
}

impl fmt::Display for Manner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Stop => "stop",
            Self::Fricative => "fricative",
            Self::Affricate => "affricate",
            Self::Nasal => "nasal",
            Self::Lateral => "lateral",
            Self::Rhotic => "rhotic",
            Self::Approximant => "approximant",
            Self::Vowel => "vowel",
        };
        f.write_str(label)
    }
}

/// Place of articulation for consonantal segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    /// Bilabial.
    Bilabial,
    /// Labiodental.
    Labiodental,
    /// Dental.
    Dental,
    /// Alveolar.
    Alveolar,
    /// Postalveolar.
    Postalveolar,
    /// Palatal.
    Palatal,
    /// Velar.
    Velar,
    /// Uvular.
    Uvular,
    /// Pharyngeal.
    Pharyngeal,
    /// Glottal.
    Glottal,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Bilabial => "bilabial",
            Self::Labiodental => "labiodental",
            Self::Dental => "dental",
            Self::Alveolar => "alveolar",
            Self::Postalveolar => "postalveolar",
            Self::Palatal => "palatal",
            Self::Velar => "velar",
            Self::Uvular => "uvular",
            Self::Pharyngeal => "pharyngeal",
            Self::Glottal => "glottal",
        };
        f.write_str(label)
    }
}

/// Vowel height dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Height {
    /// Close / high vowel.
    High,
    /// Close-mid / mid-high vowel.
    MidHigh,
    /// Open-mid / mid-low vowel.
    MidLow,
    /// Open / low vowel.
    Low,
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::High => "high",
            Self::MidHigh => "mid-high",
            Self::MidLow => "mid-low",
            Self::Low => "low",
        };
        f.write_str(label)
    }
}

/// Vowel backness dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Backness {
    /// Front vowel.
    Front,
    /// Central vowel.
    Central,
    /// Back vowel.
    Back,
}

impl fmt::Display for Backness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Front => "front",
            Self::Central => "central",
            Self::Back => "back",
        };
        f.write_str(label)
    }
}

/// Prosodic stress level carried by a [`Segment`](crate::phonology::Segment) outside its
/// [`FeatureBundle`] to avoid combinatorial blow-up in [`Inventory`](crate::phonology::Inventory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Stress {
    /// Unstressed segment.
    #[default]
    Unstressed,
    /// Primary word stress.
    Primary,
    /// Secondary stress.
    Secondary,
}

impl fmt::Display for Stress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Unstressed => "unstressed",
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        };
        f.write_str(label)
    }
}

/// Intrinsic phonological feature specification for an inventory segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeatureBundle {
    /// Primary manner of articulation.
    pub manner: Manner,
    /// Place of articulation (typically `Some` for consonants, `None` for vowels).
    pub place: Option<Place>,
    /// Vowel height (typically `Some` for vowels, `None` for consonants).
    pub height: Option<Height>,
    /// Vowel backness (typically `Some` for vowels, `None` for consonants).
    pub backness: Option<Backness>,
    /// Whether the vocal folds vibrate.
    pub voiced: bool,
    /// Whether the segment is aspirated.
    pub aspirated: bool,
    /// Whether the segment has secondary palatalization.
    pub palatalized: bool,
    /// Whether the segment has secondary labialization.
    pub labialized: bool,
    /// Whether the lips are rounded.
    pub rounded: bool,
    /// Whether the segment acts as a syllable nucleus.
    pub syllabic: bool,
    /// Whether the segment is phonologically long.
    pub long: bool,
}

impl FeatureBundle {
    /// Creates a consonantal feature bundle with default secondary and prosodic flags.
    #[must_use]
    pub const fn consonant(manner: Manner, place: Place, voiced: bool) -> Self {
        Self {
            manner,
            place: Some(place),
            height: None,
            backness: None,
            voiced,
            aspirated: false,
            palatalized: false,
            labialized: false,
            rounded: false,
            syllabic: false,
            long: false,
        }
    }

    /// Creates a voiced, syllabic vowel feature bundle.
    #[must_use]
    pub const fn vowel(height: Height, backness: Backness, rounded: bool) -> Self {
        Self {
            manner: Manner::Vowel,
            place: None,
            height: Some(height),
            backness: Some(backness),
            voiced: true,
            aspirated: false,
            palatalized: false,
            labialized: false,
            rounded,
            syllabic: true,
            long: false,
        }
    }

    /// Returns a copy with `voiced` updated.
    #[must_use]
    pub const fn with_voiced(mut self, voiced: bool) -> Self {
        self.voiced = voiced;
        self
    }

    /// Returns a copy with `aspirated` updated.
    #[must_use]
    pub const fn with_aspirated(mut self, aspirated: bool) -> Self {
        self.aspirated = aspirated;
        self
    }

    /// Returns a copy with `palatalized` updated.
    #[must_use]
    pub const fn with_palatalized(mut self, palatalized: bool) -> Self {
        self.palatalized = palatalized;
        self
    }

    /// Returns a copy with `labialized` updated.
    #[must_use]
    pub const fn with_labialized(mut self, labialized: bool) -> Self {
        self.labialized = labialized;
        self
    }

    /// Returns a copy with `rounded` updated.
    #[must_use]
    pub const fn with_rounded(mut self, rounded: bool) -> Self {
        self.rounded = rounded;
        self
    }

    /// Returns a copy with `syllabic` updated.
    #[must_use]
    pub const fn with_syllabic(mut self, syllabic: bool) -> Self {
        self.syllabic = syllabic;
        self
    }

    /// Returns a copy with `long` updated.
    #[must_use]
    pub const fn with_long(mut self, long: bool) -> Self {
        self.long = long;
        self
    }
}

impl fmt::Display for FeatureBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}", self.manner)?;
        if let Some(place) = self.place {
            write!(f, ", {place}")?;
        }
        if let Some(height) = self.height {
            write!(f, ", {height}")?;
        }
        if let Some(backness) = self.backness {
            write!(f, ", {backness}")?;
        }
        for (name, enabled) in [
            ("voiced", self.voiced),
            ("aspirated", self.aspirated),
            ("palatalized", self.palatalized),
            ("labialized", self.labialized),
            ("rounded", self.rounded),
            ("syllabic", self.syllabic),
            ("long", self.long),
        ] {
            let sign = if enabled { '+' } else { '-' };
            write!(f, ", {sign}{name}")?;
        }
        f.write_str("]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consonant_and_vowel_constructors_set_expected_defaults() {
        let p = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false);
        assert_eq!(p.manner, Manner::Stop);
        assert_eq!(p.place, Some(Place::Bilabial));
        assert_eq!(p.height, None);
        assert_eq!(p.backness, None);
        assert!(!p.voiced);
        assert!(!p.syllabic);
        assert!(!p.long);

        let o = FeatureBundle::vowel(Height::MidHigh, Backness::Back, true).with_long(true);
        assert_eq!(o.manner, Manner::Vowel);
        assert_eq!(o.place, None);
        assert_eq!(o.height, Some(Height::MidHigh));
        assert_eq!(o.backness, Some(Backness::Back));
        assert!(o.voiced);
        assert!(o.rounded);
        assert!(o.syllabic);
        assert!(o.long);
    }

    #[test]
    fn bundle_builder_flags_and_ordering_are_deterministic() {
        let k = FeatureBundle::consonant(Manner::Stop, Place::Velar, false);
        let kh = k
            .with_aspirated(true)
            .with_palatalized(true)
            .with_labialized(true)
            .with_rounded(true)
            .with_syllabic(true)
            .with_voiced(true);
        assert!(kh.voiced);
        assert!(kh.aspirated);
        assert!(kh.palatalized);
        assert!(kh.labialized);
        assert!(kh.rounded);
        assert!(kh.syllabic);
        assert!(k < kh);
    }

    #[test]
    fn display_formats_enums_and_bundles_readably() {
        let a = FeatureBundle::vowel(Height::Low, Backness::Central, false);
        let rendered = a.to_string();
        assert!(rendered.contains("vowel"));
        assert!(rendered.contains("low"));
        assert!(rendered.contains("central"));
        assert!(rendered.contains("+syllabic"));
        assert!(rendered.contains("-long"));
        assert_eq!(Stress::default(), Stress::Unstressed);
        assert_eq!(Stress::Primary.to_string(), "primary");
    }
}
