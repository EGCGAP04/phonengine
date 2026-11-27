//! End-to-end integration tests for the `phonengine` CLI binary.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const GOLDEN_MVP_TSV: &str = include_str!("fixtures/golden_mvp.tsv");

const MVP_DSL: &str = r#"
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

rule R3 "Intervocalic stop voicing":
  [-voiced, +stop] > [+voiced] / V _ V

rule R4 "Initial f- aspiration":
  f > h / # _ V
"#;

const HELLENIC_DSL: &str = r#"
segment pʰ = [stop, bilabial, -voiced, +aspirated]
segment p = [stop, bilabial, -voiced]
segment tʰ = [stop, alveolar, -voiced, +aspirated]
segment t = [stop, alveolar, -voiced]
segment s = [fricative, alveolar, -voiced]
segment h = [fricative, glottal, -voiced]
segment r = [rhotic, alveolar, +voiced]
segment aː = [vowel, low, central, +long]
segment eː = [vowel, mid-high, front, +long]
segment i = [vowel, high, front]

class V = { aː, eː, i }

stage pie "Proto-Indo-European" {}

stage proto_greek "Proto-Greek" from pie {
  rule grassmann (left-to-right):
    [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]

  rule s_debuccalization:
    s > h / # _ V
}

stage attic "Attic Greek" from proto_greek {
  rule a_raising:
    aː > eː

  rule alpha_purum "Attic alpha purum":
    eː > aː / r _
}

stage ionic "Ionic Greek" from proto_greek {
  rule a_raising:
    aː > eː
}
"#;

static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "phonengine_cli_test_{}_{}_{}",
            std::process::id(),
            id,
            label
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_file(&self, name: &str, contents: &str) -> PathBuf {
        let file_path = self.path.join(name);
        fs::write(&file_path, contents).expect("write temp file");
        file_path
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_cli(args: &[&str], stdin_data: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phonengine"));
    cmd.args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }

    let mut child = cmd.spawn().expect("spawn phonengine binary");
    if let Some(input) = stdin_data
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin.write_all(input.as_bytes()).expect("write stdin");
    }
    child.wait_with_output().expect("wait for phonengine")
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout utf8")
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf8")
}

#[test]
fn run_command_matches_golden_mvp_in_compact_and_spaced_modes() {
    let dir = TempDir::new("golden_run");
    let rules_path = dir.write_file("mvp.phon", MVP_DSL);
    let rules = rules_path.to_str().expect("utf8 path");

    let mut spaced_lines = String::from("# golden inputs\n\n");
    let mut compact_args = Vec::new();
    let mut expected_spaced = Vec::new();
    let mut expected_compact = Vec::new();

    for raw_line in GOLDEN_MVP_TSV.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = raw_line.split('\t').collect();
        assert_eq!(cols.len(), 4);
        spaced_lines.push_str(cols[0]);
        spaced_lines.push('\n');
        compact_args.push(cols[0].replace(' ', ""));
        expected_spaced.push(format!("{} -> {}", cols[0], cols[1]));
        expected_compact.push(format!(
            "{} -> {}",
            cols[0].replace(' ', ""),
            cols[1].replace(' ', "")
        ));
    }

    let words_path = dir.write_file("words_spaced.txt", &spaced_lines);
    let spaced_out = run_cli(
        &[
            "run",
            rules,
            "--spaced",
            "--words",
            words_path.to_str().expect("utf8 path"),
        ],
        None,
    );
    assert!(
        spaced_out.status.success(),
        "stderr: {}",
        stderr_str(&spaced_out)
    );
    assert_eq!(
        stdout_str(&spaced_out),
        format!("{}\n", expected_spaced.join("\n"))
    );

    let mut args = vec!["run", rules];
    for word in &compact_args {
        args.push(word);
    }
    let compact_out = run_cli(&args, None);
    assert!(
        compact_out.status.success(),
        "stderr: {}",
        stderr_str(&compact_out)
    );
    assert_eq!(
        stdout_str(&compact_out),
        format!("{}\n", expected_compact.join("\n"))
    );
}

