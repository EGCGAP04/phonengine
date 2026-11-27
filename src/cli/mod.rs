//! Command-line driver for PhonEngine (`run`, `trace`, `compare`).

pub mod args;
pub mod error;
pub mod format;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use phonengine::{DialectTree, Program, Word, derive, parse_program, tokenize_word};

use args::{Cli, Command, CommonArgs};
use error::CliError;
use format::{
    CompareRow, FlatTraceRow, RunRow, StagedTraceRow, TraceOutput, build_stage_rows,
    build_step_rows, format_compare, format_run, format_trace, format_word,
};

enum ExecutionTarget<'a> {
    Flat,
    Stage(&'a DialectTree, &'a str),
}

/// Parses arguments, executes the selected command, and maps failures to exit codes.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();

    if let Err(err) = execute(cli, &mut stdin_lock, &mut stdout_lock) {
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "error: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Executes a parsed [`Cli`] command using explicit `stdin` and `stdout` streams.
///
/// # Errors
///
/// Returns [`CliError`] on I/O, DSL parsing, empty inventory, word parsing, stage validation,
/// or engine evaluation failures.
pub fn execute<R: BufRead, W: Write>(
    cli: Cli,
    stdin: &mut R,
    stdout: &mut W,
) -> Result<(), CliError> {
    let common = match &cli.command {
        Command::Run { common, .. }
        | Command::Trace { common, .. }
        | Command::Compare { common, .. } => common,
    };

    if is_stdin_path(&common.rules) && common.words.is_empty() && common.words_file.is_none() {
        return Err(CliError::StdinConflict);
    }

    let rules_src = read_rules(&common.rules, stdin)?;
    let program = parse_program(&rules_src)?;
    if program.inventory.is_empty() {
        return Err(CliError::EmptyInventory);
    }

    let rendered = match &cli.command {
        Command::Run { stage, common } => run_command(&program, stage.as_deref(), common, stdin)?,
        Command::Trace { stage, common } => {
            trace_command(&program, stage.as_deref(), common, stdin)?
        }
        Command::Compare { stages, common } => {
            compare_command(&program, stages.as_deref(), common, stdin)?
        }
    };

    write_output(common.output.as_deref(), &rendered, stdout)
}

fn is_stdin_path(path: &Path) -> bool {
    path.as_os_str() == OsStr::new("-")
}

fn resolve_target<'a>(
    program: &'a Program,
    stage: Option<&'a str>,
) -> Result<ExecutionTarget<'a>, CliError> {
    match (&program.dialect_tree, stage) {
        (Some(tree), Some(stage_id)) => {
            if tree.node(stage_id).is_none() {
                return Err(CliError::UnknownStage(stage_id.to_owned()));
            }
            Ok(ExecutionTarget::Stage(tree, stage_id))
        }
        (Some(_), None) => Err(CliError::MissingStage),
        (None, Some(stage_id)) => Err(CliError::UnexpectedStage(stage_id.to_owned())),
        (None, None) => Ok(ExecutionTarget::Flat),
    }
}

fn run_command<R: BufRead>(
    program: &Program,
    stage: Option<&str>,
    common: &CommonArgs,
    stdin: &mut R,
) -> Result<String, CliError> {
    let target = resolve_target(program, stage)?;
    let raw_words = load_words(common, stdin)?;
    let words = parse_words(&raw_words, program, common.spaced)?;
    let mut rows = Vec::with_capacity(words.len());

    match target {
        ExecutionTarget::Stage(tree, stage_id) => {
            for (raw, word) in raw_words.iter().zip(words.iter()) {
                let trace = tree
                    .derive_stage(word, stage_id, &program.inventory)
                    .map_err(|source| CliError::Engine {
                        word: raw.clone(),
                        source,
                    })?;
                let mut rules_fired = Vec::new();
                for stage_trace in trace.stages() {
                    for rule_id in stage_trace.trace().fired_rule_ids() {
                        rules_fired.push(rule_id.to_owned());
                    }
                }
                rows.push(RunRow {
                    input: format_word(trace.initial(), common.spaced),
                    output: format_word(trace.final_word(), common.spaced),
                    rules_fired,
                });
            }
        }
        ExecutionTarget::Flat => {
            for (raw, word) in raw_words.iter().zip(words.iter()) {
                let trace = derive(word, &program.rules, &program.inventory).map_err(|source| {
                    CliError::Engine {
                        word: raw.clone(),
                        source,
                    }
                })?;
                rows.push(RunRow {
                    input: format_word(trace.initial(), common.spaced),
                    output: format_word(trace.final_word(), common.spaced),
                    rules_fired: trace
                        .fired_rule_ids()
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                });
            }
        }
    }

    Ok(format_run(stage, &rows, common.format))
}

