//! Integration tests for Milestone 2: DSL parsing and maximal-munch tokenization.

use phonengine::{
    ApplicationMode, Backness, ContextElement, DslError, DslErrorKind, FeatureBundle, Height,
    Inventory, Manner, Place, Stress, apply_rule, derive, parse_program, tokenize_word,
};

const GOLDEN_MVP_TSV: &str = include_str!("fixtures/golden_mvp.tsv");

const MVP_DSL: &str = r#"
    // MVP segment inventory (Classical Latin -> simplified Spanish)
    segment a = [vowel, low, central]
    segment e = [vowel, mid-high, front]
    segment i = [vowel, high, front]
    segment o = [vowel, mid-high, back, +rounded]
    segment u = [vowel, high, back, +rounded]

    segment p = [stop, bilabial, -voiced]
    segment b = [stop, bilabial, +voiced]
    segment t = [stop, alveolar, -voiced]
    segment d = [stop, alveolar, +voiced]
    segment k = [stop, velar, -voiced]
    segment g = [stop, velar, +voiced]

    segment f = [fricative, labiodental, -voiced]
    segment v = [fricative, labiodental, +voiced]
    segment s = [fricative, alveolar, -voiced]
    segment h = [fricative, glottal, -voiced]

    segment m = [nasal, bilabial, +voiced]
    segment n = [nasal, alveolar, +voiced]
    segment l = [lateral, alveolar, +voiced]
    segment r = [rhotic, alveolar, +voiced]

    class V = { a, e, i, o, u }

    rule R1 "Final -m deletion":
      m > ∅ / _ #

    rule R2 "Final -u lowering":
      u > o / _ #

    rule R3 "Intervocalic stop voicing" (simultaneous):
      [-voiced, +stop] > [+voiced] / V _ V

    rule R4 "Initial f- aspiration before vowel":
      f > h / # _ V
"#;

#[test]
fn dsl_mvp_and_maximal_munch_match_golden_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let program = parse_program(MVP_DSL)?;
    program.validate_against(&program.inventory)?;

    let mut rows_checked = 0usize;
    for (line_idx, raw_line) in GOLDEN_MVP_TSV.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let cols: Vec<&str> = raw_line.split('\t').collect();
        assert_eq!(
            cols.len(),
            4,
            "line {} in golden_mvp.tsv must have 4 columns",
            line_idx + 1
        );

        let unspaced_input: String = cols[0].split_whitespace().collect();
        let expected_output = cols[1];
        let expected_rules: Vec<&str> = if cols[2].is_empty() {
            Vec::new()
        } else {
            cols[2].split(',').collect()
        };
        let gloss = cols[3];

        let word = tokenize_word(&unspaced_input, &program.inventory)?;
        let trace = derive(&word, &program.rules, &program.inventory)?;

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

#[test]
fn dsl_supports_n_to_m_rewrites_and_ipa_stress_preservation()
-> Result<(), Box<dyn std::error::Error>> {
    let dsl = r#"
        segment a = [vowel, low, central]
        segment aː = [vowel, low, central, +long]
        segment e = [vowel, mid-high, front]
        segment i = [vowel, high, front]
        segment r = [rhotic, alveolar, +voiced]
        segment t = [stop, alveolar, -voiced]
        segment s = [fricative, alveolar, -voiced]
        segment p = [stop, bilabial, -voiced]

        class V = { a, aː, e, i }
        class C = { r, t, s, p }

        // 0 -> 1 prothesis
        rule prothesis:
          ∅ > e / # _ s

        // 2 -> 2 metathesis preserving stress
        rule metathesis:
          r t > $2 $1

        // 2 -> 1 compensatory lengthening with multi-char symbol
        rule lengthening:
          V s > $1[+long] / _ C

        // 1 -> 2 diphthongization
        rule diphthong:
          e > i e / p _
    "#;

    let program = parse_program(dsl)?;
    program.validate_against(&program.inventory)?;

    // Prothesis + compensatory lengthening preserving primary stress on `a` -> `aː`
    let w1 = tokenize_word("sˈast", &program.inventory)?;
    let trace1 = derive(&w1, &program.rules, &program.inventory)?;
    assert_eq!(trace1.final_word().to_spaced(), "e s aː t");
    assert_eq!(trace1.final_word().segments()[2].stress, Stress::Primary);
    assert_eq!(trace1.fired_rule_ids(), vec!["prothesis", "lengthening"]);

    // Metathesis 2 -> 2 preserving stress on `t`
    let w2 = tokenize_word("paˈrt", &program.inventory)?;
    let trace2 = derive(&w2, &program.rules, &program.inventory)?;
    assert_eq!(trace2.final_word().to_spaced(), "p a t r");
    assert_eq!(trace2.final_word().segments()[2].stress, Stress::Unstressed);
    assert_eq!(trace2.final_word().segments()[3].stress, Stress::Primary);

    // Diphthongization 1 -> 2
    let w3 = tokenize_word("pe", &program.inventory)?;
    let trace3 = derive(&w3, &program.rules, &program.inventory)?;
    assert_eq!(trace3.final_word().to_spaced(), "p i e");
    Ok(())
}

