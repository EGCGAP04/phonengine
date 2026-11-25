//! Integration tests for Milestone 3: dialect trees, directional modes,
//! non-local `ZeroOrMore` contexts, and DSL `stage` declarations.

use std::collections::BTreeMap;

use phonengine::{DslError, DslErrorKind, EngineError, Stress, parse_program, tokenize_word};

const HELLENIC_DSL: &str = r#"
    segment p = [stop, bilabial, -voiced]
    segment pʰ = [stop, bilabial, -voiced, +aspirated]
    segment t = [stop, alveolar, -voiced]
    segment tʰ = [stop, alveolar, -voiced, +aspirated]
    segment k = [stop, velar, -voiced]
    segment kʰ = [stop, velar, -voiced, +aspirated]
    segment s = [fricative, alveolar, -voiced]
    segment h = [fricative, glottal, -voiced]
    segment r = [rhotic, alveolar, +voiced]
    segment n = [nasal, alveolar, +voiced]

    segment a = [vowel, low, central]
    segment aː = [vowel, low, central, +long]
    segment e = [vowel, mid-high, front]
    segment eː = [vowel, mid-high, front, +long]
    segment i = [vowel, high, front]
    segment o = [vowel, mid-high, back, +rounded]
    segment u = [vowel, high, back, +rounded]

    class V = { a, aː, e, eː, i, o, u }
    class C = { p, pʰ, t, tʰ, k, kʰ, s, h, r, n }

    stage pie "Proto-Indo-European" {}

    stage proto_greek "Proto-Greek" from pie {
      rule s_debuccalization:
        s > h / # _ V

      // Non-local Grassmann's Law using LeftToRight + ZeroOrMore
      rule grassmann "Grassmann's Law" (left-to-right):
        [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]
    }

    stage attic "Attic Greek" from proto_greek {
      rule a_raising:
        aː > $1[mid-high, front]

      rule alpha_purum "Attic alpha purum":
        eː > $1[low, central] / r _
    }

    stage ionic "Ionic Greek" from proto_greek {
      rule a_raising:
        aː > $1[mid-high, front]
    }
"#;

#[test]
fn parses_and_derives_hellenic_dialect_tree_with_stress_and_grassmann()
-> Result<(), Box<dyn std::error::Error>> {
    let program = parse_program(HELLENIC_DSL)?;
    program.validate_against(&program.inventory)?;
    assert!(program.rules.is_empty());

    let tree = program
        .dialect_tree
        .as_ref()
        .ok_or("expected dialect tree from stage declarations")?;

    // PIE *tʰeˈkʰaːs -> Proto-Greek teˈkʰaːs -> Attic teˈkʰeːs, Ionic teˈkʰeːs
    let input1 = tokenize_word("tʰeˈkʰaːs", &program.inventory)?;
    let full1 = tree.derive_tree(&input1, &program.inventory)?;

    assert_eq!(full1.root_id(), "pie");
    assert_eq!(
        full1
            .word_at("proto_greek")
            .map(|w| w.to_spaced())
            .as_deref(),
        Some("t e kʰ aː s")
    );
    assert_eq!(
        full1.word_at("attic").map(|w| w.to_spaced()).as_deref(),
        Some("t e kʰ eː s")
    );
    assert_eq!(
        full1.word_at("ionic").map(|w| w.to_spaced()).as_deref(),
        Some("t e kʰ eː s")
    );
    // Primary stress on `kʰ` is preserved through Proto-Greek & daughter dialects.
    let attic_w1 = full1.word_at("attic").ok_or("missing attic word")?;
    assert_eq!(attic_w1.segments()[2].stress, Stress::Primary);

    // PIE *serˈaː -> Proto-Greek herˈaː -> Attic herˈaː (alpha purum), Ionic herˈeː
    let input2 = tokenize_word("serˈaː", &program.inventory)?;
    let full2 = tree.derive_tree(&input2, &program.inventory)?;
    let leaves2: BTreeMap<&str, String> = full2
        .leaf_outcomes()
        .into_iter()
        .map(|(k, w)| (k, w.to_spaced()))
        .collect();
    assert_eq!(leaves2.get("attic").map(String::as_str), Some("h e r aː"));
    assert_eq!(leaves2.get("ionic").map(String::as_str), Some("h e r eː"));
    let attic_w2 = full2.word_at("attic").ok_or("missing attic word")?;
    assert_eq!(attic_w2.segments()[3].stress, Stress::Primary);

    // Verify `derive_stage` path and unknown stage error.
    let single = tree.derive_stage(&input2, "ionic", &program.inventory)?;
    assert_eq!(single.final_word().to_spaced(), "h e r eː");
    assert_eq!(
        single
            .stages()
            .iter()
            .map(|s| s.stage_id())
            .collect::<Vec<&str>>(),
        vec!["pie", "proto_greek", "ionic"]
    );
    assert_eq!(
        tree.derive_stage(&input2, "aeolic", &program.inventory),
        Err(EngineError::UnknownStage("aeolic".to_owned()))
    );
    Ok(())
}

#[test]
fn directional_modes_diverge_across_sister_dialects_in_dsl()
-> Result<(), Box<dyn std::error::Error>> {
    let dsl = r#"
        segment a = [vowel, low, central]
        segment e = [vowel, mid-high, front]
        segment t = [stop, alveolar, -voiced]
        segment k = [stop, velar, -voiced]

        class C = { t, k }

        stage proto {}

        stage prog from proto {
          rule harmony (left-to-right):
            a > e / e C* _
        }

        stage reg from proto {
          rule harmony (right-to-left):
            a > e / _ C* e
        }

        stage sim from proto {
          rule harmony (simultaneous):
            a > e / e C* _
        }
    "#;

    let program = parse_program(dsl)?;
    program.validate_against(&program.inventory)?;
    let tree = program
        .dialect_tree
        .as_ref()
        .ok_or("expected dialect tree")?;

    let w_prog = tokenize_word("etaka", &program.inventory)?;
    let deriv = tree.derive_tree(&w_prog, &program.inventory)?;
    assert_eq!(
        deriv.word_at("prog").map(|w| w.to_spaced()).as_deref(),
        Some("e t e k e")
    );
    assert_eq!(
        deriv.word_at("sim").map(|w| w.to_spaced()).as_deref(),
        Some("e t e k a")
    );
    assert_eq!(
        deriv.word_at("reg").map(|w| w.to_spaced()).as_deref(),
        Some("e t a k a")
    );

    let w_reg = tokenize_word("atake", &program.inventory)?;
    let reg_trace = tree.derive_stage(&w_reg, "reg", &program.inventory)?;
    assert_eq!(reg_trace.final_word().to_spaced(), "e t e k e");

    let mixed = parse_program("rule r1:\n  a > e\nstage s {}");
    assert!(matches!(
        mixed,
        Err(DslError {
            kind: DslErrorKind::MixedTopLevelRulesAndStages,
            ..
        })
    ));
    Ok(())
}
