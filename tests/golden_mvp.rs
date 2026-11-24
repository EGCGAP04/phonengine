//! Golden integration test suite verifying Milestone 1 against `tests/fixtures/golden_mvp.tsv`.

use phonengine::{
    ApplicationMode, Backness, ContextElement, FeatureBundle, FeatureMutation, FeaturePattern,
    Height, Inventory, InventoryError, Manner, NaturalClass, Pattern, Place, ReplacementItem, Rule,
    RuleError, Word, derive,
};

const GOLDEN_MVP_TSV: &str = include_str!("fixtures/golden_mvp.tsv");

fn mvp_inventory() -> Result<Inventory, InventoryError> {
    let mut inv = Inventory::new();

    // Vowels
    inv.insert(
        "a",
        FeatureBundle::vowel(Height::Low, Backness::Central, false),
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

    // Stops
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
        "k",
        FeatureBundle::consonant(Manner::Stop, Place::Velar, false),
    )?;
    inv.insert(
        "g",
        FeatureBundle::consonant(Manner::Stop, Place::Velar, true),
    )?;

    // Fricatives
    inv.insert(
        "f",
        FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, false),
    )?;
    inv.insert(
        "v",
        FeatureBundle::consonant(Manner::Fricative, Place::Labiodental, true),
    )?;
    inv.insert(
        "s",
        FeatureBundle::consonant(Manner::Fricative, Place::Alveolar, false),
    )?;
    inv.insert(
        "h",
        FeatureBundle::consonant(Manner::Fricative, Place::Glottal, false),
    )?;

    // Sonorants
    inv.insert(
        "m",
        FeatureBundle::consonant(Manner::Nasal, Place::Bilabial, true),
    )?;
    inv.insert(
        "n",
        FeatureBundle::consonant(Manner::Nasal, Place::Alveolar, true),
    )?;
    inv.insert(
        "l",
        FeatureBundle::consonant(Manner::Lateral, Place::Alveolar, true),
    )?;
    inv.insert(
        "r",
        FeatureBundle::consonant(Manner::Rhotic, Place::Alveolar, true),
    )?;

    Ok(inv)
}

fn mvp_rules() -> Result<Vec<Rule>, RuleError> {
    let vowels = NaturalClass::new("V", ["a", "e", "i", "o", "u"]);
    let vowel_ctx = ContextElement::Segment(Pattern::Class(vowels));

    // R1: m > ∅ / _ #
    let r1 = Rule::new(
        "R1",
        "Final -m deletion",
        vec![Pattern::Literal("m".to_owned())],
        vec![],
        vec![],
        vec![ContextElement::WordBoundary],
        ApplicationMode::Simultaneous,
    )?;

    // R2: u > o / _ #
    let r2 = Rule::new(
        "R2",
        "Final -u lowering",
        vec![Pattern::Literal("u".to_owned())],
        vec![ReplacementItem::Literal("o".to_owned())],
        vec![],
        vec![ContextElement::WordBoundary],
        ApplicationMode::Simultaneous,
    )?;

    // R3: [-voiced, +stop] > [+voiced] / V _ V
    let r3 = Rule::new(
        "R3",
        "Intervocalic stop voicing",
        vec![Pattern::Features(
            FeaturePattern::new()
                .with_manner(Manner::Stop)
                .with_voiced(false),
        )],
        vec![ReplacementItem::Mutate {
            source_index: 0,
            mutation: FeatureMutation::new().set_voiced(true),
        }],
        vec![vowel_ctx.clone()],
        vec![vowel_ctx.clone()],
        ApplicationMode::Simultaneous,
    )?;

    // R4: f > h / # _ V
    let r4 = Rule::new(
        "R4",
        "Initial f- aspiration before vowel",
        vec![Pattern::Literal("f".to_owned())],
        vec![ReplacementItem::Literal("h".to_owned())],
        vec![ContextElement::WordBoundary],
        vec![vowel_ctx],
        ApplicationMode::Simultaneous,
    )?;

    Ok(vec![r1, r2, r3, r4])
}

#[test]
fn golden_mvp_fixture_matches_output_and_fired_rules() -> Result<(), Box<dyn std::error::Error>> {
    let inventory = mvp_inventory()?;
    let rules = mvp_rules()?;

    let mut rows_checked = 0usize;
    for (line_idx, raw_line) in GOLDEN_MVP_TSV.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let cols: Vec<&str> = raw_line.split('\t').collect();
        assert_eq!(
            cols.len(),
            4,
            "line {} in golden_mvp.tsv must have 4 tab-separated columns",
            line_idx + 1
        );

        let input_segments = cols[0];
        let expected_output = cols[1];
        let expected_rules: Vec<&str> = if cols[2].is_empty() {
            Vec::new()
        } else {
            cols[2].split(',').collect()
        };
        let gloss = cols[3];

        let word = Word::from_spaced(input_segments, &inventory)?;
        let trace = derive(&word, &rules, &inventory)?;

        assert_eq!(
            trace.final_word().to_spaced(),
            expected_output,
            "unexpected output for {gloss} (line {})",
            line_idx + 1
        );
        assert_eq!(
            trace.fired_rule_ids(),
            expected_rules,
            "unexpected fired rules for {gloss} (line {})",
            line_idx + 1
        );

        rows_checked += 1;
    }

    assert!(
        rows_checked >= 8,
        "expected at least 8 golden rows in golden_mvp.tsv, checked {rows_checked}"
    );
    Ok(())
}
