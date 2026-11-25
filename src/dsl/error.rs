//! Diagnostic spans and error types for the PhonEngine DSL and maximal-munch tokenizer.

use std::error::Error;
use std::fmt;

use crate::engine::RuleError;
use crate::phonology::InventoryError;

/// Source span tracking byte offsets and 1-based Unicode scalar line/column coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    /// Inclusive start byte offset in the source string.
    pub start: usize,
    /// Exclusive end byte offset in the source string.
    pub end: usize,
    /// 1-based starting line number.
    pub line: usize,
    /// 1-based starting column number (counted in Unicode scalar values).
    pub column: usize,
}

impl Span {
    /// Creates a new [`Span`] with explicit byte offsets and 1-based line/column coordinates.
    #[must_use]
    pub const fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    /// Combines two spans into the smallest span covering both.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        let (start, line, column) = if self.start <= other.start {
            (self.start, self.line, self.column)
        } else {
            (other.start, other.line, other.column)
        };
        let end = if self.end >= other.end {
            self.end
        } else {
            other.end
        };
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

/// Specific lexical, syntactic, or semantic error kind produced while parsing DSL source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DslErrorKind {
    /// Encountered an unexpected character during lexing.
    UnexpectedChar(char),
    /// Reached end of line or end of input inside an unclosed string literal.
    UnterminatedString,
    /// Encountered an unsupported escape sequence inside a string literal.
    InvalidEscape(char),
    /// Encountered `$` without a valid 1-based integer index.
    InvalidSourceRef(String),
    /// Encountered a token other than what the grammar expected.
    UnexpectedToken {
        /// Human-readable description of expected syntax.
        expected: String,
        /// Description of the token actually found.
        found: String,
    },
    /// Reached end of input unexpectedly.
    UnexpectedEof {
        /// Human-readable description of expected syntax.
        expected: String,
    },
    /// Referenced an unknown feature name or enum variant inside `[...]`.
    UnknownFeature(String),
    /// Provided an invalid value for a named feature field (e.g., `-manner` or `manner=none`).
    InvalidFeatureValue {
        /// Feature field name.
        field: String,
        /// Invalid value provided.
        value: String,
    },
    /// Repeated the same feature field inside a single `[...]` block.
    DuplicateFeature(String),
    /// Declared a `segment` without specifying its required `manner`.
    MissingManner {
        /// Segment symbol missing a manner feature.
        symbol: String,
    },
    /// Inventory insertion failed while registering a `segment` declaration.
    InventoryError(InventoryError),
    /// Declared a natural class with no member symbols.
    EmptyClass(String),
    /// Declared a natural class name more than once.
    DuplicateClass(String),
    /// Listed the same member symbol twice inside a single `class` declaration.
    DuplicateClassMember {
        /// Natural class name.
        class: String,
        /// Repeated symbol.
        symbol: String,
    },
    /// Used a natural class identifier in a rule's replacement sequence.
    ClassInReplacement(String),
    /// Combined `∅`/`0` with non-empty items in a target or replacement sequence.
    EmptyMixedWithItems,
    /// Used an implicit `[...]` mutation when `target.len() != 1`.
    AmbiguousImplicitMutation {
        /// Number of target patterns in the rule.
        target_len: usize,
    },
    /// Referenced `$k` where `k == 0` or `k > target.len()`.
    InvalidSourceIndex {
        /// 1-based `$k` reference from the DSL source.
        reference: usize,
        /// Number of target patterns in the rule.
        target_len: usize,
    },
    /// Specified an unrecognized rule application mode inside `(...)`.
    UnknownApplicationMode(String),
    /// Included `/` context separator without a `_` focus marker.
    MissingContextFocus,
    /// Included more than one `_` focus marker in a rule's context.
    DuplicateContextFocus,
    /// Declared a rule identifier more than once in the same program.
    DuplicateRuleId(String),
    /// Rule construction failed engine validation invariants.
    InvalidRule(RuleError),
    /// Symbol referenced by a rule or class does not exist in the validated inventory.
    UnknownSymbol(String),
}

impl fmt::Display for DslErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(ch) => write!(f, "unexpected character {ch:?}"),
            Self::UnterminatedString => f.write_str("unterminated string literal"),
            Self::InvalidEscape(ch) => write!(f, "invalid string escape '\\{ch}'"),
            Self::InvalidSourceRef(raw) => write!(f, "invalid source reference '${raw}'"),
            Self::UnexpectedToken { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            Self::UnexpectedEof { expected } => {
                write!(f, "unexpected end of input: expected {expected}")
            }
            Self::UnknownFeature(name) => write!(f, "unknown phonological feature {name:?}"),
            Self::InvalidFeatureValue { field, value } => {
                write!(f, "invalid value {value:?} for feature {field:?}")
            }
            Self::DuplicateFeature(field) => {
                write!(f, "duplicate feature {field:?} in feature block")
            }
            Self::MissingManner { symbol } => {
                write!(f, "segment {symbol:?} must specify a manner feature")
            }
            Self::InventoryError(err) => write!(f, "{err}"),
            Self::EmptyClass(name) => write!(f, "natural class {name:?} must not be empty"),
            Self::DuplicateClass(name) => write!(f, "duplicate natural class {name:?}"),
            Self::DuplicateClassMember { class, symbol } => {
                write!(f, "duplicate symbol {symbol:?} in class {class:?}")
            }
            Self::ClassInReplacement(name) => {
                write!(
                    f,
                    "natural class {name:?} cannot be used as a rule replacement"
                )
            }
            Self::EmptyMixedWithItems => {
                f.write_str("empty symbol ('∅' or '0') cannot be combined with other items")
            }
            Self::AmbiguousImplicitMutation { target_len } => write!(
                f,
                "implicit mutation '[...]' requires a single target element, found {target_len} (use $k[...] instead)"
            ),
            Self::InvalidSourceIndex {
                reference,
                target_len,
            } => write!(
                f,
                "source reference ${reference} is out of range for target of length {target_len}"
            ),
            Self::UnknownApplicationMode(mode) => {
                write!(f, "unknown application mode {mode:?}")
            }
            Self::MissingContextFocus => {
                f.write_str("context after '/' must contain a '_' focus placeholder")
            }
            Self::DuplicateContextFocus => {
                f.write_str("context after '/' must contain only one '_' focus placeholder")
            }
            Self::DuplicateRuleId(id) => write!(f, "duplicate rule id {id:?}"),
            Self::InvalidRule(err) => write!(f, "{err}"),
            Self::UnknownSymbol(sym) => {
                write!(f, "symbol {sym:?} is not registered in inventory")
            }
        }
    }
}