#[test]
fn run_supports_stdin_rules_stdin_words_markdown_json_and_output_file() {
    let dir = TempDir::new("io_formats");
    let rules_path = dir.write_file("mvp.phon", MVP_DSL);
    let rules = rules_path.to_str().expect("utf8 path");
    let out_file = dir.path().join("result.json");

    // Rules from stdin (`-`) with positional words and Markdown format.
    let md_out = run_cli(
        &["run", "-", "-f", "markdown", "vitam", "fatam"],
        Some(MVP_DSL),
    );
    assert!(md_out.status.success(), "stderr: {}", stderr_str(&md_out));
    assert_eq!(
        stdout_str(&md_out),
        "| Input | Output |\n| --- | --- |\n| vitam | vida |\n| fatam | hada |\n"
    );

    // Words from stdin (`//` and `#` comments ignored) written to `-o <PATH>` as JSON.
    let json_cmd = run_cli(
        &[
            "run",
            rules,
            "-f",
            "json",
            "-o",
            out_file.to_str().expect("utf8 path"),
        ],
        Some("// Latin accusatives\n\n# ignored comment\nvitam\n"),
    );
    assert!(
        json_cmd.status.success(),
        "stderr: {}",
        stderr_str(&json_cmd)
    );
    assert_eq!(stdout_str(&json_cmd), "");
    let written = fs::read_to_string(&out_file).expect("read -o output");
    assert_eq!(
        written,
        concat!(
            "{\n",
            "  \"command\": \"run\",\n",
            "  \"stage\": null,\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"vitam\",\n",
            "      \"output\": \"vida\",\n",
            "      \"rules_fired\": [\"R1\", \"R3\"]\n",
            "    }\n",
            "  ]\n",
            "}\n",
        )
    );
}

#[test]
fn trace_formats_flat_and_staged_derivations() {
    let dir = TempDir::new("trace");
    let flat_path = dir.write_file("mvp.phon", MVP_DSL);
    let tree_path = dir.write_file("greek.phon", HELLENIC_DSL);

    let flat_text = run_cli(
        &[
            "trace",
            flat_path.to_str().expect("utf8 path"),
            "vitam",
            "lala",
        ],
        None,
    );
    assert!(
        flat_text.status.success(),
        "stderr: {}",
        stderr_str(&flat_text)
    );
    assert_eq!(
        stdout_str(&flat_text),
        concat!(
            "vitam -> vida\n",
            "  [R1] Final -m deletion: vitam -> vita\n",
            "  [R3] Intervocalic stop voicing: vita -> vida\n",
            "\n",
            "lala -> lala\n",
            "  (no changes)\n",
        )
    );

    let staged_json = run_cli(
        &[
            "trace",
            tree_path.to_str().expect("utf8 path"),
            "--stage",
            "attic",
            "-f",
            "json",
            "pʰˈaːtʰi",
        ],
        None,
    );
    assert!(
        staged_json.status.success(),
        "stderr: {}",
        stderr_str(&staged_json)
    );
    assert_eq!(
        stdout_str(&staged_json),
        concat!(
            "{\n",
            "  \"command\": \"trace\",\n",
            "  \"stage\": \"attic\",\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"pʰˈaːtʰi\",\n",
            "      \"output\": \"pˈeːtʰi\",\n",
            "      \"stages\": [\n",
            "        {\n",
            "          \"stage_id\": \"pie\",\n",
            "          \"stage_name\": \"Proto-Indo-European\",\n",
            "          \"input\": \"pʰˈaːtʰi\",\n",
            "          \"output\": \"pʰˈaːtʰi\",\n",
            "          \"steps\": []\n",
            "        },\n",
            "        {\n",
            "          \"stage_id\": \"proto_greek\",\n",
            "          \"stage_name\": \"Proto-Greek\",\n",
            "          \"input\": \"pʰˈaːtʰi\",\n",
            "          \"output\": \"pˈaːtʰi\",\n",
            "          \"steps\": [\n",
            "            {\n",
            "              \"rule_id\": \"grassmann\",\n",
            "              \"rule_name\": \"grassmann\",\n",
            "              \"before\": \"pʰˈaːtʰi\",\n",
            "              \"after\": \"pˈaːtʰi\"\n",
            "            }\n",
            "          ]\n",
            "        },\n",
            "        {\n",
            "          \"stage_id\": \"attic\",\n",
            "          \"stage_name\": \"Attic Greek\",\n",
            "          \"input\": \"pˈaːtʰi\",\n",
            "          \"output\": \"pˈeːtʰi\",\n",
            "          \"steps\": [\n",
            "            {\n",
            "              \"rule_id\": \"a_raising\",\n",
            "              \"rule_name\": \"a_raising\",\n",
            "              \"before\": \"pˈaːtʰi\",\n",
            "              \"after\": \"pˈeːtʰi\"\n",
            "            }\n",
            "          ]\n",
            "        }\n",
            "      ]\n",
            "    }\n",
            "  ]\n",
            "}\n",
        )
    );
}