fn trace_command<R: BufRead>(
    program: &Program,
    stage: Option<&str>,
    common: &CommonArgs,
    stdin: &mut R,
) -> Result<String, CliError> {
    let target = resolve_target(program, stage)?;
    let raw_words = load_words(common, stdin)?;
    let words = parse_words(&raw_words, program, common.spaced)?;

    let output = match target {
        ExecutionTarget::Stage(tree, stage_id) => {
            let mut rows = Vec::with_capacity(words.len());
            for (raw, word) in raw_words.iter().zip(words.iter()) {
                let trace = tree
                    .derive_stage(word, stage_id, &program.inventory)
                    .map_err(|source| CliError::Engine {
                        word: raw.clone(),
                        source,
                    })?;
                rows.push(StagedTraceRow {
                    input: format_word(trace.initial(), common.spaced),
                    output: format_word(trace.final_word(), common.spaced),
                    stages: build_stage_rows(trace.stages(), common.spaced),
                });
            }
            TraceOutput::Staged {
                target_stage: stage_id.to_owned(),
                rows,
            }
        }
        ExecutionTarget::Flat => {
            let mut rows = Vec::with_capacity(words.len());
            for (raw, word) in raw_words.iter().zip(words.iter()) {
                let trace = derive(word, &program.rules, &program.inventory).map_err(|source| {
                    CliError::Engine {
                        word: raw.clone(),
                        source,
                    }
                })?;
                rows.push(FlatTraceRow {
                    input: format_word(trace.initial(), common.spaced),
                    output: format_word(trace.final_word(), common.spaced),
                    steps: build_step_rows(trace.steps(), common.spaced),
                });
            }
            TraceOutput::Flat(rows)
        }
    };

    Ok(format_trace(&output, common.format))
}

fn compare_command<R: BufRead>(
    program: &Program,
    requested_stages: Option<&[String]>,
    common: &CommonArgs,
    stdin: &mut R,
) -> Result<String, CliError> {
    let Some(tree) = program.dialect_tree.as_ref() else {
        return Err(CliError::CompareRequiresDialectTree);
    };
    let selected_stages = resolve_compare_stages(tree, requested_stages)?;

    let raw_words = load_words(common, stdin)?;
    let words = parse_words(&raw_words, program, common.spaced)?;
    let mut rows = Vec::with_capacity(words.len());

    for (raw, word) in raw_words.iter().zip(words.iter()) {
        let tree_derivation = tree
            .derive_tree(word, &program.inventory)
            .map_err(|source| CliError::Engine {
                word: raw.clone(),
                source,
            })?;
        let mut outcomes = Vec::with_capacity(selected_stages.len());
        for stage_id in &selected_stages {
            let outcome_word = tree_derivation
                .word_at(stage_id)
                .ok_or_else(|| CliError::UnknownStage(stage_id.clone()))?;
            outcomes.push(format_word(outcome_word, common.spaced));
        }
        rows.push(CompareRow {
            input: format_word(tree_derivation.initial(), common.spaced),
            outcomes,
        });
    }

    Ok(format_compare(&selected_stages, &rows, common.format))
}

fn resolve_compare_stages(
    tree: &DialectTree,
    requested: Option<&[String]>,
) -> Result<Vec<String>, CliError> {
    let Some(requested) = requested else {
        return Ok(tree.leaves().into_iter().map(str::to_owned).collect());
    };
    if requested.is_empty() {
        return Err(CliError::UnknownStage(String::new()));
    }

    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(requested.len());
    for stage_id in requested {
        if !seen.insert(stage_id.as_str()) {
            return Err(CliError::DuplicateStageSelection(stage_id.clone()));
        }
        if tree.node(stage_id).is_none() {
            return Err(CliError::UnknownStage(stage_id.clone()));
        }
        selected.push(stage_id.clone());
    }
    Ok(selected)
}

fn read_rules<R: BufRead>(rules_path: &Path, stdin: &mut R) -> Result<String, CliError> {
    if is_stdin_path(rules_path) {
        let mut buf = String::new();
        stdin
            .read_to_string(&mut buf)
            .map_err(|err| CliError::io("stdin", err))?;
        return Ok(buf);
    }
    fs::read_to_string(rules_path)
        .map_err(|err| CliError::io(rules_path.display().to_string(), err))
}

