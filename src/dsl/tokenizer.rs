//! Deterministic maximal-munch word tokenizer with IPA stress-marker support.

use std::cmp::Reverse;

use crate::phonology::{FeatureBundle, Inventory, Segment, Stress, Word};

use super::error::{TokenizeError, TokenizeErrorKind};

/// Tokenizes an unspaced word string into a [`Word`] using deterministic maximal munch.
///
/// Inventory symbols are ordered by descending byte length and lexicographically as a tie-breaker.
/// The munch is greedy and never backtracks: with the inventory `{a, ab, bc}`, `"abc"` fails at
/// `c` even though `a` + `bc` would be a valid segmentation.
///
/// IPA stress markers `'ˈ'` (`U+02C8`) and `'ˌ'` (`U+02CC`) apply [`Stress::Primary`] and
/// [`Stress::Secondary`] to the next syllabic segment (`bundle.syllabic`), skipping the
/// non-syllabic segments in between, because IPA writes the marker before the syllable:
/// `ˈvita` stresses `i`. Markers registered as inventory symbols are segments instead.
///
/// # Errors
///
/// - [`TokenizeErrorKind::UnexpectedWhitespace`] if `input` contains whitespace.
/// - [`TokenizeErrorKind::DuplicateStressMarker`] if a second marker appears before the previous
///   one reached a syllabic segment.
/// - [`TokenizeErrorKind::DanglingStressMarker`] if `input` ends with no syllabic segment after a
///   marker.
/// - [`TokenizeErrorKind::UnknownSegment`] if no inventory symbol or stress marker matches.
pub fn tokenize_word(input: &str, inventory: &Inventory) -> Result<Word, TokenizeError> {
    if input.is_empty() {
        return Ok(Word::new(Vec::new()));
    }

    let mut candidates: Vec<(&str, &FeatureBundle)> = inventory.iter().collect();
    candidates.sort_by(|(sym_a, _), (sym_b, _)| {
        Reverse(sym_a.len())
            .cmp(&Reverse(sym_b.len()))
            .then_with(|| sym_a.cmp(sym_b))
    });

    let mut segments = Vec::new();
    let mut offset = 0usize;
    let mut column = 1usize;
    let mut pending_stress: Option<(Stress, char, usize, usize)> = None;

    while offset < input.len() {
        let rest = &input[offset..];
        let Some(ch) = rest.chars().next() else {
            break;
        };

        if ch.is_whitespace() {
            return Err(TokenizeError::new(
                TokenizeErrorKind::UnexpectedWhitespace(ch),
                offset,
                column,
            ));
        }

        if let Some((sym, bundle)) = candidates
            .iter()
            .copied()
            .find(|(sym, _)| rest.starts_with(sym))
        {
            // A pending marker waits for the syllable nucleus.
            let stress = if bundle.syllabic {
                pending_stress
                    .take()
                    .map_or(Stress::Unstressed, |(level, _, _, _)| level)
            } else {
                Stress::Unstressed
            };
            segments.push(Segment::new(sym, *bundle, stress));
            offset += sym.len();
            column += sym.chars().count();
            continue;
        }

        if let Some(level) = Stress::from_ipa_marker(ch) {
            if pending_stress.is_some() {
                return Err(TokenizeError::new(
                    TokenizeErrorKind::DuplicateStressMarker(ch),
                    offset,
                    column,
                ));
            }
            pending_stress = Some((level, ch, offset, column));
            offset += ch.len_utf8();
            column += 1;
            continue;
        }

        return Err(TokenizeError::new(
            TokenizeErrorKind::UnknownSegment(ch.to_string()),
            offset,
            column,
        ));
    }

    if let Some((_, marker, marker_offset, marker_column)) = pending_stress {
        return Err(TokenizeError::new(
            TokenizeErrorKind::DanglingStressMarker(marker),
            marker_offset,
            marker_column,
        ));
    }

    Ok(Word::new(segments))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{Backness, Height, InventoryError, Manner, Place};

    fn sample_inventory() -> Result<Inventory, InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "t",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
        )?;
        inv.insert(
            "ʃ",
            FeatureBundle::consonant(Manner::Fricative, Place::Postalveolar, false),
        )?;
        inv.insert(
            "tʃ",
            FeatureBundle::consonant(Manner::Affricate, Place::Postalveolar, false),
        )?;
        inv.insert(
            "k",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false),
        )?;
        inv.insert(
            "kʷ",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false).with_labialized(true),
        )?;
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "aː",
            FeatureBundle::vowel(Height::Low, Backness::Central, false).with_long(true),
        )?;
        Ok(inv)
    }

    #[test]
    fn maximal_munch_prefers_longest_match_and_handles_stress_markers()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = sample_inventory()?;
        // IPA places markers before the syllable; the stress lands on its vowel.
        let word = tokenize_word("ˌkʷaˈtʃaː", &inv)?;
        assert_eq!(word.to_spaced(), "kʷ ˌa tʃ ˈaː");
        assert_eq!(word.segments()[0].stress, Stress::Unstressed);
        assert_eq!(word.segments()[1].stress, Stress::Secondary);
        assert_eq!(word.segments()[2].stress, Stress::Unstressed);
        assert_eq!(word.segments()[3].stress, Stress::Primary);

        // A marker right before the vowel gives the same word.
        assert_eq!(tokenize_word("kʷˌatʃˈaː", &inv)?, word);

        let empty = tokenize_word("", &inv)?;
        assert!(empty.is_empty());
        Ok(())
    }

    #[test]
    fn registered_stress_symbol_in_inventory_takes_precedence()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut inv = sample_inventory()?;
        inv.insert(
            "ˈ",
            FeatureBundle::consonant(Manner::Stop, Place::Glottal, false),
        )?;
        let word = tokenize_word("ˈa", &inv)?;
        assert_eq!(word.to_spaced(), "ˈ a");
        assert_eq!(word.segments()[0].stress, Stress::Unstressed);
        assert_eq!(word.segments()[1].stress, Stress::Unstressed);
        Ok(())
    }

    #[test]
    fn reports_tokenize_errors_with_byte_offset_and_column() -> Result<(), InventoryError> {
        let inv = sample_inventory()?;

        let unknown = tokenize_word("tʃxa", &inv);
        assert_eq!(
            unknown,
            Err(TokenizeError::new(
                TokenizeErrorKind::UnknownSegment("x".to_owned()),
                "tʃ".len(),
                3
            ))
        );

        let space = tokenize_word("a a", &inv);
        assert_eq!(
            space,
            Err(TokenizeError::new(
                TokenizeErrorKind::UnexpectedWhitespace(' '),
                1,
                2
            ))
        );

        let dup_stress = tokenize_word("ˈˌa", &inv);
        assert_eq!(
            dup_stress,
            Err(TokenizeError::new(
                TokenizeErrorKind::DuplicateStressMarker('ˌ'),
                'ˈ'.len_utf8(),
                2
            ))
        );

        let dangling = tokenize_word("aˈ", &inv);
        assert_eq!(
            dangling,
            Err(TokenizeError::new(
                TokenizeErrorKind::DanglingStressMarker('ˈ'),
                1,
                2
            ))
        );

        // The marker is still pending across `t`, so a second one is a duplicate.
        let dup_across_onset = tokenize_word("ˈtˌa", &inv);
        assert_eq!(
            dup_across_onset,
            Err(TokenizeError::new(
                TokenizeErrorKind::DuplicateStressMarker('ˌ'),
                'ˈ'.len_utf8() + 1,
                3
            ))
        );

        // No syllabic segment after the marker.
        let no_nucleus = tokenize_word("aˈtk", &inv);
        assert_eq!(
            no_nucleus,
            Err(TokenizeError::new(
                TokenizeErrorKind::DanglingStressMarker('ˈ'),
                1,
                2
            ))
        );
        Ok(())
    }

    #[test]
    fn greedy_munch_does_not_backtrack() -> Result<(), InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "ab",
            FeatureBundle::vowel(Height::Low, Backness::Back, false),
        )?;
        inv.insert(
            "bc",
            FeatureBundle::vowel(Height::High, Backness::Back, false),
        )?;
        assert_eq!(
            tokenize_word("abc", &inv),
            Err(TokenizeError::new(
                TokenizeErrorKind::UnknownSegment("c".to_owned()),
                2,
                3
            ))
        );
        Ok(())
    }
}