#[test]
fn compare_formats_default_leaves_and_custom_stages() {
    let dir = TempDir::new("compare");
    let tree_path = dir.write_file("greek.phon", HELLENIC_DSL);
    let tree = tree_path.to_str().expect("utf8 path");

    let leaves_text = run_cli(&["compare", tree, "srˈaːtʰi", "pʰˈaːtʰi"], None);
    assert!(
        leaves_text.status.success(),
        "stderr: {}",
        stderr_str(&leaves_text)
    );
    assert_eq!(
        stdout_str(&leaves_text),
        concat!(
            "Input     attic     ionic\n",
            "--------  --------  --------\n",
            "srˈaːtʰi  srˈaːtʰi  srˈeːtʰi\n",
            "pʰˈaːtʰi  pˈeːtʰi   pˈeːtʰi\n",
        )
    );

    let custom_md = run_cli(
        &[
            "compare",
            tree,
            "--stages",
            "ionic,proto_greek",
            "-f",
            "markdown",
            "srˈaːtʰi",
        ],
        None,
    );
    assert!(
        custom_md.status.success(),
        "stderr: {}",
        stderr_str(&custom_md)
    );
    assert_eq!(
        stdout_str(&custom_md),
        concat!(
            "| Input | ionic | proto_greek |\n",
            "| --- | --- | --- |\n",
            "| srˈaːtʰi | srˈeːtʰi | srˈaːtʰi |\n",
        )
    );

    let custom_json = run_cli(
        &[
            "compare",
            tree,
            "--stages",
            "ionic,attic",
            "-f",
            "json",
            "srˈaːtʰi",
        ],
        None,
    );
    assert!(
        custom_json.status.success(),
        "stderr: {}",
        stderr_str(&custom_json)
    );
    assert_eq!(
        stdout_str(&custom_json),
        concat!(
            "{\n",
            "  \"command\": \"compare\",\n",
            "  \"stages\": [\"ionic\", \"attic\"],\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"srˈaːtʰi\",\n",
            "      \"outcomes\": {\n",
            "        \"ionic\": \"srˈeːtʰi\",\n",
            "        \"attic\": \"srˈaːtʰi\"\n",
            "      }\n",
            "    }\n",
            "  ]\n",
            "}\n",
        )
    );
}

