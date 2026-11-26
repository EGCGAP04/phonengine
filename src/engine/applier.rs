//! Pure rule-application engine supporting Simultaneous, LeftToRight, RightToLeft,
//! and non-local `ZeroOrMore` contexts.

use crate::phonology::{Inventory, Segment, Stress, Word};

use super::error::{EngineError, RuleError};
use super::rule::{ApplicationMode, ContextElement, Pattern, ReplacementItem, Rule};
use super::trace::{DerivationStep, DerivationTrace};

/// Applies a single phonological `rule` to `word` using `inventory` without mutating inputs.
///
/// Supports [`ApplicationMode::Simultaneous`], [`ApplicationMode::LeftToRight`], and
/// [`ApplicationMode::RightToLeft`], as well as local and non-local (`ZeroOrMore`) contexts.
/// A [`ReplacementItem::Mutate`] keeps the stress of its source segment, and a
/// [`ReplacementItem::Literal`] inherits the stress of the matched segment at its own position
/// unless a `Mutate` already copies that segment.
///
/// # Errors
///
/// - [`EngineError::UnknownSymbol`] if a [`ReplacementItem::Literal`] symbol is missing from `inventory`.
/// - [`EngineError::UnmappedFeatureBundle`] if a [`ReplacementItem::Mutate`] yields a bundle absent from `inventory`.
///
/// Both errors carry the identifier of `rule`.
pub fn apply_rule(word: &Word, rule: &Rule, inventory: &Inventory) -> Result<Word, EngineError> {
    match rule.mode() {
        ApplicationMode::Simultaneous => apply_simultaneous(word, rule, inventory),
        ApplicationMode::LeftToRight => apply_left_to_right(word, rule, inventory),
        ApplicationMode::RightToLeft => apply_right_to_left(word, rule, inventory),
    }
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

fn apply_simultaneous(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let snapshot = word.segments();
    let target = rule.target();
    let mut output = Vec::with_capacity(snapshot.len());

    if target.is_empty() {
        for pos in 0..=snapshot.len() {
            if matches_left_context(snapshot, pos, rule.left_context())
                && matches_right_context(snapshot, pos, rule.right_context())
            {
                append_replacement(&mut output, rule, &[], inventory)?;
            }
            if let Some(seg) = snapshot.get(pos) {
                output.push(seg.clone());
            }
        }
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

fn apply_left_to_right(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let mut current = word.segments().to_vec();
    let target = rule.target();

    if target.is_empty() {
        let mut pos = 0;
        loop {
            let inserted_len = if matches_left_context(&current, pos, rule.left_context())
                && matches_right_context(&current, pos, rule.right_context())
            {
                let repl = build_replacement(rule, &[], inventory)?;
                let len = repl.len();
                current.splice(pos..pos, repl);
                len
            } else {
                0
            };
            if pos + inserted_len == current.len() {
                break;
            }
            pos += inserted_len + 1;
        }
        return Ok(Word::new(current));
    }

    let mut pos = 0;
    while pos + target.len() <= current.len() {
        if matches_target(&current[pos..pos + target.len()], target)
            && matches_left_context(&current, pos, rule.left_context())
            && matches_right_context(&current, pos + target.len(), rule.right_context())
        {
            let repl = build_replacement(rule, &current[pos..pos + target.len()], inventory)?;
            let repl_len = repl.len();
            current.splice(pos..pos + target.len(), repl);
            pos += repl_len;
        } else {
            pos += 1;
        }
    }

    Ok(Word::new(current))
}

fn apply_right_to_left(
    word: &Word,
    rule: &Rule,
    inventory: &Inventory,
) -> Result<Word, EngineError> {
    let mut current = word.segments().to_vec();
    let target = rule.target();

    if target.is_empty() {
        let mut pos = current.len();
        loop {
            if matches_left_context(&current, pos, rule.left_context())
                && matches_right_context(&current, pos, rule.right_context())
            {
                let repl = build_replacement(rule, &[], inventory)?;
                current.splice(pos..pos, repl);
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
        return Ok(Word::new(current));
    }

    let mut end = current.len();
    while end >= target.len() {
        let start = end - target.len();
        if matches_target(&current[start..end], target)
            && matches_left_context(&current, start, rule.left_context())
            && matches_right_context(&current, end, rule.right_context())
        {
            let repl = build_replacement(rule, &current[start..end], inventory)?;
            current.splice(start..end, repl);
            end = start;
        } else {
            end -= 1;
        }
    }

    Ok(Word::new(current))
}

fn matches_target(slice: &[Segment], target: &[Pattern]) -> bool {
    slice.len() == target.len()
        && slice
            .iter()
            .zip(target.iter())
            .all(|(seg, pat)| pat.matches(seg))
}

fn matches_left_context(snapshot: &[Segment], start: usize, left: &[ContextElement]) -> bool {
    if left.is_empty() {
        return true;
    }
    let n = snapshot.len();
    let mut reachable = vec![false; n + 1];
    reachable[start] = true;
    let mut boundary_reached = false;

    for elem in left.iter().rev() {
        if boundary_reached {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if !reachable[0] {
                    return false;
                }
                reachable.fill(false);
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next = vec![false; n + 1];
                let mut any = false;
                for idx in 1..=n {
                    if reachable[idx] && pat.matches(&snapshot[idx - 1]) {
                        next[idx - 1] = true;
                        any = true;
                    }
                }
                if !any {
                    return false;
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in (1..=n).rev() {
                    if reachable[idx] && pat.matches(&snapshot[idx - 1]) {
                        reachable[idx - 1] = true;
                    }
                }
            }
        }
    }

    boundary_reached || reachable.iter().any(|&flag| flag)
}

fn matches_right_context(snapshot: &[Segment], end: usize, right: &[ContextElement]) -> bool {
    if right.is_empty() {
        return true;
    }
    let n = snapshot.len();
    let mut reachable = vec![false; n + 1];
    reachable[end] = true;
    let mut boundary_reached = false;

    for elem in right {
        if boundary_reached {
            return false;
        }
        match elem {
            ContextElement::WordBoundary => {
                if !reachable[n] {
                    return false;
                }
                reachable.fill(false);
                boundary_reached = true;
            }
            ContextElement::Segment(pat) => {
                let mut next = vec![false; n + 1];
                let mut any = false;
                for idx in 0..n {
                    if reachable[idx] && pat.matches(&snapshot[idx]) {
                        next[idx + 1] = true;
                        any = true;
                    }
                }
                if !any {
                    return false;
                }
                reachable = next;
            }
            ContextElement::ZeroOrMore(pat) => {
                for idx in 0..n {
                    if reachable[idx] && pat.matches(&snapshot[idx]) {
                        reachable[idx + 1] = true;
                    }
                }
            }
        }
    }

    boundary_reached || reachable.iter().any(|&flag| flag)
}

fn build_replacement(
    rule: &Rule,
    matched: &[Segment],
    inventory: &Inventory,
) -> Result<Vec<Segment>, EngineError> {
    let mut built = Vec::with_capacity(rule.replacement().len());
    append_replacement(&mut built, rule, matched, inventory)?;
    Ok(built)
}

fn append_replacement(
    output: &mut Vec<Segment>,
    rule: &Rule,
    matched: &[Segment],
    inventory: &Inventory,
) -> Result<(), EngineError> {
    let replacement = rule.replacement();
    for (position, item) in replacement.iter().enumerate() {
        match item {
            ReplacementItem::Literal(symbol) => {
                let stress = inherited_stress(replacement, matched, position);
                let unknown_symbol = |_| EngineError::UnknownSymbol {
                    rule_id: rule.id().to_owned(),
                    symbol: symbol.clone(),
                };
                output.push(inventory.segment(symbol, stress).map_err(unknown_symbol)?);
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
                let new_symbol = inventory.resolve(&new_bundle).ok_or_else(|| {
                    EngineError::UnmappedFeatureBundle {
                        rule_id: rule.id().to_owned(),
                        bundle: new_bundle,
                    }
                })?;
                output.push(Segment::new(new_symbol, new_bundle, source.stress));
            }
        }
    }
    Ok(())
}

/// Returns the stress inherited by a literal at `position` of `replacement`.
///
/// That is the stress of `matched[position]`, unless no such segment exists (epenthesis or a
/// longer replacement) or a [`ReplacementItem::Mutate`] of the same replacement already carries
/// that segment's stress.
fn inherited_stress(
    replacement: &[ReplacementItem],
    matched: &[Segment],
    position: usize,
) -> Stress {
    let copied_by_mutation = replacement.iter().any(|item| {
        matches!(item, ReplacementItem::Mutate { source_index, .. } if *source_index == position)
    });
    match matched.get(position) {
        Some(source) if !copied_by_mutation => source.stress,
        _ => Stress::Unstressed,
    }
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
            "ph",
            FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false).with_aspirated(true),
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
            "th",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false).with_aspirated(true),
        )?;
        inv.insert(
            "d",
            FeatureBundle::consonant(Manner::Stop, Place::Alveolar, true),
        )?;
        inv.insert(
            "k",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false),
        )?;
        inv.insert(
            "kh",
            FeatureBundle::consonant(Manner::Stop, Place::Velar, false).with_aspirated(true),
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
    fn non_local_zero_or_more_supports_grassmann_boundaries_and_stacked_closures()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        // Grassmann's Law: [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]
        let grassmann = Rule::new(
            "R_GRASSMANN",
            "Grassmann's Law",
            vec![Pattern::Features(
                FeaturePattern::new()
                    .with_manner(Manner::Stop)
                    .with_aspirated(true),
            )],
            vec![ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_aspirated(false),
            }],
            vec![],
            vec![
                ContextElement::ZeroOrMore(Pattern::Features(
                    FeaturePattern::new().with_aspirated(false),
                )),
                ContextElement::Segment(Pattern::Features(
                    FeaturePattern::new().with_aspirated(true),
                )),
            ],
            ApplicationMode::LeftToRight,
        )?;

        assert_eq!(
            apply_rule(&Word::from_spaced("th i kh e", &inv)?, &grassmann, &inv)?.to_spaced(),
            "t i kh e"
        );
        // Three aspirates: LeftToRight deaspirates first and second, keeping the last.
        assert_eq!(
            apply_rule(&Word::from_spaced("ph a th e kh", &inv)?, &grassmann, &inv)?.to_spaced(),
            "p a t e kh"
        );
        // Single aspirate must remain aspirated.
        assert_eq!(
            apply_rule(&Word::from_spaced("th e o s", &inv)?, &grassmann, &inv)?.to_spaced(),
            "th e o s"
        );

        // Non-local left and right contexts combined with WordBoundary and stacked ZeroOrMore.
        let vowels = Pattern::Features(FeaturePattern::new().with_manner(Manner::Vowel));
        let boundary_and_stacked = Rule::new(
            "R_NONLOCAL_BOUND",
            "Delete t after # V* and before V* V* #",
            vec![Pattern::Literal("t".to_owned())],
            vec![],
            vec![
                ContextElement::WordBoundary,
                ContextElement::ZeroOrMore(vowels.clone()),
            ],
            vec![
                ContextElement::ZeroOrMore(vowels.clone()),
                ContextElement::ZeroOrMore(vowels),
                ContextElement::WordBoundary,
            ],
            ApplicationMode::Simultaneous,
        )?;
        assert_eq!(
            apply_rule(
                &Word::from_spaced("a e t o u", &inv)?,
                &boundary_and_stacked,
                &inv
            )?
            .to_spaced(),
            "a e o u"
        );
        // Consonant before `#` blocks right context `V* V* #`.
        assert_eq!(
            apply_rule(
                &Word::from_spaced("a e t o s", &inv)?,
                &boundary_and_stacked,
                &inv
            )?
            .to_spaced(),
            "a e t o s"
        );
        Ok(())
    }

    #[test]
    fn directional_modes_feed_bleed_and_terminate_for_n_to_m_and_epenthesis()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

        // LeftToRight progressive feeding: `a > e / e C* _` across consonants.
        let non_vowels = Pattern::Features(FeaturePattern::new().with_syllabic(false));
        let prog_harmony = Rule::new(
            "R_PROG",
            "Progressive vowel harmony",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![
                ContextElement::Segment(Pattern::Literal("e".to_owned())),
                ContextElement::ZeroOrMore(non_vowels.clone()),
            ],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("e t a k a", &inv)?, &prog_harmony, &inv)?.to_spaced(),
            "e t e k e"
        );

        // RightToLeft regressive feeding: `a > e / _ C* e`.
        let reg_harmony = Rule::new(
            "R_REG",
            "Regressive vowel harmony",
            vec![Pattern::Literal("a".to_owned())],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![
                ContextElement::ZeroOrMore(non_vowels),
                ContextElement::Segment(Pattern::Literal("e".to_owned())),
            ],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a t a k e", &inv)?, &reg_harmony, &inv)?.to_spaced(),
            "e t e k e"
        );

        // RightToLeft vs LeftToRight on overlapping `a a > b` over `"a a a"`.
        let degem_ltr = Rule::new(
            "R_LTR",
            "Degeminate LTR",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        let degem_rtl = Rule::new(
            "R_RTL",
            "Degeminate RTL",
            vec![
                Pattern::Literal("a".to_owned()),
                Pattern::Literal("a".to_owned()),
            ],
            vec![ReplacementItem::Literal("b".to_owned())],
            vec![],
            vec![],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degem_ltr, &inv)?.to_spaced(),
            "b a"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("a a a", &inv)?, &degem_rtl, &inv)?.to_spaced(),
            "a b"
        );

        // LeftToRight 1 -> 2 (`e > e a`) must not re-scan its own emitted `e`.
        let diph_ltr = Rule::new(
            "R_DIPH_LTR",
            "Non-looping 1->2",
            vec![Pattern::Literal("e".to_owned())],
            vec![
                ReplacementItem::Literal("e".to_owned()),
                ReplacementItem::Literal("a".to_owned()),
            ],
            vec![],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t e t e", &inv)?, &diph_ltr, &inv)?.to_spaced(),
            "t e a t e a"
        );

        // Directional epenthesis (0 -> 1) in both LeftToRight and RightToLeft.
        let epen_ltr = Rule::new(
            "R_EPEN_LTR",
            "Insert e after t",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![ContextElement::Segment(Pattern::Literal("t".to_owned()))],
            vec![],
            ApplicationMode::LeftToRight,
        )?;
        let epen_rtl = Rule::new(
            "R_EPEN_RTL",
            "Insert e before t",
            vec![],
            vec![ReplacementItem::Literal("e".to_owned())],
            vec![],
            vec![ContextElement::Segment(Pattern::Literal("t".to_owned()))],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            apply_rule(&Word::from_spaced("t s t", &inv)?, &epen_ltr, &inv)?.to_spaced(),
            "t e s t e"
        );
        assert_eq!(
            apply_rule(&Word::from_spaced("t s t", &inv)?, &epen_rtl, &inv)?.to_spaced(),
            "e t s e t"
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

        let word_already_voiced = Word::from_spaced("a b a m", &inv)?;
        let trace = derive(
            &word_already_voiced,
            &[vacuous_stop_voicing.clone(), delete_final_m],
            &inv,
        )?;
        assert_eq!(trace.final_word().to_spaced(), "a b a");
        assert_eq!(trace.fired_rule_ids(), vec!["R_DEL_M"]);

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
    fn engine_errors_cover_unmapped_bundle_and_unknown_symbol()
    -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;

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
            Err(EngineError::UnmappedFeatureBundle {
                rule_id: "R_UNMAPPED".to_owned(),
                bundle: FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true),
            })
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
            Err(EngineError::UnknownSymbol {
                rule_id: "R_UNKNOWN".to_owned(),
                symbol: "z".to_owned(),
            })
        );
        Ok(())
    }

    #[test]
    fn literal_replacements_inherit_stress_by_position() -> Result<(), Box<dyn std::error::Error>> {
        let inv = test_inventory()?;
        let lit = |sym: &str| ReplacementItem::Literal(sym.to_owned());
        let pat = |sym: &str| Pattern::Literal(sym.to_owned());
        let copy = |source_index: usize| ReplacementItem::Mutate {
            source_index,
            mutation: FeatureMutation::new(),
        };
        let rule = |target: Vec<Pattern>, replacement: Vec<ReplacementItem>| {
            Rule::new(
                "R_STRESS",
                "Stress inheritance",
                target,
                replacement,
                vec![],
                vec![],
                ApplicationMode::Simultaneous,
            )
        };
        let stresses = |word: &Word| -> Vec<(String, Stress)> {
            word.segments()
                .iter()
                .map(|seg| (seg.symbol.clone(), seg.stress))
                .collect()
        };
        let expect = |pairs: &[(&str, Stress)]| -> Vec<(String, Stress)> {
            pairs.iter().map(|(s, st)| ((*s).to_owned(), *st)).collect()
        };
        let (u, p) = (Stress::Unstressed, Stress::Primary);
        let p_stressed_o = Word::new(vec![inv.segment("p", u)?, inv.segment("o", p)?]);
        let t_stressed_e_r = Word::new(vec![
            inv.segment("t", u)?,
            inv.segment("e", p)?,
            inv.segment("r", u)?,
        ]);
        let p_stressed_a_r = Word::new(vec![
            inv.segment("p", u)?,
            inv.segment("a", p)?,
            inv.segment("r", u)?,
        ]);

        // 1 -> 1: `o > u` keeps the stress of `o`.
        let raise = rule(vec![pat("o")], vec![lit("u")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &raise, &inv)?),
            expect(&[("p", u), ("u", p)])
        );

        // 2 -> 2 literal: stress stays on its position, unlike `$2 $1` metathesis.
        let literal_swap = rule(vec![pat("a"), pat("r")], vec![lit("r"), lit("a")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_a_r, &literal_swap, &inv)?),
            expect(&[("p", u), ("r", p), ("a", u)])
        );

        // 1 -> 2: `e > i e` stresses position 0; `e > i $1` keeps it on the copied `e`.
        let diph_literal = rule(vec![pat("e")], vec![lit("i"), lit("e")])?;
        assert_eq!(
            stresses(&apply_rule(&t_stressed_e_r, &diph_literal, &inv)?),
            expect(&[("t", u), ("i", p), ("e", u), ("r", u)])
        );
        let diph_copy = rule(vec![pat("e")], vec![lit("i"), copy(0)])?;
        assert_eq!(
            stresses(&apply_rule(&t_stressed_e_r, &diph_copy, &inv)?),
            expect(&[("t", u), ("i", u), ("e", p), ("r", u)])
        );

        // 2 -> 1: `a r > o` inherits the stress of the first matched segment.
        let merge = rule(vec![pat("a"), pat("r")], vec![lit("o")])?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_a_r, &merge, &inv)?),
            expect(&[("p", u), ("o", p)])
        );

        // Epenthesis has no matched segment: the inserted literal is unstressed.
        let prothesis = Rule::new(
            "R_PROTH_STRESS",
            "Prothesis before p",
            vec![],
            vec![lit("e")],
            vec![ContextElement::WordBoundary],
            vec![ContextElement::Segment(pat("p"))],
            ApplicationMode::LeftToRight,
        )?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &prothesis, &inv)?),
            expect(&[("e", u), ("p", u), ("o", p)])
        );

        // Directional modes share the same replacement builder.
        let raise_rtl = Rule::new(
            "R_RAISE_RTL",
            "o > u (right-to-left)",
            vec![pat("o")],
            vec![lit("u")],
            vec![],
            vec![],
            ApplicationMode::RightToLeft,
        )?;
        assert_eq!(
            stresses(&apply_rule(&p_stressed_o, &raise_rtl, &inv)?),
            expect(&[("p", u), ("u", p)])
        );
        Ok(())
    }
}
