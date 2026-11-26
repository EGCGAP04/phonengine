//! Segment and Word representations separated from raw text.

use std::fmt;

use super::features::{FeatureBundle, Stress};
use super::inventory::{Inventory, InventoryError};

/// A phonological segment pairing its surface symbol, intrinsic features, and prosodic stress.
///
/// [`Inventory::segment`], [`Word::from_spaced`], and `tokenize_word` build segments whose
/// `symbol` and `bundle` agree with the inventory. A segment built by hand must keep them
/// consistent: literal patterns and natural classes compare `symbol`, while feature patterns
/// compare `bundle`.
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
    /// resolved against `inventory`.
    ///
    /// A token that is not a registered symbol may start with an IPA stress marker (`ˈ` or `ˌ`)
    /// followed by a registered symbol, which yields that segment with the marker's stress
    /// (unless the marker itself is registered as a symbol). This reads back the output of
    /// [`Word::to_spaced`].
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::UnknownSymbol`] with the whole token if a token is neither a
    /// registered symbol nor a stress marker followed by one.
    pub fn from_spaced(input: &str, inventory: &Inventory) -> Result<Self, InventoryError> {
        let mut segments = Vec::new();
        for token in input.split_whitespace() {
            segments.push(spaced_token_segment(token, inventory)?);
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

    /// Formats the word as space-separated segment symbols, prefixing stressed segments with
    /// their IPA marker (`ˈ` primary, `ˌ` secondary), e.g. `t e kʰ ˈeː s`.
    ///
    /// When no stress marker is registered as an inventory symbol, [`Word::from_spaced`] reads
    /// the output back into an equal word.
    #[must_use]
    pub fn to_spaced(&self) -> String {
        let mut out = String::new();
        for (idx, seg) in self.segments.iter().enumerate() {
            if idx > 0 {
                out.push(' ');
            }
            if let Some(marker) = seg.stress.ipa_marker() {
                out.push(marker);
            }
            out.push_str(&seg.symbol);
        }
        out
    }
}

/// Resolves one token of the spaced format, accepting an optional leading stress marker.
fn spaced_token_segment(token: &str, inventory: &Inventory) -> Result<Segment, InventoryError> {
    if inventory.get(token).is_some() {
        return inventory.segment(token, Stress::Unstressed);
    }
    if let Some(first) = token.chars().next()
        && let Some(stress) = Stress::from_ipa_marker(first)
    {
        let (marker, symbol) = token.split_at(first.len_utf8());
        if inventory.get(marker).is_none() && inventory.get(symbol).is_some() {
            return inventory.segment(symbol, stress);
        }
    }
    Err(InventoryError::UnknownSymbol(token.to_owned()))
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
    fn spaced_format_round_trips_stress_markers() -> Result<(), InventoryError> {
        let inv = sample_inventory()?;
        let word = Word::new(vec![
            inv.segment("p", Stress::Unstressed)?,
            inv.segment("a", Stress::Primary)?,
            inv.segment("t", Stress::Unstressed)?,
            inv.segment("a", Stress::Secondary)?,
        ]);
        assert_eq!(word.to_spaced(), "p ˈa t ˌa");
        assert_eq!(word.to_string(), "p ˈa t ˌa");
        assert_eq!(Word::from_spaced(&word.to_spaced(), &inv)?, word);

        // A marker must be followed by a registered symbol; the error reports the whole token.
        assert_eq!(
            Word::from_spaced("p ˈx", &inv),
            Err(InventoryError::UnknownSymbol("ˈx".to_owned()))
        );
        assert_eq!(
            Word::from_spaced("ˈ a", &inv),
            Err(InventoryError::UnknownSymbol("ˈ".to_owned()))
        );
        assert_eq!(
            Word::from_spaced("ˈˈa", &inv),
            Err(InventoryError::UnknownSymbol("ˈˈa".to_owned()))
        );
        Ok(())
    }

    #[test]
    fn spaced_format_prefers_registered_symbols_over_stress_markers() -> Result<(), InventoryError>
    {
        let mut inv = sample_inventory()?;
        // `ˈa` registered as a symbol wins over "stressed `a`".
        inv.insert(
            "ˈa",
            FeatureBundle::vowel(Height::Low, Backness::Central, false).with_long(true),
        )?;
        let word = Word::from_spaced("p ˈa", &inv)?;
        assert_eq!(word.segments()[1].symbol, "ˈa");
        assert_eq!(word.segments()[1].stress, Stress::Unstressed);

        // A registered marker is a segment, not a stress prefix.
        let mut with_marker = sample_inventory()?;
        with_marker.insert(
            "ˈ",
            FeatureBundle::consonant(Manner::Approximant, Place::Glottal, false),
        )?;
        assert_eq!(
            Word::from_spaced("ˈt", &with_marker),
            Err(InventoryError::UnknownSymbol("ˈt".to_owned()))
        );
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