#[test]
fn cli_reports_expected_exit_codes_for_clap_and_runtime_errors() {
    let dir = TempDir::new("errors");
    let flat_path = dir.write_file("mvp.phon", MVP_DSL);
    let tree_path = dir.write_file("greek.phon", HELLENIC_DSL);
    let empty_inv_path = dir.write_file("empty_inv.phon", "rule r1:\n  a > o\n");
    let bad_dsl_path = dir.write_file("bad.phon", "segment a = []\n");
    let words_path = dir.write_file("words.txt", "vitam\n");

    let flat = flat_path.to_str().expect("utf8 path");
    let tree = tree_path.to_str().expect("utf8 path");
    let empty_inv = empty_inv_path.to_str().expect("utf8 path");
    let bad_dsl = bad_dsl_path.to_str().expect("utf8 path");
    let words = words_path.to_str().expect("utf8 path");

    // Clap argument errors -> exit code 2
    let conflict = run_cli(&["run", flat, "--words", words, "vitam"], None);
    assert_eq!(conflict.status.code(), Some(2));

    let unknown_flag = run_cli(&["run", flat, "--no-such-flag"], None);
    assert_eq!(unknown_flag.status.code(), Some(2));

    // Runtime errors -> exit code 1 + "error: ..." on stderr
    let missing_rules = dir.path().join("does_not_exist.phon");
    let missing_words = dir.path().join("missing_words.txt");
    let unwritable_out = dir.path().join("no_such_dir").join("out.txt");
    let unmapped_dsl_path = dir.write_file(
        "unmapped.phon",
        "segment f = [fricative, labiodental, -voiced]\nrule r1:\n  f > [+voiced]\n",
    );
    let missing_rules_s = missing_rules.to_str().expect("utf8 path");
    let missing_words_s = missing_words.to_str().expect("utf8 path");
    let unwritable_out_s = unwritable_out.to_str().expect("utf8 path");
    let unmapped_dsl_s = unmapped_dsl_path.to_str().expect("utf8 path");

    let cases: &[(&[&str], Option<&str>, &str)] = &[
        (&["run", empty_inv, "a"], None, "segment"),
        (&["run", bad_dsl, "a"], None, "DSL error"),
        (&["run", tree, "pʰˈaːtʰi"], None, "--stage"),
        (&["run", flat, "--stage", "attic", "vitam"], None, "--stage"),
        (
            &["run", tree, "--stage", "doric", "pʰˈaːtʰi"],
            None,
            "unknown stage",
        ),
        (&["compare", flat, "vitam"], None, "stage"),
        (
            &["compare", tree, "--stages", "attic,attic", "pʰˈaːtʰi"],
            None,
            "more than once",
        ),
        (
            &["compare", tree, "--stages", "doric", "pʰˈaːtʰi"],
            None,
            "unknown stage",
        ),
        (
            &["compare", tree, "--stages", "", "pʰˈaːtʰi"],
            None,
            "unknown stage",
        ),
        (&["run", "-"], Some(MVP_DSL), "stdin"),
        (&["run", flat, "xyz"], None, "tokenize"),
        (&["run", flat, "--spaced", "v x"], None, "spaced"),
        (&["run", unmapped_dsl_s, "f"], None, "derivation failed"),
        (
            &["run", missing_rules_s, "vitam"],
            None,
            "does_not_exist.phon",
        ),
        (
            &["run", flat, "--words", missing_words_s],
            None,
            "missing_words.txt",
        ),
        (
            &["run", flat, "-o", unwritable_out_s, "vitam"],
            None,
            "no_such_dir",
        ),
    ];

    for (args, stdin_data, expected_substr) in cases {
        let out = run_cli(args, *stdin_data);
        assert_eq!(
            out.status.code(),
            Some(1),
            "expected exit code 1 for {args:?}, stderr: {}",
            stderr_str(&out)
        );
        let err = stderr_str(&out);
        assert!(
            err.starts_with("error: ") && err.contains(expected_substr),
            "unexpected stderr for {args:?}: {err}"
        );
    }
}

