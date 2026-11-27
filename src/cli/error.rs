//! Error types for the PhonEngine CLI.

use std::error::Error;
use std::fmt;
use std::io;

use phonengine::{DslError, EngineError, InventoryError, TokenizeError};

/// Errors returned during CLI execution (mapped to exit code `1`).
#[derive(Debug)]
pub enum CliError {
    /// Failed to read from a file/`stdin` or write to a file/`stdout`.
    Io {
        /// File path or stream name (`"stdin"` / `"stdout"`) that failed.
        path: String,
        /// Underlying OS I/O error.
        source: io::Error,
    },
    /// Failed to lex, parse, or validate the DSL rules file.
    Dsl(DslError),
    /// The parsed DSL program declared no `segment` entries.
    EmptyInventory,
    /// Failed to tokenize an input word in compact mode.
    Tokenize {
        /// Original word string that failed tokenization.
        word: String,
        /// Underlying tokenizer error.
        source: TokenizeError,
    },
    /// Failed to parse an input word in `--spaced` mode.
    SpacedParse {
        /// Original word string that failed spaced parsing.
        word: String,
        /// Underlying inventory lookup error.
        source: InventoryError,
    },
    /// Failed while applying rules or deriving stages for a word.
    Engine {
        /// Input word being derived when the error occurred.
        word: String,
        /// Underlying engine error.
        source: EngineError,
    },
    /// `run` or `trace` was invoked on a dialect tree without `--stage`.
    MissingStage,
    /// `--stage` was passed to `run` or `trace` on a flat rule program.
    UnexpectedStage(String),
    /// `compare` was invoked on a flat rule program without `stage` blocks.
    CompareRequiresDialectTree,
    /// `--stages` listed the same stage identifier more than once.
    DuplicateStageSelection(String),
    /// A requested stage identifier does not exist in the dialect tree.
    UnknownStage(String),
    /// Both `<RULES>` and word input tried to consume `stdin`.
    StdinConflict,
}

impl CliError {
    /// Constructs a [`CliError::Io`] carrying the failing file path or stream name.
    #[must_use]
    pub fn io(path: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error on {path:?}: {source}"),
            Self::Dsl(err) => write!(f, "DSL error at {err}"),
            Self::EmptyInventory => {
                f.write_str("rules file must declare at least one `segment` inventory entry")
            }
            Self::Tokenize { word, source } => {
                write!(f, "failed to tokenize word {word:?}: {source}")
            }
            Self::SpacedParse { word, source } => {
                write!(f, "failed to parse spaced word {word:?}: {source}")
            }
            Self::Engine { word, source } => {
                write!(f, "derivation failed for word {word:?}: {source}")
            }
            Self::MissingStage => {
                f.write_str("program defines `stage` blocks and requires `--stage <STAGE_ID>`")
            }
            Self::UnexpectedStage(stage) => write!(
                f,
                "`--stage {stage}` cannot be used because the rules file defines no `stage` blocks"
            ),
            Self::CompareRequiresDialectTree => {
                f.write_str("`compare` requires a rules file with `stage` blocks")
            }
            Self::DuplicateStageSelection(stage) => {
                write!(f, "stage {stage:?} was specified more than once in `--stages`")
            }
            Self::UnknownStage(stage) => write!(f, "unknown stage {stage:?} in dialect tree"),
            Self::StdinConflict => f.write_str(
                "cannot read both rules (`-`) and words from stdin; pass words as arguments or `--words <PATH>`",
            ),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Dsl(err) => Some(err),
            Self::Tokenize { source, .. } => Some(source),
            Self::SpacedParse { source, .. } => Some(source),
            Self::Engine { source, .. } => Some(source),
            Self::EmptyInventory
            | Self::MissingStage
            | Self::UnexpectedStage(_)
            | Self::CompareRequiresDialectTree
            | Self::DuplicateStageSelection(_)
            | Self::UnknownStage(_)
            | Self::StdinConflict => None,
        }
    }
}

impl From<DslError> for CliError {
    fn from(err: DslError) -> Self {
        Self::Dsl(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phonengine::{FeatureBundle, Manner, Place, parse_program, tokenize_word};

    #[test]
    fn cli_error_variants_implement_display_and_error_source() {
        let io_err = CliError::io(
            "missing.phon",
            io::Error::new(io::ErrorKind::NotFound, "missing"),
        );
        let io_msg = io_err.to_string();
        assert!(io_msg.contains("I/O error") && io_msg.contains("missing.phon"));
        assert!(io_err.source().is_some());

        let dsl_err = parse_program("rule bad").expect_err("invalid DSL");
        let wrapped_dsl: CliError = dsl_err.into();
        assert!(wrapped_dsl.to_string().contains("DSL error"));
        assert!(wrapped_dsl.source().is_some());

        let prog = parse_program("segment a = [vowel, low, central]").expect("valid segment");
        let tok_err = tokenize_word("z", &prog.inventory).expect_err("unknown segment");
        let wrapped_tok = CliError::Tokenize {
            word: "z".to_owned(),
            source: tok_err,
        };
        assert!(wrapped_tok.to_string().contains("tokenize"));
        assert!(wrapped_tok.source().is_some());

        let spaced_err = CliError::SpacedParse {
            word: "z".to_owned(),
            source: InventoryError::UnknownSymbol("z".to_owned()),
        };
        assert!(spaced_err.to_string().contains("spaced"));
        assert!(spaced_err.source().is_some());

        let engine_err = CliError::Engine {
            word: "p".to_owned(),
            source: EngineError::UnmappedFeatureBundle {
                rule_id: "R1".to_owned(),
                bundle: FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true),
            },
        };
        assert!(engine_err.to_string().contains("derivation failed"));
        assert!(engine_err.source().is_some());

        let plain_variants = [
            CliError::EmptyInventory,
            CliError::MissingStage,
            CliError::UnexpectedStage("attic".to_owned()),
            CliError::CompareRequiresDialectTree,
            CliError::DuplicateStageSelection("ionic".to_owned()),
            CliError::UnknownStage("doric".to_owned()),
            CliError::StdinConflict,
        ];
        for err in plain_variants {
            assert!(!err.to_string().is_empty());
            assert!(err.source().is_none());
        }
    }
}
