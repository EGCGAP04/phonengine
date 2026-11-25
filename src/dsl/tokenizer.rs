//! Deterministic maximal-munch word tokenizer with IPA stress-marker support.

use std::cmp::Reverse;

use crate::phonology::{FeatureBundle, Inventory, Segment, Stress, Word};

use super::error::{TokenizeError, TokenizeErrorKind};

const PRIMARY_STRESS_MARKER: char = 'ˈ';
const SECONDARY_STRESS_MARKER: char = 'ˌ';

/// Tokenizes an unspaced word string into a [`Word`] using deterministic maximal munch.
///
/// Inventory symbols are ordered by descending byte length and lexicographically as a tie-breaker.
/// IPA stress markers `'ˈ'` (`U+02C8`) and `'ˌ'` (`U+02CC`) apply [`Stress::Primary`] and
/// [`Stress::Secondary`] to the immediately following segment unless explicitly registered as
/// inventory symbols themselves.
///
/// # Errors
///
/// - [`TokenizeErrorKind::UnexpectedWhitespace`] if `input` contains whitespace.
/// - [`TokenizeErrorKind::DuplicateStressMarker`] if two stress markers precede a segment.
/// - [`TokenizeErrorKind::DanglingStressMarker`] if a stress marker appears at the end of `input`.
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
            let stress = pending_stress
                .take()
                .map_or(Stress::Unstressed, |(level, _, _, _)| level);
            segments.push(Segment::new(sym, *bundle, stress));
            offset += sym.len();
            column += sym.chars().count();
            continue;
        }

        if ch == PRIMARY_STRESS_MARKER || ch == SECONDARY_STRESS_MARKER {
            if pending_stress.is_some() {
                return Err(TokenizeError::new(
                    TokenizeErrorKind::DuplicateStressMarker(ch),
                    offset,
                    column,
                ));
            }
            let level = if ch == PRIMARY_STRESS_MARKER {
                Stress::Primary
            } else {
                Stress::Secondary
            };
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
        let word = tokenize_word("ˌkʷaˈtʃaː", &inv)?;
        assert_eq!(word.to_spaced(), "kʷ a tʃ aː");
        assert_eq!(word.segments()[0].stress, Stress::Secondary);
        assert_eq!(word.segments()[1].stress, Stress::Unstressed);
        assert_eq!(word.segments()[2].stress, Stress::Primary);
        assert_eq!(word.segments()[3].stress, Stress::Unstressed);

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
        Ok(())
    }
}