#[test]
fn run_and_trace_support_staged_text_markdown_json_and_spaced_modes() {
    let dir = TempDir::new("staged_modes");
    let tree_path = dir.write_file("greek.phon", HELLENIC_DSL);
    let tree = tree_path.to_str().expect("utf8 path");

    // `run --stage attic` across text, markdown, and json.
    let run_text = run_cli(&["run", tree, "--stage", "attic", "pʰˈaːtʰi"], None);
    assert!(
        run_text.status.success(),
        "stderr: {}",
        stderr_str(&run_text)
    );
    assert_eq!(stdout_str(&run_text), "pʰˈaːtʰi -> pˈeːtʰi\n");

    let run_md = run_cli(
        &[
            "run",
            tree,
            "--stage",
            "attic",
            "-f",
            "markdown",
            "pʰˈaːtʰi",
        ],
        None,
    );
    assert!(run_md.status.success(), "stderr: {}", stderr_str(&run_md));
    assert_eq!(
        stdout_str(&run_md),
        "| Input | Output |\n| --- | --- |\n| pʰˈaːtʰi | pˈeːtʰi |\n"
    );

    let run_json = run_cli(
        &["run", tree, "--stage", "attic", "-f", "json", "pʰˈaːtʰi"],
        None,
    );
    assert!(
        run_json.status.success(),
        "stderr: {}",
        stderr_str(&run_json)
    );
    assert_eq!(
        stdout_str(&run_json),
        concat!(
            "{\n",
            "  \"command\": \"run\",\n",
            "  \"stage\": \"attic\",\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"pʰˈaːtʰi\",\n",
            "      \"output\": \"pˈeːtʰi\",\n",
            "      \"rules_fired\": [\"grassmann\", \"a_raising\"]\n",
            "    }\n",
            "  ]\n",
            "}\n",
        )
    );

    // `trace --stage attic` in text and markdown (including `--spaced`).
    let trace_text = run_cli(
        &["trace", tree, "--stage", "attic", "--spaced", "pʰ ˈaː tʰ i"],
        None,
    );
    assert!(
        trace_text.status.success(),
        "stderr: {}",
        stderr_str(&trace_text)
    );
    assert_eq!(
        stdout_str(&trace_text),
        concat!(
            "pʰ ˈaː tʰ i -> p ˈeː tʰ i (stage: attic)\n",
            "  stage pie (Proto-Indo-European): pʰ ˈaː tʰ i -> pʰ ˈaː tʰ i\n",
            "    (no changes)\n",
            "  stage proto_greek (Proto-Greek): pʰ ˈaː tʰ i -> p ˈaː tʰ i\n",
            "    [grassmann] grassmann: pʰ ˈaː tʰ i -> p ˈaː tʰ i\n",
            "  stage attic (Attic Greek): p ˈaː tʰ i -> p ˈeː tʰ i\n",
            "    [a_raising] a_raising: p ˈaː tʰ i -> p ˈeː tʰ i\n",
        )
    );

    let trace_md = run_cli(
        &[
            "trace",
            tree,
            "--stage",
            "attic",
            "-f",
            "markdown",
            "pʰˈaːtʰi",
        ],
        None,
    );
    assert!(
        trace_md.status.success(),
        "stderr: {}",
        stderr_str(&trace_md)
    );
    assert_eq!(
        stdout_str(&trace_md),
        concat!(
            "## pʰˈaːtʰi -> pˈeːtʰi (stage: attic)\n\n",
            "### pie — Proto-Indo-European (pʰˈaːtʰi -> pʰˈaːtʰi)\n\n",
            "_No changes._\n\n",
            "### proto_greek — Proto-Greek (pʰˈaːtʰi -> pˈaːtʰi)\n\n",
            "| Rule ID | Rule | Before | After |\n",
            "| --- | --- | --- | --- |\n",
            "| grassmann | grassmann | pʰˈaːtʰi | pˈaːtʰi |\n\n",
            "### attic — Attic Greek (pˈaːtʰi -> pˈeːtʰi)\n\n",
            "| Rule ID | Rule | Before | After |\n",
            "| --- | --- | --- | --- |\n",
            "| a_raising | a_raising | pˈaːtʰi | pˈeːtʰi |\n",
        )
    );

    // `compare --spaced` and repeated `--stages`.
    let compare_spaced = run_cli(
        &[
            "compare",
            tree,
            "--spaced",
            "--stages",
            "ionic",
            "--stages",
            "attic",
            "s r ˈaː tʰ i",
        ],
        None,
    );
    assert!(
        compare_spaced.status.success(),
        "stderr: {}",
        stderr_str(&compare_spaced)
    );
    assert_eq!(
        stdout_str(&compare_spaced),
        concat!(
            "Input         ionic         attic\n",
            "------------  ------------  ------------\n",
            "s r ˈaː tʰ i  s r ˈeː tʰ i  s r ˈaː tʰ i\n",
        )
    );
}

