//! Pure rule-application engine implementing Milestone 1 `Simultaneous` mode.

use crate::phonology::{Inventory, Segment, Stress, Word};

use super::error::{EngineError, RuleError};
use super::rule::{ApplicationMode, ContextElement, Pattern, ReplacementItem, Rule};
use super::trace::{DerivationStep, DerivationTrace};

/// Applies a single phonological `rule` to `word` using `inventory` without mutating inputs.
///
/// In Milestone 1, only [`ApplicationMode::Simultaneous`] and local contexts
/// ([`ContextElement::WordBoundary`] and [`ContextElement::Segment`]) are supported.
///
/// # Errors
///
/// - [`EngineError::UnsupportedMode`] if `rule.mode()` is not [`ApplicationMode::Simultaneous`].
/// - [`EngineError::UnsupportedContext`] if either context contains [`ContextElement::ZeroOrMore`].
/// - [`EngineError::UnknownSymbol`] if a [`ReplacementItem::Literal`] symbol is missing from `inventory`.
/// - [`EngineError::UnmappedFeatureBundle`] if a [`ReplacementItem::Mutate`] yields a bundle absent from `inventory`.
pub fn apply_rule(word: &Word, rule: &Rule, inventory: &Inventory) -> Result<Word, EngineError> {
    if rule.mode() != ApplicationMode::Simultaneous {
        return Err(EngineError::UnsupportedMode(rule.mode()));
    }
    if has_non_local_context(rule.left_context()) || has_non_local_context(rule.right_context()) {
        return Err(EngineError::UnsupportedContext {
            rule_id: rule.id().to_owned(),
        });
    }

    let snapshot = word.segments();
    let target = rule.target();
    let mut output = Vec::with_capacity(snapshot.len());

    if target.is_empty() {
        apply_epenthesis(snapshot, rule, inventory, &mut output)?;
        return Ok(Word::new(output));
    }

    let mut pos = 0;
    while pos < snapshot.len() {
        if pos + target.len() <= snapshot.len()
            && matches_target(&snapshot[pos..pos + target.len()], target)
            && matches_left_context(snapshot, pos, rule.left_context())
            && matches_right_context(snapshot, pos + target.len(), rule.right_context())
        {
            let matched = &snapshot[pos..pos + target.len()];
            append_replacement(&mut output, rule, matched, inventory)?;
            pos += target.len();
        } else {
            output.push(snapshot[pos].clone());
            pos += 1;
        }
    }

    Ok(Word::new(output))
}

/// Applies an ordered sequence of `rules` to `initial`, returning an immutable [`DerivationTrace`].
///
/// Vacuous matches where `word_after == word_before` are omitted from [`DerivationTrace::steps`].
///
/// # Errors
///
/// Propagates any [`EngineError`] produced while evaluating a rule.
pub fn derive(
    initial: &Word,
    rules: &[Rule],
    inventory: &Inventory,
) -> Result<DerivationTrace, EngineError> {
    let mut current = initial.clone();
    let mut steps = Vec::new();

    for rule in rules {
        let next = apply_rule(&current, rule, inventory)?;
        if next != current {
            steps.push(DerivationStep::new(
                rule.id(),
                rule.name(),
                current,
                next.clone(),
            ));
            current = next;
        }
    }

    Ok(DerivationTrace::new(initial.clone(), steps, current))
}

fn has_non_local_context(context: &[ContextElement]) -> bool {
    context
        .iter()
        .any(|elem| matches!(elem, ContextElement::ZeroOrMore(_)))
}

fn apply_epenthesis(
    snapshot: &[Segment],
    rule: &Rule,
    inventory: &Inventory,
    output: &mut Vec<Segment>,
) -> Result<(), EngineError> {
    for pos in 0..=snapshot.len() {
        if matches_left_context(snapshot, pos, rule.left_context())
            && matches_right_context(snapshot, pos, rule.right_context())
        {
            append_replacement(output, rule, &[], inventory)?;
        }
        if let Some(seg) = snapshot.get(pos) {
            output.push(seg.clone());
        }
    }
    Ok(())
}

