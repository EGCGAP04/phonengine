//! Integration tests for Milestone 5 real-world historical grammar examples.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use phonengine::{parse_program, tokenize_word};

const LATIN_TO_ROMANCE_DSL: &str = include_str!("../examples/latin_to_romance.phon");
const PIE_TO_GREEK_DSL: &str = include_str!("../examples/pie_to_greek.phon");

#[test]
fn latin_to_romance_grammar_validates_and_derives_golden_etyma()
-> Result<(), Box<dyn std::error::Error>> {
    let program = parse_program(LATIN_TO_ROMANCE_DSL)?;
    program.validate_against(&program.inventory)?;
    assert_eq!(program.inventory.len(), 32);
    assert!(program.rules.is_empty());

    let tree = program
        .dialect_tree
        .as_ref()
        .ok_or("expected dialect tree in latin_to_romance.phon")?;

    let cases: &[(&str, &str, &str, &str, &str, &str)] = &[
        ("vˈiːtam", "vˈita", "vˈita", "vˈida", "vˈida", "vˈiə"),
        ("fˈaːtam", "fˈata", "fˈata", "fˈada", "hˈada", "fˈeə"),
        (
            "pˈetram",
            "pˈɛtra",
            "pjˈɛtra",
            "pˈɛdra",
            "pjˈedra",
            "pjˈɛrə",
        ),
        ("pˈortam", "pˈɔrta", "pˈɔrta", "pˈɔrta", "pwˈerta", "pˈɔrtə"),
        ("pˈontem", "pˈɔnte", "pˈɔnte", "pˈɔnte", "pwˈente", "pˈɔnt"),
        ("bˈonum", "bˈɔno", "bwˈɔno", "bˈɔno", "bwˈeno", "bˈɔn"),
        ("fˈokum", "fˈɔko", "fwˈɔko", "fˈɔgo", "fwˈego", "fˈɔk"),
        ("hˈerbam", "ˈɛrba", "ˈɛrba", "ˈɛrba", "jˈerba", "ˈɛrbə"),
        (
            "skˈolam",
            "skˈɔla",
            "skwˈɔla",
            "eskˈɔla",
            "eskwˈela",
            "ekˈɔlə",
        ),
        ("stˈatum", "stˈato", "stˈato", "estˈado", "estˈado", "etˈe"),
        ("sˈeptem", "sˈɛpte", "sˈɛtte", "sˈɛpte", "sjˈete", "sˈɛt"),
        ("mˈuːrum", "mˈuro", "mˈuro", "mˈuro", "mˈuro", "mˈyr"),
        ("lˈuːnam", "lˈuna", "lˈuna", "lˈuna", "lˈuna", "lˈynə"),
        ("kˈalidum", "kˈaldo", "kˈaldo", "kˈaldo", "kˈaldo", "kˈalt"),
        ("vˈiridem", "vˈerde", "vˈerde", "vˈerde", "vˈerde", "vˈert"),
        ("ˈalterum", "ˈaltro", "ˈaltro", "ˈaltro", "ˈaltro", "ˈaltrə"),
        (
            "trˈunkum",
            "trˈoŋko",
            "trˈoŋko",
            "trˈoŋko",
            "trˈoŋko",
            "trˈoŋk",
        ),
        (
            "inpˈuːrum",
            "empˈuro",
            "empˈuro",
            "empˈuro",
            "empˈuro",
            "empˈyr",
        ),
    ];

    for &(input, vl, it, wr, es, fr) in cases {
        let word = tokenize_word(input, &program.inventory)?;
        let deriv = tree.derive_tree(&word, &program.inventory)?;
        assert_eq!(deriv.root_id(), "classical_latin");

        let compact_stage = |id: &str| -> Result<String, Box<dyn std::error::Error>> {
            let derived = deriv
                .word_at(id)
                .ok_or_else(|| format!("missing stage {id} for {input}"))?;
            let compact = derived.to_compact();
            assert_eq!(
                tokenize_word(&compact, &program.inventory)?,
                *derived,
                "round-trip compact tokenization failed at stage {id} for {input} ({compact})"
            );
            Ok(compact)
        };

        assert_eq!(
            compact_stage("vulgar_latin")?,
            vl,
            "VL mismatch for {input}"
        );
        assert_eq!(compact_stage("italian")?, it, "IT mismatch for {input}");
        assert_eq!(
            compact_stage("western_romance")?,
            wr,
            "WR mismatch for {input}"
        );
        assert_eq!(compact_stage("spanish")?, es, "ES mismatch for {input}");
        assert_eq!(compact_stage("french")?, fr, "FR mismatch for {input}");

        let leaf_names: Vec<&str> = deriv.leaf_outcomes().keys().copied().collect();
        assert_eq!(leaf_names, vec!["french", "italian", "spanish"]);
    }

    // Check fired rule traces on key interactions.
    let petram = tokenize_word("pˈetram", &program.inventory)?;
    let fr_trace = tree.derive_stage(&petram, "french", &program.inventory)?;
    let fr_rules: Vec<&str> = fr_trace
        .stages()
        .iter()
        .flat_map(|s| s.trace().fired_rule_ids())
        .collect();
    assert!(fr_rules.contains(&"open_e_diph"));
    assert!(fr_rules.contains(&"intervocalic_dental_loss"));
    assert!(fr_rules.contains(&"final_a_reduction"));

    // Verify `initial_f_aspiration` in Spanish is bled by `spanish_o_diph` on `fˈokum` -> `fwˈego`.
    let fokum = tokenize_word("fˈokum", &program.inventory)?;
    let es_fokum = tree.derive_stage(&fokum, "spanish", &program.inventory)?;
    let es_fokum_rules: Vec<&str> = es_fokum
        .stages()
        .iter()
        .flat_map(|s| s.trace().fired_rule_ids())
        .collect();
    assert!(!es_fokum_rules.contains(&"initial_f_aspiration"));

    Ok(())
}

