//! Command-line argument definitions using `clap` derive.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Supported output export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable plain text.
    Text,
    /// GitHub-flavored Markdown tables and headings.
    Markdown,
    /// Deterministic RFC 8259 JSON.
    Json,
}

/// Top-level CLI parser for `phonengine`.
#[derive(Debug, Parser)]
#[command(
    name = "phonengine",
    version,
    about = "Deterministic phonological evolution and comparative reconstruction engine"
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Shared options accepted by `run`, `trace`, and `compare`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CommonArgs {
    /// Path to the DSL rules file (or '-' to read rules from stdin).
    pub rules: PathBuf,

    /// Input words to derive (mutually exclusive with --words).
    #[arg(conflicts_with = "words_file")]
    pub words: Vec<String>,

    /// Read one input word per line from a UTF-8 file.
    #[arg(short = 'w', long = "words", value_name = "PATH")]
    pub words_file: Option<PathBuf>,

    /// Parse inputs and format outputs as space-separated segments.
    #[arg(long)]
    pub spaced: bool,

    /// Output format.
    #[arg(short = 'f', long = "format", value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Write output to a UTF-8 file instead of stdout.
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,
}

/// Available `phonengine` CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Derive words to their final form.
    Run {
        /// Target dialect/stage identifier (required when rules define `stage` blocks).
        #[arg(short = 's', long = "stage", value_name = "STAGE_ID")]
        stage: Option<String>,

        /// Common rule, input, and export options.
        #[command(flatten)]
        common: CommonArgs,
    },

    /// Derive words and show every non-vacuous step in the trace.
    Trace {
        /// Target dialect/stage identifier (required when rules define `stage` blocks).
        #[arg(short = 's', long = "stage", value_name = "STAGE_ID")]
        stage: Option<String>,

        /// Common rule, input, and export options.
        #[command(flatten)]
        common: CommonArgs,
    },

    /// Compare word outcomes across stages of a dialect tree.
    Compare {
        /// Comma-separated or repeated stage IDs to compare (defaults to all leaves).
        #[arg(long = "stages", value_name = "STAGE_ID", value_delimiter = ',')]
        stages: Option<Vec<String>>,

        /// Common rule, input, and export options.
        #[command(flatten)]
        common: CommonArgs,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_trace_and_compare_arguments() -> Result<(), clap::Error> {
        let run = Cli::try_parse_from([
            "phonengine",
            "run",
            "latin.phon",
            "--spaced",
            "-f",
            "json",
            "-o",
            "out.json",
            "v i t a m",
            "f a t a m",
        ])?;
        assert_eq!(
            run.command,
            Command::Run {
                stage: None,
                common: CommonArgs {
                    rules: PathBuf::from("latin.phon"),
                    words: vec!["v i t a m".to_owned(), "f a t a m".to_owned()],
                    words_file: None,
                    spaced: true,
                    format: OutputFormat::Json,
                    output: Some(PathBuf::from("out.json")),
                },
            }
        );

        let trace = Cli::try_parse_from([
            "phonengine",
            "trace",
            "-",
            "--stage",
            "attic",
            "-w",
            "words.txt",
            "--format",
            "markdown",
        ])?;
        assert_eq!(
            trace.command,
            Command::Trace {
                stage: Some("attic".to_owned()),
                common: CommonArgs {
                    rules: PathBuf::from("-"),
                    words: Vec::new(),
                    words_file: Some(PathBuf::from("words.txt")),
                    spaced: false,
                    format: OutputFormat::Markdown,
                    output: None,
                },
            }
        );

        let compare_default = Cli::try_parse_from(["phonengine", "compare", "greek.phon"])?;
        assert_eq!(
            compare_default.command,
            Command::Compare {
                stages: None,
                common: CommonArgs {
                    rules: PathBuf::from("greek.phon"),
                    words: Vec::new(),
                    words_file: None,
                    spaced: false,
                    format: OutputFormat::Text,
                    output: None,
                },
            }
        );

        let compare = Cli::try_parse_from([
            "phonengine",
            "compare",
            "greek.phon",
            "--stages",
            "attic,ionic",
            "--stages",
            "proto_greek",
            "tʰeːs",
        ])?;
        assert_eq!(
            compare.command,
            Command::Compare {
                stages: Some(vec![
                    "attic".to_owned(),
                    "ionic".to_owned(),
                    "proto_greek".to_owned(),
                ]),
                common: CommonArgs {
                    rules: PathBuf::from("greek.phon"),
                    words: vec!["tʰeːs".to_owned()],
                    words_file: None,
                    spaced: false,
                    format: OutputFormat::Text,
                    output: None,
                },
            }
        );

        let compare_empty_flag =
            Cli::try_parse_from(["phonengine", "compare", "greek.phon", "--stages", ""])?;
        assert_eq!(
            compare_empty_flag.command,
            Command::Compare {
                stages: Some(vec![String::new()]),
                common: CommonArgs {
                    rules: PathBuf::from("greek.phon"),
                    words: Vec::new(),
                    words_file: None,
                    spaced: false,
                    format: OutputFormat::Text,
                    output: None,
                },
            }
        );
        Ok(())
    }

    #[test]
    fn rejects_conflicting_words_and_words_file() {
        let conflict = Cli::try_parse_from([
            "phonengine",
            "run",
            "rules.phon",
            "--words",
            "words.txt",
            "vitam",
        ]);
        assert!(conflict.is_err());

        let unknown_flag = Cli::try_parse_from(["phonengine", "run", "rules.phon", "--bogus"]);
        assert!(unknown_flag.is_err());
    }
}