fn matches_target(slice: &[Segment], target: &[Pattern]) -> bool {
    slice.len() == target.len()
        && slice
            .iter()
            .zip(target.iter())
            .all(|(seg, pat)| pat.matches(seg))
}

fn matches_left_context(snapshot: &[Segment], start: usize, left: &[ContextElement]) -> bool {
    let mut idx = start;
    let mut at_boundary = false;

    for elem in left.iter().rev() {
        if at_boundary {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if idx != 0 {
                    return false;
                }
                at_boundary = true;
            }
            ContextElement::Segment(pat) => {
                if idx == 0 {
                    return false;
                }
                idx -= 1;
                if !pat.matches(&snapshot[idx]) {
                    return false;
                }
            }
            ContextElement::ZeroOrMore(_) => return false,
        }
    }

    true
}

fn matches_right_context(snapshot: &[Segment], end: usize, right: &[ContextElement]) -> bool {
    let mut idx = end;
    let mut at_boundary = false;

    for elem in right {
        if at_boundary {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if idx != snapshot.len() {
                    return false;
                }
                at_boundary = true;
            }
            ContextElement::Segment(pat) => {
                let Some(seg) = snapshot.get(idx) else {
                    return false;
                };
                if !pat.matches(seg) {
                    return false;
                }
                idx += 1;
            }
            ContextElement::ZeroOrMore(_) => return false,
        }
    }

    true
}