#[test]
fn rules_only_dsl_validates_against_external_inventory_and_defers_m3_features()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inv = Inventory::new();
    inv.insert(
        "a",
        FeatureBundle::vowel(Height::Low, Backness::Central, false),
    )?;
    inv.insert(
        "e",
        FeatureBundle::vowel(Height::MidHigh, Backness::Front, false),
    )?;
    inv.insert(
        "t",
        FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false),
    )?;

    let rules_only = r#"
        class V = { a, e }
        rule non_local:
          a > e / V* _ #
        rule directional (left-to-right):
          e > a
    "#;
    let program = parse_program(rules_only)?;
    assert!(program.inventory.is_empty());
    assert!(program.dialect_tree.is_none());
    program.validate_against(&inv)?;

    assert_eq!(
        program.rules[0].left_context(),
        &[ContextElement::ZeroOrMore(phonengine::Pattern::Class(
            program.classes["V"].clone()
        ))]
    );
    assert_eq!(program.rules[1].mode(), ApplicationMode::LeftToRight);

    let word = tokenize_word("ta", &inv)?;
    assert_eq!(
        apply_rule(&word, &program.rules[0], &inv)?.to_spaced(),
        "t e"
    );
    assert_eq!(
        apply_rule(&tokenize_word("ete", &inv)?, &program.rules[1], &inv)?.to_spaced(),
        "a t a"
    );

    let invalid = parse_program("rule bad:\n  a > z\n")?;
    assert!(matches!(
        invalid.validate_against(&inv),
        Err(DslError {
            kind: DslErrorKind::UnknownSymbol(sym),
            ..
        }) if sym == "z"
    ));
    Ok(())
}

#[test]
fn parses_and_validates_complete_spec_example() -> Result<(), Box<dyn std::error::Error>> {
    let spec_example = r#"
        // Inventario mínimo
        segment p = [stop, bilabial, -voiced]
        segment b = [stop, bilabial, +voiced]
        segment t = [stop, alveolar, -voiced]
        segment r = [rhotic, alveolar, +voiced]
        segment s = [fricative, alveolar, -voiced]
        segment a = [vowel, low, central]
        segment aː = [vowel, low, central, +long]
        segment e = [vowel, mid-high, front]
        segment m = [nasal, bilabial, +voiced]

        class V = { a, aː, e }
        class C = { p, b, t, r, s, m }

        rule final_m_loss "Final -m deletion":
          m > ∅ / _ #

        rule intervocalic_voicing (simultaneous):
          [-voiced, +stop] > [+voiced] / V _ V

        // Regla 2 -> 2 (metátesis)
        rule metathesis_rt:
          r t > $2 $1

        // Regla 2 -> 1 (alargamiento compensatorio)
        rule comp_lengthening:
          V s > $1[+long] / _ C
    "#;

    let program = parse_program(spec_example)?;
    program.validate_against(&program.inventory)?;
    assert_eq!(program.inventory.len(), 9);
    assert_eq!(program.classes.len(), 2);
    assert_eq!(program.rules.len(), 4);
    assert_eq!(
        program.rules.iter().map(|r| r.id()).collect::<Vec<&str>>(),
        vec![
            "final_m_loss",
            "intervocalic_voicing",
            "metathesis_rt",
            "comp_lengthening",
        ]
    );
    Ok(())
}
