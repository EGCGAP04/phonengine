//! Unicode-aware lexer for the PhonEngine phonological rule DSL.

use std::fmt;

use super::error::{DslError, DslErrorKind, Span};

/// Token kind produced by the DSL lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// Reserved keyword `segment`.
    Segment,
    /// Reserved keyword `class`.
    Class,
    /// Reserved keyword `rule`.
    Rule,
    /// Unquoted identifier or phonological symbol.
    Ident(String),
    /// Double-quoted string literal.
    StringLit(String),
    /// 1-based `$k` target reference.
    SourceRef(usize),
    /// Empty marker `∅` or `0`.
    Empty,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `=`
    Equals,
    /// `>`
    Gt,
    /// `/`
    Slash,
    /// `_`
    Underscore,
    /// `#`
    Hash,
    /// `*`
    Star,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// Logical line terminator (`\n` or `\r\n`).
    Newline,
    /// End of input.
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Segment => f.write_str("keyword 'segment'"),
            Self::Class => f.write_str("keyword 'class'"),
            Self::Rule => f.write_str("keyword 'rule'"),
            Self::Ident(ident) => write!(f, "{ident:?}"),
            Self::StringLit(text) => write!(f, "string {text:?}"),
            Self::SourceRef(idx) => write!(f, "'${idx}'"),
            Self::Empty => f.write_str("'∅'"),
            Self::LBrace => f.write_str("'{'"),
            Self::RBrace => f.write_str("'}'"),
            Self::LBracket => f.write_str("'['"),
            Self::RBracket => f.write_str("']'"),
            Self::LParen => f.write_str("'('"),
            Self::RParen => f.write_str("')'"),
            Self::Comma => f.write_str("','"),
            Self::Colon => f.write_str("':'"),
            Self::Equals => f.write_str("'='"),
            Self::Gt => f.write_str("'>'"),
            Self::Slash => f.write_str("'/'"),
            Self::Underscore => f.write_str("'_'"),
            Self::Hash => f.write_str("'#'"),
            Self::Star => f.write_str("'*'"),
            Self::Plus => f.write_str("'+'"),
            Self::Minus => f.write_str("'-'"),
            Self::Newline => f.write_str("newline"),
            Self::Eof => f.write_str("end of input"),
        }
    }
}

/// Single lexed token paired with its source [`Span`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