/// Positioned diagnostic error returned by the DSL lexer, parser, and validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError {
    /// Specific error kind.
    pub kind: DslErrorKind,
    /// Source span where the error occurred.
    pub span: Span,
}

impl DslError {
    /// Constructs a new [`DslError`] at `span`.
    #[must_use]
    pub const fn new(kind: DslErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.span.line, self.span.column, self.kind)
    }
}

impl Error for DslError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DslErrorKind::InventoryError(err) => Some(err),
            DslErrorKind::InvalidRule(err) => Some(err),
            _ => None,
        }
    }
}

/// Specific error kind produced by the maximal-munch word tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenizeErrorKind {
    /// No registered inventory symbol (or IPA stress marker) matched at this position.
    UnknownSegment(String),
    /// Unspaced word input contained whitespace.
    UnexpectedWhitespace(char),
    /// Two consecutive IPA stress markers appeared before a segment.
    DuplicateStressMarker(char),
    /// An IPA stress marker appeared at the end of the word with no following segment.
    DanglingStressMarker(char),
}

impl fmt::Display for TokenizeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSegment(seg) => {
                write!(f, "no inventory segment matches starting at {seg:?}")
            }
            Self::UnexpectedWhitespace(ch) => {
                write!(f, "unexpected whitespace {ch:?} inside unspaced word input")
            }
            Self::DuplicateStressMarker(marker) => {
                write!(f, "duplicate stress marker {marker:?} before segment")
            }
            Self::DanglingStressMarker(marker) => {
                write!(
                    f,
                    "dangling stress marker {marker:?} at end of word without following segment"
                )
            }
        }
    }
}

/// Positioned error returned by [`tokenize_word`](crate::dsl::tokenize_word).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizeError {
    /// Specific tokenization error kind.
    pub kind: TokenizeErrorKind,
    /// Zero-based byte offset in the input string.
    pub offset: usize,
    /// 1-based Unicode scalar column in the input string.
    pub column: usize,
}

impl TokenizeError {
    /// Creates a new [`TokenizeError`] at the given byte `offset` and 1-based `column`.
    #[must_use]
    pub const fn new(kind: TokenizeErrorKind, offset: usize, column: usize) -> Self {
        Self {
            kind,
            offset,
            column,
        }
    }
}

impl fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.kind)
    }
}

impl Error for TokenizeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_merge_covers_both_spans_in_either_order() {
        let a = Span::new(2, 5, 1, 3);
        let b = Span::new(8, 12, 1, 9);
        assert_eq!(a.merge(b), Span::new(2, 12, 1, 3));
        assert_eq!(b.merge(a), Span::new(2, 12, 1, 3));
    }

    #[test]
    fn dsl_error_formats_coordinates_and_exposes_wrapped_sources() {
        let span = Span::new(0, 4, 3, 5);
        let inv_err = DslError::new(
            DslErrorKind::InventoryError(InventoryError::DuplicateSymbol("p".to_owned())),
            span,
        );
        assert_eq!(inv_err.to_string(), "3:5: duplicate inventory symbol \"p\"");
        assert!(inv_err.source().is_some());

        let rule_err = DslError::new(DslErrorKind::InvalidRule(RuleError::EmptyId), span);
        assert!(rule_err.source().is_some());

        let plain_err = DslError::new(DslErrorKind::MissingContextFocus, span);
        assert!(!plain_err.to_string().is_empty());
        assert!(plain_err.source().is_none());
    }

    #[test]
    fn tokenize_error_implements_display_and_error() {
        let errors = [
            TokenizeError::new(TokenizeErrorKind::UnknownSegment("x".to_owned()), 2, 3),
            TokenizeError::new(TokenizeErrorKind::UnexpectedWhitespace(' '), 1, 2),
            TokenizeError::new(TokenizeErrorKind::DuplicateStressMarker('ˌ'), 2, 2),
            TokenizeError::new(TokenizeErrorKind::DanglingStressMarker('ˈ'), 4, 5),
        ];
        for err in errors {
            assert!(!err.to_string().is_empty());
            let as_dyn: &dyn Error = &err;
            assert!(as_dyn.source().is_none());
        }
    }
}