fn append_replacement(
    output: &mut Vec<Segment>,
    rule: &Rule,
    matched: &[Segment],
    inventory: &Inventory,
) -> Result<(), EngineError> {
    for item in rule.replacement() {
        match item {
            ReplacementItem::Literal(symbol) => {
                let seg = inventory
                    .segment(symbol, Stress::Unstressed)
                    .map_err(|_| EngineError::UnknownSymbol(symbol.clone()))?;
                output.push(seg);
            }
            ReplacementItem::Mutate {
                source_index,
                mutation,
            } => {
                let source = matched.get(*source_index).ok_or_else(|| {
                    EngineError::InvalidRule(RuleError::InvalidSourceIndex {
                        rule_id: rule.id().to_owned(),
                        source_index: *source_index,
                        target_len: rule.target().len(),
                    })
                })?;
                let new_bundle = mutation.apply(&source.bundle);
                let new_symbol = inventory
                    .resolve(&new_bundle)
                    .ok_or(EngineError::UnmappedFeatureBundle(new_bundle))?;
                output.push(Segment::new(new_symbol, new_bundle, source.stress));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::{
        Backness, FeatureBundle, FeatureMutation, FeaturePattern, Height, Manner, Place,
    };

    fn test_inventory() -> Result<Inventory, crate::phonology::InventoryError> {
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "a:",
            FeatureBundle::vowel(Height::Low, Backness::Central, false).with_long(true),
        )?;
        inv.insert(
            "e",
            FeatureBundle::vowel(Height::MidHigh, Backness::Front, false),
        )?;
        inv.insert(
            "i",
            FeatureBundle::vowel(Height::High, Backness::Front, false),
        )?;
        inv.insert(
            "o",
            FeatureBundle::vowel(Height::MidHigh, Backness::Back, true),
        )?;
        inv.insert(
            "u",
            FeatureBundle::vowel(Height::High, Backness::Back, true),
        )?;
        inv.insert(
            "j",
            FeatureBundle::consonant(Manner::Approximant, Place::Palatal, true),
        )?;
        inv.insert(
            "p",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false),
        )?;
        inv.insert(
            "b",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true),
        )?;
        inv.insert(
            "t",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
        )?;
        inv.insert(
            "d",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, true),
        )?;
        inv.insert(
            "f",
            FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, false),
        )?;
        inv.insert(
            "s",
            FeatureBundle::consonant(Manner::Fricative, Place::Alveolar, false),
        )?;
        inv.insert(
            "m",
            FeatureBundle::consonant(Manner::Nasal, Place::Bilabial, true),
        )?;
        inv.insert(
            "n",
            FeatureBundle::consonant(Manner::Nasal, Place::Alveolar, true),
        )?;
        inv.insert(
            "r",
            FeatureBundle::consonant(Manner::Rhotic, Place::Alveolar, true),
        )?;
        Ok(inv)
    }

    #[test]
    fn deletion_1_to_0_removes_final_m() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let rule = Rule::new(
            "R_DEL",
            "Final m deletion",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;
        let word = Word::from_spaced("a m a m", &inv)?;
        let out = apply_rule(&word, &rule, &inv)?;
        assert_eq!(out.to_spaced(), "a m a");
        Ok(())
    }

    #[test]
    fn epenthesis_0_to_1_supports_initial_medial_and_final_contexts()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let prothesis = Rule::new(
            "R_PROTH",
            "Initial e-prothesis before s+stop",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::WordBoundary],
            vec![
                ContextElement::Segment(Pattern::Literal("s".to_owned())),
                ContextElement::Segment(Pattern::Features(
                    FeaturePattern::new().with_manner(Manner::Stop),
                )),
            ],
            ApplicationMode::Simultaneous,
        )?;
        let medial = Rule::new(
            "R_MED",
            "Insert b between m and r",
            vec![],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("m".to_owned()))],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        let paragoge = Rule::new(
            "R_FIN",
            "Insert a after final r",
            vec![],
            vec![ReplacementItem::Literal("a".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("r".to_owned()))],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;

        assert_eq!(
            apply_rule(&Word::from_spaced("s p a t a", &inv)?, &prothesis, &inv)?.to_spaced(),
            "e s p a t a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a m r a", &inv)?, &medial, &inv)?.to_spaced(),
            "a m b r a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("m a r", &inv)?, &paragoge, &inv)?.to_spaced(),
            "m a r a"
        );
        Ok(())
    }

    #[test]
    fn diphthongization_1_to_2_and_compensatory_lengthening_2_to_1()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let diphthongize = Rule::new(
            "R_DIPH",
            "e diphthongization",
            vec![Pattern::Literal("e".to_owned())],
            vec![
                ReplacementItem::Literal("j".to_owned()),
                ReplacementItem::Mutate {
                    source_index: 0,
                    mutation: FeatureMutation::new(),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let stressed_e = Word::new(vec![
            inv.segment("t", Stress::Unstressed)?,
            inv.segment("e", Stress::Primary)?,
            inv.segment("r", Stress::Unstressed)?,
        ]);
        let diph_out = apply_rule(&stressed_e, &diphthongize, &inv)?;
        assert_eq!(diph_out.to_spaced(), "t j e r");
        assert_eq!(diph_out.segments()[1].stress, Stress::Unstressed);
        assert_eq!(diph_out.segments()[2].stress, Stress::Primary);

        let comp_len = Rule::new(
            "R_COMP",
            "Compensatory lengthening before s",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("n".to_owned()),
            ],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_long(true),
            }],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("s".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        let comp_out = apply_rule(&Word::from_spaced("m a n s a", &inv)?, &comp_len, &inv)?;
        assert_eq!(comp_out.to_spaced(), "m a: s a");
        Ok(())
    }

    #[test]
    fn metathesis_2_to_2_swaps_segments_and_preserves_stress()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let metathesis = Rule::new(
            "R_META",
            "a r -> r a metathesis",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("r".to_owned()),
            ],
            vec![
                ReplacementItem::Mutate {
                    source_index: 1,
                    mutation: FeatureMutation::new(),
                },
                ReplacementItem::Mutate {
                    source_index: 0,
                    mutation: FeatureMutation::new(),
                },
            ],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let input = Word::new(vec![
            inv.segment("p", Stress::Unstressed)?,
            inv.segment("a", Stress::Primary)?,
            inv.segment("r", Stress::Unstressed)?,
        ]);
        let out = apply_rule(&input, &metathesis, &inv)?;
        assert_eq!(out.to_spaced(), "p r a");
        assert_eq!(out.segments()[1].stress, Stress::Unstressed);
        assert_eq!(out.segments()[2].stress, Stress::Primary);
        Ok(())
    }

    #[test]
    fn simultaneous_snapshot_prevents_cascading_and_overlapping_matches()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let chain = Rule::new(
            "R_SNAP",
            "a -> b before a",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("a".to_owned()))],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &chain, &inv)?.to_spaced(),
            "b b a"
        );

        let degeminate = Rule::new(
            "R_DEGEM",
            "a a -> b",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degeminate, &inv)?.to_spaced(),
            "b a"
        );
        Ok(())
    }

    #[test]
    fn derive_excludes_vacuous_matches_from_trace() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let vacuous_stop_voicing = Rule::new(
            "R_VAC",
            "Voice any stop between vowels",
            vec![Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Stop),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_voiced(true),
            }],
            vec![ContextElement::Segment(Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Vowel),
            ))],
            vec![ContextElement::Segment(Pattern::Features(
                FeaturePattern::new().with_manner(Manner::Vowel),
            ))],
            ApplicationMode::Simultaneous,
        )?;
        let delete_final_m = Rule::new(
            "R_DEL_M",
            "Delete final m",
            vec![Pattern::Literal("m".to_owned())],
            vec![],
            vec![],
            vec![ContextElement::WordBoundary],
            ApplicationMode::Simultaneous,
        )?;

        // `b` is already a voiced stop, so `R_VAC` matches but produces no change.
        let word_already_voiced = Word::from_spaced("a b a m", &inv)?;
        let trace = derive(
            &word_already_voiced,
            &[vacuous_stop_voicing.clone(), delete_final_m],
            &inv,
        )?;
        assert_eq!(trace.final_word().to_spaced(), "a b a");
        assert_eq!(trace.fired_rule_ids(), vec!["R_DEL_M"]);

        // Pure vacuous derivation returns zero steps.
        let vacuous_only = derive(
            &Word::from_spaced("a b a", &inv)?,
            &[vacuous_stop_voicing],
            &inv,
        )?;
        assert!(vacuous_only.steps().is_empty());
        assert!(vacuous_only.fired_rule_ids().is_empty());
        assert_eq!(vacuous_only.initial(), vacuous_only.final_word());
        Ok(())
    }

    #[test]
    fn engine_errors_cover_unmapped_bundle_unknown_symbol_and_unsupported_features()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        // Voicing `f` produces voiced labiodental fricative (`v`), which is absent from `test_inventory`.
        let voice_f = Rule::new(
            "R_UNMAPPED",
            "Voice f",
            vec![Pattern::Literal("f".to_owned())],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_voiced(true),
            }],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        let unmapped = apply_rule(&Word::from_spaced("f a", &inv)?, &voice_f, &inv);
        assert_eq!(
            unmapped,
            Err(EngineError::UnmappedFeatureBundle(
                FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true)
            ))
        );

        let unknown_lit = Rule::new(
            "R_UNKNOWN",
            "Rewrite a to z",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("z".to_owned())],
            vec![],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a", &inv)?, &unknown_lit, &inv),
            Err(EngineError::UnknownSymbol("z".to_owned()))
        );

        for mode in [ApplicationMode::LeftToRight, ApplicationMode::RightToLeft] {
            let directional = Rule::new(
                "R_DIR",
                "Directional",
                vec![Pattern::Literal("a".to_owned())],
                vec![],
                vec![],
                vec![],
                mode,
            )?;
            assert_eq!(
                apply_rule(&Word::from_spaced("a", &inv)?, &directional, &inv),
                Err(EngineError::UnsupportedMode(mode))
            );
        }

        let nonlocal = Rule::new(
            "R_NONLOCAL",
            "Non-local context",
            vec![Pattern::Literal("a".to_owned())],
            vec![],
            vec![ContextElement::ZeroOrMore(Pattern::Literal("s".to_owned()))],
            vec![],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a", &inv)?, &nonlocal, &inv),
            Err(EngineError::UnsupportedContext {
                rule_id: "R_NONLOCAL".to_owned()
            })
        );
        Ok(())
    }
}