#[test]
fn cli_handles_bom_zero_words_empty_deletion_and_special_escaping() {
    let dir = TempDir::new("edge_cases");

    // 1. UTF-8 BOM in `--words` (before comment and before word) and `stdin`.
    let mvp_path = dir.write_file("mvp.phon", MVP_DSL);
    let mvp = mvp_path.to_str().expect("utf8 path");
    let bom_words = dir.write_file("bom_words.txt", "\u{FEFF}# comment\nvitam\n");
    let bom_words_err = dir.write_file("bom_first.txt", "\u{FEFF}vitam\n");

    let bom_comment_out = run_cli(
        &[
            "run",
            mvp,
            "--words",
            bom_words.to_str().expect("utf8 path"),
        ],
        None,
    );
    assert!(
        bom_comment_out.status.success(),
        "stderr: {}",
        stderr_str(&bom_comment_out)
    );
    assert_eq!(stdout_str(&bom_comment_out), "vitam -> vida\n");

    let bom_first_out = run_cli(
        &[
            "run",
            mvp,
            "--words",
            bom_words_err.to_str().expect("utf8 path"),
        ],
        None,
    );
    assert!(
        bom_first_out.status.success(),
        "stderr: {}",
        stderr_str(&bom_first_out)
    );
    assert_eq!(stdout_str(&bom_first_out), "vitam -> vida\n");

    let bom_stdin_out = run_cli(&["run", mvp], Some("\u{FEFF}// header\nvitam\n"));
    assert!(
        bom_stdin_out.status.success(),
        "stderr: {}",
        stderr_str(&bom_stdin_out)
    );
    assert_eq!(stdout_str(&bom_stdin_out), "vitam -> vida\n");

    // 2. Total deletion to empty word `""` across `run`, `trace`, and `compare`.
    let del_tree_dsl = r#"
segment m = [nasal, bilabial, +voiced]
stage root {}
stage keep from root {}
stage drop from root {
  rule del_m:
    m > ∅ / _ #
}
"#;
    let del_tree_path = dir.write_file("del_tree.phon", del_tree_dsl);
    let del_tree = del_tree_path.to_str().expect("utf8 path");

    let del_run = run_cli(&["run", mvp, "m"], None);
    assert_eq!(stdout_str(&del_run), "m -> \n");

    let del_trace = run_cli(&["trace", mvp, "m"], None);
    assert_eq!(
        stdout_str(&del_trace),
        "m -> \n  [R1] Final -m deletion: m -> \n"
    );

    let del_compare_text = run_cli(&["compare", del_tree, "m"], None);
    assert!(
        del_compare_text.status.success(),
        "stderr: {}",
        stderr_str(&del_compare_text)
    );
    let compare_str = stdout_str(&del_compare_text);
    assert_eq!(
        compare_str,
        "Input  drop  keep\n-----  ----  ----\nm            m\n"
    );
    for line in compare_str.lines() {
        assert!(!line.ends_with(' '), "trailing space in {line:?}");
    }

    let del_compare_last_empty =
        run_cli(&["compare", del_tree, "--stages", "keep,drop", "m"], None);
    assert_eq!(
        stdout_str(&del_compare_last_empty),
        "Input  keep  drop\n-----  ----  ----\nm      m\n"
    );

    // 3. Zero input words -> exit code 0 across all commands and formats.
    let tree_path = dir.write_file("greek.phon", HELLENIC_DSL);
    let tree = tree_path.to_str().expect("utf8 path");
    for (args, expected_stdout) in [
        (&["run", mvp][..], ""),
        (
            &["run", mvp, "-f", "markdown"][..],
            "| Input | Output |\n| --- | --- |\n",
        ),
        (
            &["run", mvp, "-f", "json"][..],
            "{\n  \"command\": \"run\",\n  \"stage\": null,\n  \"results\": []\n}\n",
        ),
        (&["trace", mvp][..], ""),
        (&["trace", mvp, "-f", "markdown"][..], ""),
        (
            &["trace", mvp, "-f", "json"][..],
            "{\n  \"command\": \"trace\",\n  \"stage\": null,\n  \"results\": []\n}\n",
        ),
        (&["compare", tree][..], ""),
        (
            &["compare", tree, "-f", "markdown"][..],
            "| Input | attic | ionic |\n| --- | --- | --- |\n",
        ),
        (
            &["compare", tree, "-f", "json"][..],
            "{\n  \"command\": \"compare\",\n  \"stages\": [\"attic\", \"ionic\"],\n  \"results\": []\n}\n",
        ),
    ] {
        let out = run_cli(args, Some(""));
        assert!(out.status.success(), "args {args:?}: {}", stderr_str(&out));
        assert_eq!(stdout_str(&out), expected_stdout, "args {args:?}");
    }

    // 4. Rule name containing quotes, backslashes, and pipes in Markdown and JSON.
    let special_dsl = r#"
segment a = [vowel, low, central]
segment o = [vowel, mid-high, back, +rounded]
rule R_ESC "a|b \"q\" \\s":
  a > o
"#;
    let special_path = dir.write_file("special.phon", special_dsl);
    let special = special_path.to_str().expect("utf8 path");

    let special_md = run_cli(&["trace", special, "-f", "markdown", "a"], None);
    assert_eq!(
        stdout_str(&special_md),
        concat!(
            "## a -> o\n\n",
            "| Rule ID | Rule | Before | After |\n",
            "| --- | --- | --- | --- |\n",
            "| R_ESC | a\\|b \"q\" \\s | a | o |\n",
        )
    );

    let special_json = run_cli(&["trace", special, "-f", "json", "a"], None);
    assert_eq!(
        stdout_str(&special_json),
        concat!(
            "{\n",
            "  \"command\": \"trace\",\n",
            "  \"stage\": null,\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"a\",\n",
            "      \"output\": \"o\",\n",
            "      \"steps\": [\n",
            "        {\n",
            "          \"rule_id\": \"R_ESC\",\n",
            "          \"rule_name\": \"a|b \\\"q\\\" \\\\s\",\n",
            "          \"before\": \"a\",\n",
            "          \"after\": \"o\"\n",
            "        }\n",
            "      ]\n",
            "    }\n",
            "  ]\n",
            "}\n",
        )
    );
}
