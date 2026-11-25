//! Phonological rule DSL lexer, recursive-descent parser, diagnostics, and maximal-munch tokenizer.

pub mod error;
pub mod lexer;
pub mod parser;
pub mod tokenizer;

pub use error::{DslError, DslErrorKind, Span, TokenizeError, TokenizeErrorKind};
pub use parser::{Program, parse_program};
pub use tokenizer::tokenize_word;