impl Token {
    const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

fn is_ident_char(ch: char) -> bool {
    !ch.is_whitespace()
        && !matches!(
            ch,
            '{' | '}'
                | '['
                | ']'
                | '('
                | ')'
                | ','
                | ':'
                | '='
                | '>'
                | '/'
                | '_'
                | '#'
                | '*'
                | '+'
                | '-'
                | '$'
                | '"'
                | ';'
                | '∅'
        )
}

struct Scanner<'a> {
    source: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Scanner<'a> {
    const fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.pos..].chars();
        chars.next()?;
        chars.next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            self.advance();
        }
    }

    fn lex_string(&mut self, start: usize, line: usize, column: usize) -> Result<Token, DslError> {
        let mut value = String::new();
        loop {
            let Some(ch) = self.peek() else {
                return Err(DslError::new(
                    DslErrorKind::UnterminatedString,
                    Span::new(start, self.pos, line, column),
                ));
            };
            if ch == '\n' || ch == '\r' {
                return Err(DslError::new(
                    DslErrorKind::UnterminatedString,
                    Span::new(start, self.pos, line, column),
                ));
            }
            self.advance();
            match ch {
                '"' => {
                    return Ok(Token::new(
                        TokenKind::StringLit(value),
                        Span::new(start, self.pos, line, column),
                    ));
                }
                '\\' => {
                    let esc_start = self.pos;
                    let esc_line = self.line;
                    let esc_col = self.column;
                    let Some(esc) = self.peek() else {
                        return Err(DslError::new(
                            DslErrorKind::UnterminatedString,
                            Span::new(start, self.pos, line, column),
                        ));
                    };
                    if esc == '\n' || esc == '\r' {
                        return Err(DslError::new(
                            DslErrorKind::UnterminatedString,
                            Span::new(start, self.pos, line, column),
                        ));
                    }
                    self.advance();
                    match esc {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        other => {
                            return Err(DslError::new(
                                DslErrorKind::InvalidEscape(other),
                                Span::new(esc_start, self.pos, esc_line, esc_col),
                            ));
                        }
                    }
                }
                other => value.push(other),
            }
        }
    }

    fn lex_source_ref(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, DslError> {
        let mut digits = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                digits.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let span = Span::new(start, self.pos, line, column);
        if digits.is_empty() {
            return Err(DslError::new(DslErrorKind::InvalidSourceRef(digits), span));
        }
        let index = digits
            .parse::<usize>()
            .map_err(|_| DslError::new(DslErrorKind::InvalidSourceRef(digits), span))?;
        Ok(Token::new(TokenKind::SourceRef(index), span))
    }

    fn lex_ident(&mut self, first: char, start: usize, line: usize, column: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while let Some(ch) = self.peek() {
            if is_ident_char(ch)
                || ((ch == '-' || ch == '_') && self.peek_next().is_some_and(is_ident_char))
            {
                text.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let kind = match text.as_str() {
            "segment" => TokenKind::Segment,
            "class" => TokenKind::Class,
            "rule" => TokenKind::Rule,
            "0" => TokenKind::Empty,
            _ => TokenKind::Ident(text),
        };
        Token::new(kind, Span::new(start, self.pos, line, column))
    }
}

/// Tokenizes `source` into a deterministic vector of [`Token`]s ending with [`TokenKind::Eof`].
pub(crate) fn lex(source: &str) -> Result<Vec<Token>, DslError> {
    let mut scanner = Scanner::new(source);
    let mut tokens = Vec::new();

    while let Some(ch) = scanner.peek() {
        let start = scanner.pos;
        let line = scanner.line;
        let column = scanner.column;

        if ch == '\r' {
            scanner.advance();
            if scanner.peek() == Some('\n') {
                scanner.advance();
                tokens.push(Token::new(
                    TokenKind::Newline,
                    Span::new(start, scanner.pos, line, column),
                ));
                continue;
            }
            return Err(DslError::new(
                DslErrorKind::UnexpectedChar('\r'),
                Span::new(start, scanner.pos, line, column),
            ));
        }

        if ch == '\n' {
            scanner.advance();
            tokens.push(Token::new(
                TokenKind::Newline,
                Span::new(start, scanner.pos, line, column),
            ));
            continue;
        }

        if ch.is_whitespace() {
            scanner.advance();
            continue;
        }

        if ch == '/' && scanner.peek_next() == Some('/') {
            scanner.advance();
            scanner.advance();
            scanner.skip_line_comment();
            continue;
        }

        scanner.advance();
        let single_span = Span::new(start, scanner.pos, line, column);
        let token = match ch {
            '"' => scanner.lex_string(start, line, column)?,
            '$' => scanner.lex_source_ref(start, line, column)?,
            '∅' => Token::new(TokenKind::Empty, single_span),
            '{' => Token::new(TokenKind::LBrace, single_span),
            '}' => Token::new(TokenKind::RBrace, single_span),
            '[' => Token::new(TokenKind::LBracket, single_span),
            ']' => Token::new(TokenKind::RBracket, single_span),
            '(' => Token::new(TokenKind::LParen, single_span),
            ')' => Token::new(TokenKind::RParen, single_span),
            ',' => Token::new(TokenKind::Comma, single_span),
            ':' => Token::new(TokenKind::Colon, single_span),
            '=' => Token::new(TokenKind::Equals, single_span),
            '>' => Token::new(TokenKind::Gt, single_span),
            '/' => Token::new(TokenKind::Slash, single_span),
            '_' => Token::new(TokenKind::Underscore, single_span),
            '#' => Token::new(TokenKind::Hash, single_span),
            '*' => Token::new(TokenKind::Star, single_span),
            '+' => Token::new(TokenKind::Plus, single_span),
            '-' => Token::new(TokenKind::Minus, single_span),
            ';' => {
                return Err(DslError::new(
                    DslErrorKind::UnexpectedChar(';'),
                    single_span,
                ));
            }
            other => scanner.lex_ident(other, start, line, column),
        };
        tokens.push(token);
    }

    tokens.push(Token::new(
        TokenKind::Eof,
        Span::new(scanner.pos, scanner.pos, scanner.line, scanner.column),
    ));
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_unicode_ipa_comments_compounds_and_adjacent_source_ref() -> Result<(), DslError> {
        let src = "// header\r\nsegment tʃ = [affricate, postalveolar, -voiced]\nrule r1 \"a\\\"b\":\n  V s > $1[+long] / _ C\n";
        let tokens = lex(src)?;
        let kinds: Vec<&TokenKind> = tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &TokenKind::Newline,
                &TokenKind::Segment,
                &TokenKind::Ident("tʃ".to_owned()),
                &TokenKind::Equals,
                &TokenKind::LBracket,
                &TokenKind::Ident("affricate".to_owned()),
                &TokenKind::Comma,
                &TokenKind::Ident("postalveolar".to_owned()),
                &TokenKind::Comma,
                &TokenKind::Minus,
                &TokenKind::Ident("voiced".to_owned()),
                &TokenKind::RBracket,
                &TokenKind::Newline,
                &TokenKind::Rule,
                &TokenKind::Ident("r1".to_owned()),
                &TokenKind::StringLit("a\"b".to_owned()),
                &TokenKind::Colon,
                &TokenKind::Newline,
                &TokenKind::Ident("V".to_owned()),
                &TokenKind::Ident("s".to_owned()),
                &TokenKind::Gt,
                &TokenKind::SourceRef(1),
                &TokenKind::LBracket,
                &TokenKind::Plus,
                &TokenKind::Ident("long".to_owned()),
                &TokenKind::RBracket,
                &TokenKind::Slash,
                &TokenKind::Underscore,
                &TokenKind::Ident("C".to_owned()),
                &TokenKind::Newline,
                &TokenKind::Eof,
            ]
        );

        let ref_pos = tokens
            .iter()
            .position(|t| t.kind == TokenKind::SourceRef(1))
            .ok_or_else(|| {
                DslError::new(DslErrorKind::UnexpectedChar('?'), Span::new(0, 0, 1, 1))
            })?;
        assert_eq!(
            tokens[ref_pos].span.end,
            tokens[ref_pos + 1].span.start,
            "$1 and '[' must be adjacent"
        );
        Ok(())
    }

    #[test]
    fn lexes_compound_hyphenated_identifiers_and_empty_aliases() -> Result<(), DslError> {
        let tokens = lex("mid-high left-to-right ∅ 0 -voiced kʷ ɲ ʔ aː")?;
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("mid-high".to_owned()),
                TokenKind::Ident("left-to-right".to_owned()),
                TokenKind::Empty,
                TokenKind::Empty,
                TokenKind::Minus,
                TokenKind::Ident("voiced".to_owned()),
                TokenKind::Ident("kʷ".to_owned()),
                TokenKind::Ident("ɲ".to_owned()),
                TokenKind::Ident("ʔ".to_owned()),
                TokenKind::Ident("aː".to_owned()),
                TokenKind::Eof,
            ]
        );
        Ok(())
    }

    #[test]
    fn reports_lexical_errors_with_accurate_positions() {
        let unterminated = lex("rule r1 \"unclosed\n");
        assert!(matches!(
            unterminated,
            Err(DslError {
                kind: DslErrorKind::UnterminatedString,
                span: Span { line: 1, .. }
            })
        ));

        let bad_escape = lex("rule r1 \"bad\\q\"");
        assert!(matches!(
            bad_escape,
            Err(DslError {
                kind: DslErrorKind::InvalidEscape('q'),
                ..
            })
        ));

        let bad_ref = lex("r > $");
        assert!(matches!(
            bad_ref,
            Err(DslError {
                kind: DslErrorKind::InvalidSourceRef(_),
                ..
            })
        ));

        let stray_semi = lex("m > ∅;");
        assert!(matches!(
            stray_semi,
            Err(DslError {
                kind: DslErrorKind::UnexpectedChar(';'),
                ..
            })
        ));
    }

    #[test]
    fn lexes_snake_case_identifiers_and_standalone_context_underscore() -> Result<(), DslError> {
        let tokens = lex("final_m_loss non_local _ # _# V _ V")?;
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("final_m_loss".to_owned()),
                TokenKind::Ident("non_local".to_owned()),
                TokenKind::Underscore,
                TokenKind::Hash,
                TokenKind::Underscore,
                TokenKind::Hash,
                TokenKind::Ident("V".to_owned()),
                TokenKind::Underscore,
                TokenKind::Ident("V".to_owned()),
                TokenKind::Eof,
            ]
        );
        Ok(())
    }
}