fn load_words<R: BufRead>(common: &CommonArgs, stdin: &mut R) -> Result<Vec<String>, CliError> {
    if !common.words.is_empty() {
        return Ok(common.words.clone());
    }
    if let Some(path) = &common.words_file {
        let contents = fs::read_to_string(path)
            .map_err(|err| CliError::io(path.display().to_string(), err))?;
        return Ok(parse_word_lines(&contents));
    }
    let mut buf = String::new();
    stdin
        .read_to_string(&mut buf)
        .map_err(|err| CliError::io("stdin", err))?;
    Ok(parse_word_lines(&buf))
}

fn parse_word_lines(input: &str) -> Vec<String> {
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn parse_words(
    raw_words: &[String],
    program: &Program,
    spaced: bool,
) -> Result<Vec<Word>, CliError> {
    let mut parsed = Vec::with_capacity(raw_words.len());
    for raw in raw_words {
        let word = if spaced {
            Word::from_spaced(raw, &program.inventory).map_err(|source| CliError::SpacedParse {
                word: raw.clone(),
                source,
            })?
        } else {
            tokenize_word(raw, &program.inventory).map_err(|source| CliError::Tokenize {
                word: raw.clone(),
                source,
            })?
        };
        parsed.push(word);
    }
    Ok(parsed)
}

fn write_output<W: Write>(
    output_path: Option<&Path>,
    rendered: &str,
    stdout: &mut W,
) -> Result<(), CliError> {
    if let Some(path) = output_path {
        fs::write(path, rendered.as_bytes())
            .map_err(|err| CliError::io(path.display().to_string(), err))?;
    } else {
        stdout
            .write_all(rendered.as_bytes())
            .map_err(|err| CliError::io("stdout", err))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const FLAT_DSL: &str = r#"
segment a = [vowel, low, central]
segment i = [vowel, high, front]
segment u = [vowel, high, back, +rounded]
segment o = [vowel, mid-high, back, +rounded]
segment m = [nasal, bilabial, +voiced]
segment t = [stop, alveolar, -voiced]
segment d = [stop, alveolar, +voiced]
segment v = [fricative, labiodental, +voiced]

class V = { a, i, u, o }

rule R1 "Final -m deletion":
  m > ∅ / _ #

rule R3 "Intervocalic voicing":
  [-voiced, +stop] > [+voiced] / V _ V
"#;

    const TREE_DSL: &str = r#"
segment aː = [vowel, low, central, +long]
segment eː = [vowel, mid-high, front, +long]
segment r = [rhotic, alveolar, +voiced]
segment tʰ = [stop, alveolar, -voiced, +aspirated]
segment s = [fricative, alveolar, -voiced]

stage pie "Proto-Indo-European" {}

stage proto_greek "Proto-Greek" from pie {}

stage attic "Attic Greek" from proto_greek {
  rule a_raising:
    aː > eː

  rule alpha_purum:
    eː > aː / r _
}

stage ionic "Ionic Greek" from proto_greek {
  rule a_raising:
    aː > eː
}
"#;

    fn write_temp_file(name: &str, contents: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("phonengine_unit_{}_{name}", std::process::id()));
        fs::write(&path, contents).expect("write temp DSL");
        path
    }

    #[test]
    fn parse_word_lines_strips_bom_and_skips_blank_and_comment_lines() {
        let words = parse_word_lines("\u{FEFF}\n  # heading\n  // comment\n  vitam  \n\n  fatam\n");
        assert_eq!(words, vec!["vitam", "fatam"]);

        let bom_comment = parse_word_lines("\u{FEFF}# leading comment\nvitam\n");
        assert_eq!(bom_comment, vec!["vitam"]);

        let bom_word = parse_word_lines("\u{FEFF}vitam\n");
        assert_eq!(bom_word, vec!["vitam"]);
    }

    #[test]
    fn execute_supports_stdin_rules_stdin_words_and_reports_io_paths() {
        let mut stdin = Cursor::new(FLAT_DSL.as_bytes());
        let mut stdout = Vec::new();
        let cli = Cli::try_parse_from(["phonengine", "run", "-", "vitam"]).expect("valid args");
        execute(cli, &mut stdin, &mut stdout).expect("stdin rules with positional words");
        assert_eq!(String::from_utf8(stdout).expect("utf8"), "vitam -> vida\n");

        let rules_path = write_temp_file("stdin_words.phon", FLAT_DSL);
        let mut stdin_words = Cursor::new("\u{FEFF}// comment\nvitam\n\n".as_bytes());
        let mut stdout = Vec::new();
        let cli_compact =
            Cli::try_parse_from(["phonengine", "run", rules_path.to_str().expect("utf8 path")])
                .expect("valid args");
        execute(cli_compact, &mut stdin_words, &mut stdout).expect("words from stdin");
        assert_eq!(String::from_utf8(stdout).expect("utf8"), "vitam -> vida\n");

        let mut empty_stdin = Cursor::new(b"");
        let mut stdout = Vec::new();
        let conflict_cli = Cli::try_parse_from(["phonengine", "run", "-"]).expect("valid args");
        let err = execute(conflict_cli, &mut empty_stdin, &mut stdout).expect_err("stdin conflict");
        assert!(matches!(err, CliError::StdinConflict));

        let missing_cli =
            Cli::try_parse_from(["phonengine", "run", "nonexistent_rules_file.phon", "vitam"])
                .expect("valid args");
        let missing_err =
            execute(missing_cli, &mut Cursor::new(b""), &mut Vec::new()).expect_err("missing file");
        assert!(matches!(
            missing_err,
            CliError::Io { ref path, .. } if path == "nonexistent_rules_file.phon"
        ));

        let _ = fs::remove_file(rules_path);
    }

    #[test]
    fn execute_validates_stage_constraints_and_empty_inventory() {
        let rules_only_path = write_temp_file("empty_inv.phon", "rule r1:\n  a > o\n");
        let mut stdin = Cursor::new(b"");
        let mut stdout = Vec::new();
        let cli = Cli::try_parse_from([
            "phonengine",
            "run",
            rules_only_path.to_str().expect("utf8 path"),
            "a",
        ])
        .expect("valid args");
        let err = execute(cli, &mut stdin, &mut stdout).expect_err("empty inventory");
        assert!(matches!(err, CliError::EmptyInventory));
        let _ = fs::remove_file(rules_only_path);

        let flat_path = write_temp_file("flat_stage_err.phon", FLAT_DSL);
        let mut stdout = Vec::new();
        let unexpected = Cli::try_parse_from([
            "phonengine",
            "run",
            flat_path.to_str().expect("utf8 path"),
            "--stage",
            "attic",
            "vitam",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(unexpected, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::UnexpectedStage(ref s)) if s == "attic"
        ));

        let compare_flat = Cli::try_parse_from([
            "phonengine",
            "compare",
            flat_path.to_str().expect("utf8 path"),
            "vitam",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(compare_flat, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::CompareRequiresDialectTree)
        ));
        let _ = fs::remove_file(flat_path);

        let tree_path = write_temp_file("tree_stage_err.phon", TREE_DSL);
        let missing_stage = Cli::try_parse_from([
            "phonengine",
            "trace",
            tree_path.to_str().expect("utf8 path"),
            "tʰaːs",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(missing_stage, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::MissingStage)
        ));

        let unknown_stage = Cli::try_parse_from([
            "phonengine",
            "run",
            tree_path.to_str().expect("utf8 path"),
            "--stage",
            "doric",
            "tʰaːs",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(unknown_stage, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::UnknownStage(ref s)) if s == "doric"
        ));

        let empty_stages_flag = Cli::try_parse_from([
            "phonengine",
            "compare",
            tree_path.to_str().expect("utf8 path"),
            "--stages",
            "",
            "tʰaːs",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(empty_stages_flag, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::UnknownStage(ref s)) if s.is_empty()
        ));

        let dup_stages = Cli::try_parse_from([
            "phonengine",
            "compare",
            tree_path.to_str().expect("utf8 path"),
            "--stages",
            "attic,attic",
            "tʰaːs",
        ])
        .expect("valid args");
        assert!(matches!(
            execute(dup_stages, &mut Cursor::new(b""), &mut stdout),
            Err(CliError::DuplicateStageSelection(ref s)) if s == "attic"
        ));

        let valid_empty_words = Cli::try_parse_from([
            "phonengine",
            "compare",
            tree_path.to_str().expect("utf8 path"),
        ])
        .expect("valid args");
        execute(valid_empty_words, &mut Cursor::new(b""), &mut stdout).expect("empty words ok");
        assert!(stdout.is_empty());

        let _ = fs::remove_file(tree_path);
    }
}
