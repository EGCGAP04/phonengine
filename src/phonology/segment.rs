//! Segment and Word representations separated from raw text.

use std::fmt;

use super::features::{FeatureBundle, Stress};
use super::inventory::{Inventory, InventoryError};

/// A phonological segment pairing its surface symbol, intrinsic features, and prosodic stress.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segment {
    /// Canonical IPA/orthographic symbol registered in the inventory.
    pub symbol: String,
    /// Intrinsic phonological features.
    pub bundle: FeatureBundle,
    /// Prosodic stress level (kept outside [`FeatureBundle`]).
    pub stress: Stress,
}

impl Segment {
    /// Creates a new [`Segment`] with the given symbol, feature bundle, and stress.
    #[must_use]
    pub fn new(symbol: impl Into<String>, bundle: FeatureBundle, stress: Stress) -> Self {
        Self {
            symbol: symbol.into(),
            bundle,
            stress,
        }
    }

    /// Creates an unstressed [`Segment`].
    #[must_use]
    pub fn unstressed(symbol: impl Into<String>, bundle: FeatureBundle) -> Self {
        Self::new(symbol, bundle, Stress::Unstressed)
    }
}

/// An ordered sequence of phonological [`Segment`]s representing a word.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Word {
    segments: Vec<Segment>,
}

impl Word {
    /// Constructs a [`Word`] from an explicit vector of segments.
    #[must_use]
    pub fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }

    /// Parses a whitespace-delimited sequence of inventory symbols (Milestone 1 format)
    /// into unstressed segments resolved against `inventory`.
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::UnknownSymbol`] if any token is not registered in `inventory`.
    pub fn from_spaced(input: &str, inventory: &Inventory) -> Result<Self, InventoryError> {
        let mut segments = Vec::new();
        for token in input.split_whitespace() {
            segments.push(inventory.segment(token, Stress::Unstressed)?);
        }
        Ok(Self { segments })
    }

    /// Returns the underlying slice of segments.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Consumes the word and returns its segment vector.
    #[must_use]
    pub fn into_segments(self) -> Vec<Segment> {
        self.segments
    }

    /// Returns the number of segments in the word.
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns `true` if the word contains no segments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Formats the word as space-separated segment symbols.
    #[must_use]
    pub fn to_spaced(&self) -> String {
        let mut out = String::new();
        for (idx, seg) in self.segments.iter().enumerate() {
            if idx > 0 {
                out.push(' ');
            }
            out.push_str(&seg.symbol);
        }
        out
    }
}

impl From<Vec<Segment>> for Word {
    fn from(segments: Vec<Segment>) -> Self {
        Self::new(segments)
    }
}

impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_spaced())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::features::{Backness, Height, Manner, Place};

    fn sample_inventory() -> Result<Inventory, InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        )?;
        inv.insert(
            "t",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
        )?;
        Ok(inv)
    }

    #[test]
    fn from_spaced_and_to_spaced_round_trip() -> Result<(), InventoryError> {
        let inv = sample_inventory()?;
        let word = Word::from_spaced("  p a t a  ", &inv)?;
        assert_eq!(word.len(), 4);
        assert!(!word.is_empty());
        assert_eq!(word.to_spaced(), "p a t a");
        assert_eq!(word.to_string(), "p a t a");
        assert_eq!(word.segments()[1].stress, Stress::Unstressed);

        let empty = Word::from_spaced("   ", &inv)?;
        assert!(empty.is_empty());
        assert_eq!(empty.to_spaced(), "");
        Ok(())
    }

    #[test]
    fn from_spaced_rejects_unknown_symbol() -> Result<(), InventoryError> {
        let inv = sample_inventory()?;
        let err = Word::from_spaced("p a x a", &inv);
        assert_eq!(err, Err(InventoryError::UnknownSymbol("x".to_owned())));
        Ok(())
    }

    #[test]
    fn stress_is_stored_on_segment_independently_of_feature_bundle() -> Result<(), InventoryError> {
        let inv = sample_inventory()?;
        let stressed = inv.segment("a", Stress::Primary)?;
        let unstressed = inv.segment("a", Stress::Unstressed)?;
        assert_eq!(stressed.bundle, unstressed.bundle);
        assert_ne!(stressed, unstressed);
        Ok(())
    }
}