#[test]
fn pie_to_greek_grammar_validates_and_derives_golden_etyma()
-> Result<(), Box<dyn std::error::Error>> {
    let program = parse_program(PIE_TO_GREEK_DSL)?;
    program.validate_against(&program.inventory)?;
    assert_eq!(program.inventory.len(), 33);
    assert!(program.rules.is_empty());

    let tree = program
        .dialect_tree
        .as_ref()
        .ok_or("expected dialect tree in pie_to_greek.phon")?;

    let cases: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "dʰegʰˈaːs",
            "tekʰˈaːs",
            "tekʰˈeːs",
            "tekʰˈeːs",
            "tekʰˈeːs",
            "tekʰˈaːs",
        ),
        (
            "dʰrˈigʰos",
            "trˈikʰos",
            "trˈikʰos",
            "trˈikʰos",
            "trˈikʰos",
            "trˈikʰos",
        ),
        (
            "dʰrˈigʰs",
            "tʰrˈiks",
            "tʰrˈiks",
            "tʰrˈiks",
            "tʰrˈiks",
            "tʰrˈiks",
        ),
        (
            "lˈegtos", "lˈektos", "lˈektos", "lˈektos", "lˈektos", "lˈektos",
        ),
        (
            "lˈeptʰeːn",
            "lˈepʰtʰeːn",
            "lˈepʰtʰeːn",
            "lˈepʰtʰeːn",
            "lˈepʰtʰeːn",
            "lˈepʰtʰeːn",
        ),
        (
            "gʷʰˈermos",
            "tʰˈermos",
            "tʰˈermos",
            "tʰˈermos",
            "tʰˈermos",
            "tʰˈermos",
        ),
        (
            "gʷʰˈonos",
            "pʰˈonos",
            "pʰˈonos",
            "pʰˈonos",
            "pʰˈonos",
            "pʰˈonos",
        ),
        ("kʷˈis", "tˈis", "tˈis", "tˈis", "tˈis", "tˈis"),
        ("lˈukʷos", "lˈukos", "lˈukos", "lˈukos", "lˈukos", "lˈukos"),
        (
            "kʷˈuklos",
            "kˈuklos",
            "kˈuklos",
            "kˈuklos",
            "kˈuklos",
            "kˈuklos",
        ),
        (
            "gˈenesos",
            "gˈeneos",
            "gˈeneos",
            "gˈenoːs",
            "gˈeneos",
            "gˈeneos",
        ),
        (
            "gʰˈoːraː",
            "kʰˈoːraː",
            "kʰˈoːreː",
            "kʰˈoːraː",
            "kʰˈoːreː",
            "kʰˈoːraː",
        ),
        (
            "kˈardiaː",
            "kˈardiaː",
            "kˈardieː",
            "kˈardiaː",
            "kˈardieː",
            "kˈardiaː",
        ),
        ("sˈaːma", "hˈaːma", "hˈeːma", "hˈeːma", "ˈeːma", "hˈaːma"),
        (
            "bʰˈeronti",
            "pʰˈeronti",
            "pʰˈeroːsi",
            "pʰˈeroːsi",
            "pʰˈeroːsi",
            "pʰˈeronti",
        ),
        (
            "dˈidoːti",
            "dˈidoːti",
            "dˈidoːsi",
            "dˈidoːsi",
            "dˈidoːsi",
            "dˈidoːti",
        ),
        (
            "wˈergon", "wˈergon", "ˈergon", "ˈergon", "ˈergon", "wˈergon",
        ),
        (
            "mˈaːteːr",
            "mˈaːteːr",
            "mˈeːteːr",
            "mˈeːteːr",
            "mˈeːteːr",
            "mˈaːteːr",
        ),
    ];

    for &(input, pgk, ai, att, ion, dor) in cases {
        let word = tokenize_word(input, &program.inventory)?;
        let deriv = tree.derive_tree(&word, &program.inventory)?;
        assert_eq!(deriv.root_id(), "pie");

        let compact_stage = |id: &str| -> Result<String, Box<dyn std::error::Error>> {
            let derived = deriv
                .word_at(id)
                .ok_or_else(|| format!("missing stage {id} for {input}"))?;
            let compact = derived.to_compact();
            assert_eq!(
                tokenize_word(&compact, &program.inventory)?,
                *derived,
                "round-trip compact tokenization failed at stage {id} for {input} ({compact})"
            );
            Ok(compact)
        };

        assert_eq!(
            compact_stage("proto_greek")?,
            pgk,
            "Proto-Greek mismatch for {input}"
        );
        assert_eq!(
            compact_stage("attic_ionic")?,
            ai,
            "Proto-Attic-Ionic mismatch for {input}"
        );
        assert_eq!(compact_stage("attic")?, att, "Attic mismatch for {input}");
        assert_eq!(compact_stage("ionic")?, ion, "Ionic mismatch for {input}");
        assert_eq!(compact_stage("doric")?, dor, "Doric mismatch for {input}");

        let leaves: BTreeMap<&str, String> = deriv
            .leaf_outcomes()
            .into_iter()
            .map(|(k, w)| (k, w.to_compact()))
            .collect();
        let leaf_names: Vec<&str> = leaves.keys().copied().collect();
        assert_eq!(leaf_names, vec!["attic", "doric", "ionic"]);
    }

    // Verify stop_before_s bleeds Grassmann's Law on dʰrˈigʰs -> tʰrˈiks.
    let thrigs = tokenize_word("dʰrˈigʰs", &program.inventory)?;
    let thrigs_trace = tree.derive_stage(&thrigs, "proto_greek", &program.inventory)?;
    let thrigs_rules: Vec<&str> = thrigs_trace
        .stages()
        .iter()
        .flat_map(|s| s.trace().fired_rule_ids())
        .collect();
    assert!(thrigs_rules.contains(&"stop_before_s"));
    assert!(!thrigs_rules.contains(&"grassmann"));

    // Whereas dʰrˈigʰos -> trˈikʰos fires Grassmann's Law.
    let thrikhos = tokenize_word("dʰrˈigʰos", &program.inventory)?;
    let thrikhos_trace = tree.derive_stage(&thrikhos, "proto_greek", &program.inventory)?;
    let thrikhos_rules: Vec<&str> = thrikhos_trace
        .stages()
        .iter()
        .flat_map(|s| s.trace().fired_rule_ids())
        .collect();
    assert!(thrikhos_rules.contains(&"grassmann"));

    Ok(())
}

#[test]
fn cli_runs_bundled_example_grammars() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let romance_path = manifest_dir.join("examples").join("latin_to_romance.phon");
    let greek_path = manifest_dir.join("examples").join("pie_to_greek.phon");

    let romance_out = Command::new(env!("CARGO_BIN_EXE_phonengine"))
        .arg("compare")
        .arg(&romance_path)
        .args(["pˈetram", "skˈolam", "sˈeptem"])
        .output()?;
    assert!(
        romance_out.status.success(),
        "CLI compare failed on latin_to_romance.phon: {}",
        String::from_utf8_lossy(&romance_out.stderr)
    );
    let romance_stdout = String::from_utf8(romance_out.stdout)?;
    assert!(romance_stdout.contains("french"));
    assert!(romance_stdout.contains("italian"));
    assert!(romance_stdout.contains("spanish"));
    assert!(romance_stdout.contains("pjˈɛrə"));
    assert!(romance_stdout.contains("eskwˈela"));
    assert!(romance_stdout.contains("sˈɛtte"));

    let greek_out = Command::new(env!("CARGO_BIN_EXE_phonengine"))
        .arg("compare")
        .arg(&greek_path)
        .args(["--stages", "attic,ionic,doric", "-f", "json"])
        .args(["sˈaːma", "gˈenesos", "bʰˈeronti"])
        .output()?;
    assert!(
        greek_out.status.success(),
        "CLI compare -f json failed on pie_to_greek.phon: {}",
        String::from_utf8_lossy(&greek_out.stderr)
    );
    let greek_stdout = String::from_utf8(greek_out.stdout)?;
    assert!(greek_stdout.contains("\"attic\": \"hˈeːma\""));
    assert!(greek_stdout.contains("\"ionic\": \"ˈeːma\""));
    assert!(greek_stdout.contains("\"doric\": \"hˈaːma\""));
    assert!(greek_stdout.contains("\"attic\": \"gˈenoːs\""));
    assert!(greek_stdout.contains("\"attic\": \"pʰˈeroːsi\""));
    assert!(greek_stdout.contains("\"doric\": \"pʰˈeronti\""));

    Ok(())
}
