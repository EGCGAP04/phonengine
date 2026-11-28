//! Manual recursive-descent parser for the PhonEngine phonological rule DSL.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use crate::engine::{
    ApplicationMode, ConstrainedPattern, ContextElement, DialectError, DialectNode, DialectTree,
    Environment, NaturalClass, Pattern, PatternBase, ReplacementItem, Rule, SegmentMutation,
};
use crate::phonology::{
    Backness, FeatureBundle, FeatureField, FeatureMutation, FeaturePattern, FeatureVar, FieldMatch,
    FieldUpdate, Height, Inventory, LiteralStress, Manner, Place, Stress, StressMatch,
    StressUpdate, VarPolarity,
};

use super::error::{DslError, DslErrorKind, Span};
use super::lexer::{Token, TokenKind, lex};

/// Complete parsed DSL program containing an optional inventory, natural classes, rules,
/// and an optional dialect tree.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Segment inventory populated from `segment` declarations (empty if none are present).
    pub inventory: Inventory,
    /// Declared natural classes keyed deterministically by name.
    pub classes: BTreeMap<String, NaturalClass>,
    /// Ordered top-level phonological rules (empty when `stage` blocks are declared).
    pub rules: Vec<Rule>,
    /// Rooted chronological dialect tree populated from `stage` blocks, if present.
    pub dialect_tree: Option<DialectTree>,
    symbol_refs: Vec<SymbolRef>,
}

/// Literal segment symbol referenced in the source, recorded for [`Program::validate_against`].
#[derive(Debug, Clone)]
struct SymbolRef {
    symbol: String,
    span: Span,
    /// `true` for target and context patterns, where a later `class` name may have been meant.
    in_pattern: bool,
}

impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.inventory == other.inventory
            && self.classes == other.classes
            && self.rules == other.rules
            && self.dialect_tree == other.dialect_tree
    }
}

impl Eq for Program {}

impl Program {
    /// Validates that every literal segment symbol referenced in `class` declarations,
    /// target/context patterns, and literal replacements exists in `inventory`.
    ///
    /// Only the symbols recorded while parsing are checked: rules added to or changed in this
    /// [`Program`] afterwards are not revalidated.
    ///
    /// # Errors
    ///
    /// Returns [`DslError`] at the first unmapped symbol's source [`Span`], with
    /// [`DslErrorKind::ClassUsedBeforeDeclaration`] if a pattern names a class declared later,
    /// or [`DslErrorKind::UnknownSymbol`] otherwise.
    pub fn validate_against(&self, inventory: &Inventory) -> Result<(), DslError> {
        for symbol_ref in &self.symbol_refs {
            if inventory.get(&symbol_ref.symbol).is_none() {
                let kind = if symbol_ref.in_pattern && self.classes.contains_key(&symbol_ref.symbol)
                {
                    DslErrorKind::ClassUsedBeforeDeclaration(symbol_ref.symbol.clone())
                } else {
                    DslErrorKind::UnknownSymbol(symbol_ref.symbol.clone())
                };
                return Err(DslError::new(kind, symbol_ref.span));
            }
        }
        Ok(())
    }
}

/// Parses a PhonEngine DSL source string into a validated [`Program`].
///
/// If the program declares at least one `segment`, its symbols are also validated against that
/// inventory with [`Program::validate_against`]; otherwise the caller must validate them against
/// the inventory it will use.
///
/// # Errors
///
/// Returns a positioned [`DslError`] on lexical, syntactic, or semantic validation failure.
pub fn parse_program(source: &str) -> Result<Program, DslError> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedStress {
    /// `+stressed`: matches `Primary | Secondary`; sets `Primary` in mutations/literals.
    StressedFlag,
    /// `stress=unstressed|primary|secondary` or `-stressed`.
    Exact(Stress),
}

impl ParsedStress {
    const fn to_match(self) -> StressMatch {
        match self {
            Self::StressedFlag => StressMatch::Stressed,
            Self::Exact(level) => StressMatch::Is(level),
        }
    }

    const fn to_stress(self) -> Stress {
        match self {
            Self::StressedFlag => Stress::Primary,
            Self::Exact(level) => level,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FeatureValue {
    Manner(Manner),
    Place(Option<Place>),
    Height(Option<Height>),
    Backness(Option<Backness>),
    Bool(bool),
    Stress(ParsedStress),
    Var { name: String, polarity: VarPolarity },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeatureEntry {
    field: FeatureField,
    value: FeatureValue,
    span: Span,
}

impl FeatureEntry {
    const fn new(field: FeatureField, value: FeatureValue, span: Span) -> Self {
        Self { field, value, span }
    }
}

enum EnumVariant {
    Manner(Manner),
    Place(Place),
    Height(Height),
    Backness(Backness),
}

impl EnumVariant {
    const fn field(&self) -> FeatureField {
        match self {
            Self::Manner(_) => FeatureField::Manner,
            Self::Place(_) => FeatureField::Place,
            Self::Height(_) => FeatureField::Height,
            Self::Backness(_) => FeatureField::Backness,
        }
    }

    const fn into_value(self) -> FeatureValue {
        match self {
            Self::Manner(m) => FeatureValue::Manner(m),
            Self::Place(p) => FeatureValue::Place(Some(p)),
            Self::Height(h) => FeatureValue::Height(Some(h)),
            Self::Backness(b) => FeatureValue::Backness(Some(b)),
        }
    }
}

fn parse_manner(name: &str) -> Option<Manner> {
    match name {
        "stop" => Some(Manner::Stop),
        "fricative" => Some(Manner::Fricative),
        "affricate" => Some(Manner::Affricate),
        "nasal" => Some(Manner::Nasal),
        "lateral" => Some(Manner::Lateral),
        "rhotic" => Some(Manner::Rhotic),
        "approximant" => Some(Manner::Approximant),
        "vowel" => Some(Manner::Vowel),
        _ => None,
    }
}

fn parse_place(name: &str) -> Option<Place> {
    match name {
        "bilabial" => Some(Place::Bilabial),
        "labiodental" => Some(Place::Labiodental),
        "dental" => Some(Place::Dental),
        "alveolar" => Some(Place::Alveolar),
        "postalveolar" => Some(Place::Postalveolar),
        "palatal" => Some(Place::Palatal),
        "velar" => Some(Place::Velar),
        "uvular" => Some(Place::Uvular),
        "pharyngeal" => Some(Place::Pharyngeal),
        "glottal" => Some(Place::Glottal),
        _ => None,
    }
}

fn parse_height(name: &str) -> Option<Height> {
    match name {
        "high" => Some(Height::High),
        "mid-high" => Some(Height::MidHigh),
        "mid-low" => Some(Height::MidLow),
        "low" => Some(Height::Low),
        _ => None,
    }
}

fn parse_backness(name: &str) -> Option<Backness> {
    match name {
        "front" => Some(Backness::Front),
        "central" => Some(Backness::Central),
        "back" => Some(Backness::Back),
        _ => None,
    }
}

fn parse_enum_variant(name: &str) -> Option<EnumVariant> {
    if let Some(m) = parse_manner(name) {
        return Some(EnumVariant::Manner(m));
    }
    if let Some(p) = parse_place(name) {
        return Some(EnumVariant::Place(p));
    }
    if let Some(h) = parse_height(name) {
        return Some(EnumVariant::Height(h));
    }
    if let Some(b) = parse_backness(name) {
        return Some(EnumVariant::Backness(b));
    }
    None
}

fn parse_bool_field(name: &str) -> Option<FeatureField> {
    match name {
        "voiced" => Some(FeatureField::Voiced),
        "aspirated" => Some(FeatureField::Aspirated),
        "palatalized" => Some(FeatureField::Palatalized),
        "labialized" => Some(FeatureField::Labialized),
        "rounded" => Some(FeatureField::Rounded),
        "syllabic" => Some(FeatureField::Syllabic),
        "long" => Some(FeatureField::Long),
        _ => None,
    }
}

fn parse_field_name(name: &str) -> Option<FeatureField> {
    match name {
        "manner" => Some(FeatureField::Manner),
        "place" => Some(FeatureField::Place),
        "height" => Some(FeatureField::Height),
        "backness" => Some(FeatureField::Backness),
        "stress" => Some(FeatureField::Stress),
        other => parse_bool_field(other),
    }
}

fn is_greek_var_char(ch: char) -> bool {
    ('\u{03B1}'..='\u{03C9}').contains(&ch)
}

fn parse_explicit_var_name(raw: &str) -> Option<String> {
    let mut chars = raw.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) if is_greek_var_char(ch) => Some(raw.to_owned()),
        _ => raw.strip_prefix('@').and_then(|rest| {
            (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| raw.to_owned())
        }),
    }
}

fn parse_prefixed_greek_var(ident: &str) -> Option<(String, FeatureField)> {
    let mut chars = ident.chars();
    let first = chars.next()?;
    if !is_greek_var_char(first) {
        return None;
    }
    let rest = chars.as_str();
    if rest.is_empty() {
        return None;
    }
    let field = parse_field_name(rest)?;
    Some((first.to_string(), field))
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    inventory: Inventory,
    classes: BTreeMap<String, NaturalClass>,
    rules: Vec<Rule>,
    rule_ids: BTreeSet<String>,
    stages: Vec<(DialectNode, Span, Span)>,
    symbol_refs: Vec<SymbolRef>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            inventory: Inventory::new(),
            classes: BTreeMap::new(),
            rules: Vec::new(),
            rule_ids: BTreeSet::new(),
            stages: Vec::new(),
            symbol_refs: Vec::new(),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.cursor)
            .or_else(|| self.tokens.last())
            .unwrap_or(&EOF_TOKEN)
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn advance(&mut self) -> Token {
        if self.cursor < self.tokens.len() {
            let tok = self.tokens[self.cursor].clone();
            if tok.kind != TokenKind::Eof {
                self.cursor += 1;
            }
            tok
        } else {
            EOF_TOKEN.clone()
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    fn peek_non_newline_kind(&self) -> &TokenKind {
        let mut idx = self.cursor;
        while let Some(tok) = self.tokens.get(idx) {
            if tok.kind != TokenKind::Newline {
                return &tok.kind;
            }
            idx += 1;
        }
        &TokenKind::Eof
    }

    /// Records a literal symbol reference for [`Program::validate_against`].
    fn record_symbol(&mut self, symbol: String, span: Span, in_pattern: bool) {
        self.symbol_refs.push(SymbolRef {
            symbol,
            span,
            in_pattern,
        });
    }

    fn expect_line_end(&mut self) -> Result<(), DslError> {
        match self.peek_kind() {
            TokenKind::Newline | TokenKind::Eof => {
                self.skip_newlines();
                Ok(())
            }
            other => Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: "end of line".to_owned(),
                    found: other.to_string(),
                },
                self.peek().span,
            )),
        }
    }

    fn expect_rule_end(&mut self, allow_rbrace_end: bool) -> Result<(), DslError> {
        if allow_rbrace_end && matches!(self.peek_kind(), TokenKind::RBrace) {
            return Ok(());
        }
        self.expect_line_end()
    }

    fn expect_ident(&mut self, expected: &str) -> Result<(String, Span), DslError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, tok.span))
            }
            TokenKind::Eof => Err(DslError::new(
                DslErrorKind::UnexpectedEof {
                    expected: expected.to_owned(),
                },
                tok.span,
            )),
            other => Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: expected.to_owned(),
                    found: other.to_string(),
                },
                tok.span,
            )),
        }
    }

    fn expect_token(
        &mut self,
        expected_kind: &TokenKind,
        expected_desc: &str,
    ) -> Result<Span, DslError> {
        let tok = self.peek().clone();
        if &tok.kind == expected_kind {
            self.advance();
            Ok(tok.span)
        } else if tok.kind == TokenKind::Eof {
            Err(DslError::new(
                DslErrorKind::UnexpectedEof {
                    expected: expected_desc.to_owned(),
                },
                tok.span,
            ))
        } else {
            Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: expected_desc.to_owned(),
                    found: tok.kind.to_string(),
                },
                tok.span,
            ))
        }
    }

    fn parse_program(&mut self) -> Result<Program, DslError> {
        loop {
            self.skip_newlines();
            match self.peek_kind() {
                TokenKind::Eof => break,
                TokenKind::Segment => self.parse_segment_decl()?,
                TokenKind::Class => self.parse_class_decl()?,
                TokenKind::Rule => {
                    if !self.stages.is_empty() {
                        return Err(DslError::new(
                            DslErrorKind::MixedTopLevelRulesAndStages,
                            self.peek().span,
                        ));
                    }
                    let mut ids = std::mem::take(&mut self.rule_ids);
                    let rule_res = self.parse_rule_decl(&mut ids, false);
                    self.rule_ids = ids;
                    self.rules.push(rule_res?);
                }
                TokenKind::Stage => {
                    if !self.rules.is_empty() {
                        return Err(DslError::new(
                            DslErrorKind::MixedTopLevelRulesAndStages,
                            self.peek().span,
                        ));
                    }
                    self.parse_stage_decl()?;
                }
                other => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "'segment', 'class', 'rule', or 'stage' declaration"
                                .to_owned(),
                            found: other.to_string(),
                        },
                        self.peek().span,
                    ));
                }
            }
        }

        let dialect_tree = if self.stages.is_empty() {
            None
        } else {
            let stages = std::mem::take(&mut self.stages);
            let mut nodes = Vec::with_capacity(stages.len());
            let mut spans = StageSpans::default();
            let mut combined_span: Option<Span> = None;

            for (node, header_span, full_span) in stages {
                match spans.first.entry(node.id().to_owned()) {
                    Entry::Vacant(slot) => {
                        slot.insert(header_span);
                    }
                    Entry::Occupied(slot) => {
                        spans
                            .repeated
                            .entry(slot.key().clone())
                            .or_insert(header_span);
                    }
                }
                combined_span = Some(match combined_span {
                    Some(prev) => prev.merge(full_span),
                    None => full_span,
                });
                nodes.push(node);
            }

            let fallback_span = combined_span.unwrap_or(EOF_TOKEN.span);
            let tree = DialectTree::new(nodes).map_err(|err| {
                let span = dialect_error_span(&err, &spans, fallback_span);
                DslError::new(DslErrorKind::InvalidDialectTree(err), span)
            })?;
            Some(tree)
        };

        let program = Program {
            inventory: std::mem::take(&mut self.inventory),
            classes: std::mem::take(&mut self.classes),
            rules: std::mem::take(&mut self.rules),
            dialect_tree,
            symbol_refs: std::mem::take(&mut self.symbol_refs),
        };
        // A program that declares segments carries its own inventory: validate against it.
        if !program.inventory.is_empty() {
            program.validate_against(&program.inventory)?;
        }
        Ok(program)
    }

    fn parse_stage_decl(&mut self) -> Result<(), DslError> {
        let kw_span = self.advance().span;
        let (id, id_span) = self.expect_ident("stage identifier")?;
        let header_span = kw_span.merge(id_span);

        let name = if let TokenKind::StringLit(label) = self.peek_kind().clone() {
            self.advance();
            label
        } else {
            id.clone()
        };

        let parent = if matches!(self.peek_kind(), TokenKind::Ident(kw) if kw == "from") {
            self.advance();
            let (parent_id, _) = self.expect_ident("parent stage identifier")?;
            Some(parent_id)
        } else {
            None
        };

        self.expect_token(&TokenKind::LBrace, "'{'")?;
        let mut stage_rules = Vec::new();
        let mut stage_rule_ids = BTreeSet::new();

        loop {
            self.skip_newlines();
            match self.peek_kind() {
                TokenKind::RBrace => break,
                TokenKind::Rule => {
                    stage_rules.push(self.parse_rule_decl(&mut stage_rule_ids, true)?);
                }
                TokenKind::Eof => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedEof {
                            expected: "'rule' declaration or '}'".to_owned(),
                        },
                        self.peek().span,
                    ));
                }
                other => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "'rule' declaration or '}'".to_owned(),
                            found: other.to_string(),
                        },
                        self.peek().span,
                    ));
                }
            }
        }

        let close_span = self.expect_token(&TokenKind::RBrace, "'}'")?;
        self.expect_line_end()?;

        self.stages.push((
            DialectNode::new(id, name, parent, stage_rules),
            header_span,
            kw_span.merge(close_span),
        ));
        Ok(())
    }

    fn parse_segment_decl(&mut self) -> Result<(), DslError> {
        let kw_span = self.advance().span;
        let (symbol, _) = self.expect_ident("segment symbol")?;
        self.expect_token(&TokenKind::Equals, "'='")?;
        let (entries, block_span) = self.parse_feature_entries(true)?;
        self.expect_line_end()?;

        let mut manner: Option<Manner> = None;
        for entry in &entries {
            match &entry.value {
                FeatureValue::Stress(_) => {
                    return Err(DslError::new(DslErrorKind::StressInSegmentDecl, entry.span));
                }
                FeatureValue::Var { name, .. } => {
                    return Err(DslError::new(
                        DslErrorKind::VariableInSegmentDecl(name.clone()),
                        entry.span,
                    ));
                }
                FeatureValue::Manner(m) if entry.field == FeatureField::Manner => {
                    manner = Some(*m);
                }
                _ => {}
            }
        }
        let Some(manner) = manner else {
            return Err(DslError::new(
                DslErrorKind::MissingManner { symbol },
                block_span,
            ));
        };

        let is_vowel = manner == Manner::Vowel;
        let mut bundle = FeatureBundle {
            manner,
            place: None,
            height: None,
            backness: None,
            voiced: is_vowel,
            aspirated: false,
            palatalized: false,
            labialized: false,
            rounded: false,
            syllabic: is_vowel,
            long: false,
        };

        for entry in entries {
            match (entry.field, entry.value) {
                (FeatureField::Manner, FeatureValue::Manner(m)) => bundle.manner = m,
                (FeatureField::Place, FeatureValue::Place(p)) => bundle.place = p,
                (FeatureField::Height, FeatureValue::Height(h)) => bundle.height = h,
                (FeatureField::Backness, FeatureValue::Backness(b)) => bundle.backness = b,
                (FeatureField::Voiced, FeatureValue::Bool(v)) => bundle.voiced = v,
                (FeatureField::Aspirated, FeatureValue::Bool(v)) => bundle.aspirated = v,
                (FeatureField::Palatalized, FeatureValue::Bool(v)) => bundle.palatalized = v,
                (FeatureField::Labialized, FeatureValue::Bool(v)) => bundle.labialized = v,
                (FeatureField::Rounded, FeatureValue::Bool(v)) => bundle.rounded = v,
                (FeatureField::Syllabic, FeatureValue::Bool(v)) => bundle.syllabic = v,
                (FeatureField::Long, FeatureValue::Bool(v)) => bundle.long = v,
                _ => {}
            }
        }

        let decl_span = kw_span.merge(block_span);
        self.inventory
            .insert(symbol, bundle)
            .map_err(|err| DslError::new(DslErrorKind::InventoryError(err), decl_span))
    }

    fn parse_class_decl(&mut self) -> Result<(), DslError> {
        self.advance();
        let (name, name_span) = self.expect_ident("natural class name")?;
        if self.classes.contains_key(&name) {
            return Err(DslError::new(DslErrorKind::DuplicateClass(name), name_span));
        }

        self.expect_token(&TokenKind::Equals, "'='")?;
        let open_span = self.expect_token(&TokenKind::LBrace, "'{'")?;

        let mut members = BTreeSet::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek_kind(), TokenKind::RBrace) {
                break;
            }
            let (member, member_span) = self.expect_ident("segment symbol")?;
            if !members.insert(member.clone()) {
                return Err(DslError::new(
                    DslErrorKind::DuplicateClassMember {
                        class: name,
                        symbol: member,
                    },
                    member_span,
                ));
            }
            self.record_symbol(member, member_span, false);

            self.skip_newlines();
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        self.skip_newlines();
        let close_span = self.expect_token(&TokenKind::RBrace, "'}'")?;
        self.expect_line_end()?;

        if members.is_empty() {
            return Err(DslError::new(
                DslErrorKind::EmptyClass(name),
                open_span.merge(close_span),
            ));
        }

        self.classes.insert(
            name.clone(),
            NaturalClass {
                name: name.clone(),
                members,
            },
        );
        Ok(())
    }

    fn parse_rule_decl(
        &mut self,
        seen_rule_ids: &mut BTreeSet<String>,
        allow_rbrace_end: bool,
    ) -> Result<Rule, DslError> {
        let kw_span = self.advance().span;
        let (id, id_span) = self.expect_ident("rule identifier")?;
        if !seen_rule_ids.insert(id.clone()) {
            return Err(DslError::new(DslErrorKind::DuplicateRuleId(id), id_span));
        }

        let name = if let TokenKind::StringLit(label) = self.peek_kind().clone() {
            self.advance();
            label
        } else {
            id.clone()
        };

        let mode = if matches!(self.peek_kind(), TokenKind::LParen) {
            self.advance();
            let (mode_name, mode_span) = self.expect_ident("application mode")?;
            self.expect_token(&TokenKind::RParen, "')'")?;
            match mode_name.as_str() {
                "simultaneous" => ApplicationMode::Simultaneous,
                "left-to-right" => ApplicationMode::LeftToRight,
                "right-to-left" => ApplicationMode::RightToLeft,
                _ => {
                    return Err(DslError::new(
                        DslErrorKind::UnknownApplicationMode(mode_name),
                        mode_span,
                    ));
                }
            }
        } else {
            ApplicationMode::Simultaneous
        };

        self.expect_token(&TokenKind::Colon, "':'")?;
        self.skip_newlines();

        let (target, target_span) = self.parse_rule_target()?;
        self.expect_token(&TokenKind::Gt, "'>'")?;
        let (replacement, repl_span) =
            self.parse_rule_replacement(target.len(), allow_rbrace_end)?;

        let (environments, end_span) = if matches!(self.peek_kind(), TokenKind::Slash) {
            self.parse_rule_contexts(allow_rbrace_end)?
        } else {
            (Vec::new(), repl_span)
        };

        self.expect_rule_end(allow_rbrace_end)?;

        let rule_span = kw_span.merge(target_span).merge(end_span);
        Rule::with_environments(id, name, target, replacement, environments, mode)
            .map_err(|err| DslError::new(DslErrorKind::InvalidRule(err), rule_span))
    }

    fn parse_rule_target(&mut self) -> Result<(Vec<Pattern>, Span), DslError> {
        let mut patterns = Vec::new();
        let mut saw_empty = false;
        let mut combined_span: Option<Span> = None;

        while !matches!(self.peek_kind(), TokenKind::Gt) {
            let tok = self.peek().clone();
            match tok.kind {
                TokenKind::Empty => {
                    self.advance();
                    if !patterns.is_empty() || saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    saw_empty = true;
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(tok.span),
                        None => tok.span,
                    });
                }
                TokenKind::Ident(_) | TokenKind::LBracket => {
                    if saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    let (pattern, pat_span) = self.parse_pattern()?;
                    patterns.push(pattern);
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(pat_span),
                        None => pat_span,
                    });
                }
                TokenKind::Newline => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "'>'".to_owned(),
                            found: tok.kind.to_string(),
                        },
                        tok.span,
                    ));
                }
                TokenKind::Eof => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedEof {
                            expected: "'>'".to_owned(),
                        },
                        tok.span,
                    ));
                }
                other => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "target pattern, '∅', or '>'".to_owned(),
                            found: other.to_string(),
                        },
                        tok.span,
                    ));
                }
            }
        }

        let Some(span) = combined_span else {
            return Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: "target pattern or '∅'".to_owned(),
                    found: self.peek_kind().to_string(),
                },
                self.peek().span,
            ));
        };
        Ok((patterns, span))
    }

    fn parse_rule_replacement(
        &mut self,
        target_len: usize,
        allow_rbrace_end: bool,
    ) -> Result<(Vec<ReplacementItem>, Span), DslError> {
        let mut items = Vec::new();
        let mut saw_empty = false;
        let mut combined_span: Option<Span> = None;

        while !matches!(
            self.peek_kind(),
            TokenKind::Slash | TokenKind::Pipe | TokenKind::Newline | TokenKind::Eof
        ) && !(allow_rbrace_end && matches!(self.peek_kind(), TokenKind::RBrace))
        {
            let tok = self.peek().clone();
            match tok.kind {
                TokenKind::Empty => {
                    self.advance();
                    if !items.is_empty() || saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    saw_empty = true;
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(tok.span),
                        None => tok.span,
                    });
                }
                TokenKind::Ident(sym) => {
                    if saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    self.advance();
                    if self.classes.contains_key(&sym) {
                        return Err(DslError::new(
                            DslErrorKind::ClassInReplacement(sym),
                            tok.span,
                        ));
                    }
                    self.record_symbol(sym.clone(), tok.span, false);
                    let (item, item_span) = if matches!(self.peek_kind(), TokenKind::LBracket)
                        && self.peek().span.start == tok.span.end
                    {
                        let (entries, bracket_span) = self.parse_feature_entries(false)?;
                        let stress = entries_to_literal_stress(entries)?;
                        let item = match stress {
                            LiteralStress::Inherit => ReplacementItem::Literal(sym),
                            other => ReplacementItem::LiteralWithStress {
                                symbol: sym,
                                stress: other,
                            },
                        };
                        (item, tok.span.merge(bracket_span))
                    } else {
                        (ReplacementItem::Literal(sym), tok.span)
                    };
                    items.push(item);
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(item_span),
                        None => item_span,
                    });
                }
                TokenKind::SourceRef(reference) => {
                    if saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    self.advance();
                    if reference == 0 || reference > target_len {
                        return Err(DslError::new(
                            DslErrorKind::InvalidSourceIndex {
                                reference,
                                target_len,
                            },
                            tok.span,
                        ));
                    }
                    let (mutation, item_span) = if matches!(self.peek_kind(), TokenKind::LBracket)
                        && self.peek().span.start == tok.span.end
                    {
                        let (mut_spec, mut_span) = self.parse_segment_mutation(false)?;
                        (mut_spec, tok.span.merge(mut_span))
                    } else {
                        (SegmentMutation::new(), tok.span)
                    };
                    items.push(ReplacementItem::Mutate {
                        source_index: reference - 1,
                        mutation,
                    });
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(item_span),
                        None => item_span,
                    });
                }
                TokenKind::LBracket => {
                    if saw_empty {
                        return Err(DslError::new(DslErrorKind::EmptyMixedWithItems, tok.span));
                    }
                    if target_len != 1 {
                        return Err(DslError::new(
                            DslErrorKind::AmbiguousImplicitMutation { target_len },
                            tok.span,
                        ));
                    }
                    let (mutation, mut_span) = self.parse_segment_mutation(false)?;
                    items.push(ReplacementItem::Mutate {
                        source_index: 0,
                        mutation,
                    });
                    combined_span = Some(match combined_span {
                        Some(prev) => prev.merge(mut_span),
                        None => mut_span,
                    });
                }
                other => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "replacement literal, '$k', '[...]', or '∅'".to_owned(),
                            found: other.to_string(),
                        },
                        tok.span,
                    ));
                }
            }
        }

        let Some(span) = combined_span else {
            return Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: "replacement pattern or '∅'".to_owned(),
                    found: self.peek_kind().to_string(),
                },
                self.peek().span,
            ));
        };
        Ok((items, span))
    }

    fn parse_rule_contexts(
        &mut self,
        allow_rbrace_end: bool,
    ) -> Result<(Vec<Environment>, Span), DslError> {
        self.advance();
        let mut environments = Vec::new();
        let mut last_span;

        loop {
            let (env, branch_span) = self.parse_context_branch(allow_rbrace_end)?;
            environments.push(env);
            last_span = branch_span;

            if matches!(self.peek_kind(), TokenKind::Pipe) {
                self.advance();
                self.skip_newlines();
            } else if matches!(self.peek_kind(), TokenKind::Newline)
                && matches!(self.peek_non_newline_kind(), TokenKind::Pipe)
            {
                self.skip_newlines();
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        Ok((environments, last_span))
    }

    fn parse_context_branch(
        &mut self,
        allow_rbrace_end: bool,
    ) -> Result<(Environment, Span), DslError> {
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut saw_focus = false;
        let mut last_span = self.peek().span;
        // First identifier containing `_` (e.g. `V_V`), reported if no standalone focus appears.
        let mut focus_in_ident: Option<(String, Span)> = None;

        while !matches!(
            self.peek_kind(),
            TokenKind::Pipe | TokenKind::Newline | TokenKind::Eof
        ) && !(allow_rbrace_end && matches!(self.peek_kind(), TokenKind::RBrace))
        {
            let tok = self.peek().clone();
            match tok.kind {
                TokenKind::Underscore => {
                    self.advance();
                    if saw_focus {
                        return Err(DslError::new(DslErrorKind::DuplicateContextFocus, tok.span));
                    }
                    saw_focus = true;
                    last_span = tok.span;
                }
                TokenKind::Hash => {
                    self.advance();
                    if matches!(self.peek_kind(), TokenKind::Star) {
                        return Err(DslError::new(
                            DslErrorKind::UnexpectedToken {
                                expected: "pattern before '*'".to_owned(),
                                found: "'#'".to_owned(),
                            },
                            tok.span.merge(self.peek().span),
                        ));
                    }
                    let dest = if saw_focus { &mut right } else { &mut left };
                    dest.push(ContextElement::WordBoundary);
                    last_span = tok.span;
                }
                TokenKind::LParen => {
                    let open_span = self.advance().span;
                    let next_tok = self.peek().clone();
                    match &next_tok.kind {
                        TokenKind::Ident(name) => {
                            if name.contains('_') && focus_in_ident.is_none() {
                                focus_in_ident = Some((name.clone(), next_tok.span));
                            }
                        }
                        TokenKind::LBracket => {}
                        TokenKind::Eof => {
                            return Err(DslError::new(
                                DslErrorKind::UnexpectedEof {
                                    expected: "pattern inside '(...)'".to_owned(),
                                },
                                next_tok.span,
                            ));
                        }
                        other => {
                            return Err(DslError::new(
                                DslErrorKind::UnexpectedToken {
                                    expected: "pattern inside '(...)'".to_owned(),
                                    found: other.to_string(),
                                },
                                next_tok.span,
                            ));
                        }
                    }
                    let (pattern, _) = self.parse_pattern()?;
                    let close_span = self.expect_token(&TokenKind::RParen, "')'")?;
                    if matches!(self.peek_kind(), TokenKind::Star) {
                        let star_span = self.advance().span;
                        return Err(DslError::new(
                            DslErrorKind::UnexpectedToken {
                                expected: "context element (cannot apply '*' to optional '(...)')"
                                    .to_owned(),
                                found: "'*'".to_owned(),
                            },
                            open_span.merge(star_span),
                        ));
                    }
                    let elem_span = open_span.merge(close_span);
                    let dest = if saw_focus { &mut right } else { &mut left };
                    dest.push(ContextElement::Optional(pattern));
                    last_span = elem_span;
                }
                TokenKind::Ident(_) | TokenKind::LBracket => {
                    if let TokenKind::Ident(name) = &tok.kind
                        && name.contains('_')
                        && focus_in_ident.is_none()
                    {
                        focus_in_ident = Some((name.clone(), tok.span));
                    }
                    let (pattern, pat_span) = self.parse_pattern()?;
                    let (elem, elem_span) = if matches!(self.peek_kind(), TokenKind::Star) {
                        let star_span = self.advance().span;
                        (
                            ContextElement::ZeroOrMore(pattern),
                            pat_span.merge(star_span),
                        )
                    } else {
                        (ContextElement::Segment(pattern), pat_span)
                    };
                    let dest = if saw_focus { &mut right } else { &mut left };
                    dest.push(elem);
                    last_span = elem_span;
                }
                other => {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "context element ('#', pattern, '(pattern)', or '_')"
                                .to_owned(),
                            found: other.to_string(),
                        },
                        tok.span,
                    ));
                }
            }
        }

        if !saw_focus {
            return Err(match focus_in_ident {
                Some((ident, span)) => {
                    DslError::new(DslErrorKind::FocusInsideIdentifier(ident), span)
                }
                None => DslError::new(DslErrorKind::MissingContextFocus, last_span),
            });
        }

        Ok((Environment::new(left, right), last_span))
    }

    fn parse_pattern(&mut self) -> Result<(Pattern, Span), DslError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(sym) => {
                self.advance();
                let base = if let Some(class) = self.classes.get(&sym) {
                    PatternBase::Class(class.clone())
                } else {
                    self.record_symbol(sym.clone(), tok.span, true);
                    PatternBase::Literal(sym)
                };

                if matches!(self.peek_kind(), TokenKind::LBracket)
                    && self.peek().span.start == tok.span.end
                {
                    let (entries, bracket_span) = self.parse_feature_entries(false)?;
                    let (stress, vars) = entries_to_non_feature_constraints(entries)?;
                    let pat = if stress == StressMatch::Any && vars.is_empty() {
                        match base {
                            PatternBase::Literal(s) => Pattern::Literal(s),
                            PatternBase::Class(c) => Pattern::Class(c),
                            PatternBase::Features(f) => Pattern::Features(f),
                        }
                    } else {
                        Pattern::Constrained(ConstrainedPattern { base, stress, vars })
                    };
                    Ok((pat, tok.span.merge(bracket_span)))
                } else {
                    let pat = match base {
                        PatternBase::Literal(s) => Pattern::Literal(s),
                        PatternBase::Class(c) => Pattern::Class(c),
                        PatternBase::Features(f) => Pattern::Features(f),
                    };
                    Ok((pat, tok.span))
                }
            }
            TokenKind::LBracket => {
                let (entries, span) = self.parse_feature_entries(false)?;
                let pat = entries_to_pattern(entries);
                Ok((pat, span))
            }
            TokenKind::Eof => Err(DslError::new(
                DslErrorKind::UnexpectedEof {
                    expected: "pattern or feature block '[...]'".to_owned(),
                },
                tok.span,
            )),
            other => Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: "pattern or feature block '[...]'".to_owned(),
                    found: other.to_string(),
                },
                tok.span,
            )),
        }
    }

    fn parse_segment_mutation(
        &mut self,
        allow_newlines: bool,
    ) -> Result<(SegmentMutation, Span), DslError> {
        let (entries, span) = self.parse_feature_entries(allow_newlines)?;
        let mut features = FeatureMutation::new();
        let mut stress = StressUpdate::Keep;
        let mut vars = Vec::new();
        for entry in entries {
            match (entry.field, entry.value) {
                (FeatureField::Manner, FeatureValue::Manner(m)) => features.manner = Some(m),
                (FeatureField::Place, FeatureValue::Place(Some(p))) => {
                    features.place = FieldUpdate::Set(p);
                }
                (FeatureField::Place, FeatureValue::Place(None)) => {
                    features.place = FieldUpdate::Clear;
                }
                (FeatureField::Height, FeatureValue::Height(Some(h))) => {
                    features.height = FieldUpdate::Set(h);
                }
                (FeatureField::Height, FeatureValue::Height(None)) => {
                    features.height = FieldUpdate::Clear;
                }
                (FeatureField::Backness, FeatureValue::Backness(Some(b))) => {
                    features.backness = FieldUpdate::Set(b);
                }
                (FeatureField::Backness, FeatureValue::Backness(None)) => {
                    features.backness = FieldUpdate::Clear;
                }
                (FeatureField::Voiced, FeatureValue::Bool(v)) => features.voiced = Some(v),
                (FeatureField::Aspirated, FeatureValue::Bool(v)) => features.aspirated = Some(v),
                (FeatureField::Palatalized, FeatureValue::Bool(v)) => {
                    features.palatalized = Some(v);
                }
                (FeatureField::Labialized, FeatureValue::Bool(v)) => {
                    features.labialized = Some(v);
                }
                (FeatureField::Rounded, FeatureValue::Bool(v)) => features.rounded = Some(v),
                (FeatureField::Syllabic, FeatureValue::Bool(v)) => features.syllabic = Some(v),
                (FeatureField::Long, FeatureValue::Bool(v)) => features.long = Some(v),
                (FeatureField::Stress, FeatureValue::Stress(ps)) => {
                    stress = StressUpdate::Set(ps.to_stress());
                }
                (field, FeatureValue::Var { name, polarity }) => vars.push(FeatureVar {
                    field,
                    name,
                    polarity,
                }),
                _ => {}
            }
        }
        Ok((
            SegmentMutation {
                features,
                stress,
                vars,
            },
            span,
        ))
    }

    fn parse_feature_entries(
        &mut self,
        allow_newlines: bool,
    ) -> Result<(Vec<FeatureEntry>, Span), DslError> {
        let open_span = self.expect_token(&TokenKind::LBracket, "'['")?;
        let mut entries = Vec::new();
        let mut seen_fields = BTreeSet::new();

        loop {
            if allow_newlines {
                self.skip_newlines();
            }
            if matches!(self.peek_kind(), TokenKind::RBracket) {
                break;
            }

            let entry = self.parse_single_feature()?;
            if !seen_fields.insert(entry.field.name()) {
                return Err(DslError::new(
                    DslErrorKind::DuplicateFeature(entry.field.name().to_owned()),
                    entry.span,
                ));
            }
            entries.push(entry);

            if allow_newlines {
                self.skip_newlines();
            }
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        if allow_newlines {
            self.skip_newlines();
        }
        let close_span = self.expect_token(&TokenKind::RBracket, "']'")?;
        Ok((entries, open_span.merge(close_span)))
    }

    fn parse_single_feature(&mut self) -> Result<FeatureEntry, DslError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Plus => {
                self.advance();
                let (name, name_span) = self.expect_ident("feature name")?;
                let span = tok.span.merge(name_span);
                if name == "stressed" {
                    return Ok(FeatureEntry::new(
                        FeatureField::Stress,
                        FeatureValue::Stress(ParsedStress::StressedFlag),
                        span,
                    ));
                }
                if let Some(field) = parse_bool_field(&name) {
                    return Ok(FeatureEntry::new(field, FeatureValue::Bool(true), span));
                }
                if let Some(variant) = parse_enum_variant(&name) {
                    return Ok(FeatureEntry::new(
                        variant.field(),
                        variant.into_value(),
                        span,
                    ));
                }
                if let Some((var_name, field)) = parse_prefixed_greek_var(&name) {
                    return Ok(FeatureEntry::new(
                        field,
                        FeatureValue::Var {
                            name: var_name,
                            polarity: VarPolarity::Same,
                        },
                        span,
                    ));
                }
                Err(DslError::new(DslErrorKind::UnknownFeature(name), name_span))
            }
            TokenKind::Minus => {
                self.advance();
                let (name, name_span) = self.expect_ident("feature name")?;
                let span = tok.span.merge(name_span);
                if name == "stressed" {
                    return Ok(FeatureEntry::new(
                        FeatureField::Stress,
                        FeatureValue::Stress(ParsedStress::Exact(Stress::Unstressed)),
                        span,
                    ));
                }
                if let Some(field) = parse_bool_field(&name) {
                    return Ok(FeatureEntry::new(field, FeatureValue::Bool(false), span));
                }
                match name.as_str() {
                    "place" => Ok(FeatureEntry::new(
                        FeatureField::Place,
                        FeatureValue::Place(None),
                        span,
                    )),
                    "height" => Ok(FeatureEntry::new(
                        FeatureField::Height,
                        FeatureValue::Height(None),
                        span,
                    )),
                    "backness" => Ok(FeatureEntry::new(
                        FeatureField::Backness,
                        FeatureValue::Backness(None),
                        span,
                    )),
                    "manner" => Err(DslError::new(
                        DslErrorKind::InvalidFeatureValue {
                            field: "manner".to_owned(),
                            value: "none".to_owned(),
                        },
                        span,
                    )),
                    "stress" => Err(DslError::new(
                        DslErrorKind::InvalidFeatureValue {
                            field: "stress".to_owned(),
                            value: "-stress".to_owned(),
                        },
                        span,
                    )),
                    _ => {
                        if let Some((var_name, field)) = parse_prefixed_greek_var(&name) {
                            if field.is_bool() {
                                return Ok(FeatureEntry::new(
                                    field,
                                    FeatureValue::Var {
                                        name: var_name,
                                        polarity: VarPolarity::Opposite,
                                    },
                                    span,
                                ));
                            }
                            return Err(DslError::new(
                                DslErrorKind::InvalidFeatureValue {
                                    field: field.name().to_owned(),
                                    value: format!("-{var_name}"),
                                },
                                span,
                            ));
                        }
                        if let Some(variant) = parse_enum_variant(&name) {
                            return Err(DslError::new(
                                DslErrorKind::InvalidFeatureValue {
                                    field: variant.field().name().to_owned(),
                                    value: format!("-{name}"),
                                },
                                span,
                            ));
                        }
                        Err(DslError::new(DslErrorKind::UnknownFeature(name), name_span))
                    }
                }
            }
            TokenKind::Ident(name) => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::Equals) {
                    self.advance();
                    if matches!(self.peek_kind(), TokenKind::Minus) {
                        self.advance();
                        let (val, val_span) = self.expect_ident("feature variable")?;
                        let full_span = tok.span.merge(val_span);
                        let Some(field) = parse_field_name(&name) else {
                            return Err(DslError::new(
                                DslErrorKind::UnknownFeature(name),
                                tok.span,
                            ));
                        };
                        if let Some(var_name) = parse_explicit_var_name(&val)
                            && field.is_bool()
                        {
                            return Ok(FeatureEntry::new(
                                field,
                                FeatureValue::Var {
                                    name: var_name,
                                    polarity: VarPolarity::Opposite,
                                },
                                full_span,
                            ));
                        }
                        return Err(DslError::new(
                            DslErrorKind::InvalidFeatureValue {
                                field: name,
                                value: format!("-{val}"),
                            },
                            full_span,
                        ));
                    }
                    let (val, val_span) = self.expect_ident("feature value")?;
                    let span = tok.span.merge(val_span);
                    if let Some(field) = parse_field_name(&name)
                        && let Some(var_name) = parse_explicit_var_name(&val)
                    {
                        return Ok(FeatureEntry::new(
                            field,
                            FeatureValue::Var {
                                name: var_name,
                                polarity: VarPolarity::Same,
                            },
                            span,
                        ));
                    }
                    return parse_assigned_feature(&name, &val, tok.span, span);
                }
                if let Some(variant) = parse_enum_variant(&name) {
                    return Ok(FeatureEntry::new(
                        variant.field(),
                        variant.into_value(),
                        tok.span,
                    ));
                }
                if let Some((var_name, field)) = parse_prefixed_greek_var(&name) {
                    return Ok(FeatureEntry::new(
                        field,
                        FeatureValue::Var {
                            name: var_name,
                            polarity: VarPolarity::Same,
                        },
                        tok.span,
                    ));
                }
                if parse_bool_field(&name).is_some()
                    || matches!(
                        name.as_str(),
                        "manner" | "place" | "height" | "backness" | "stress" | "stressed"
                    )
                {
                    return Err(DslError::new(
                        DslErrorKind::UnexpectedToken {
                            expected: "'+'/'-' prefix or '=value'".to_owned(),
                            found: format!("{name:?}"),
                        },
                        tok.span,
                    ));
                }
                Err(DslError::new(DslErrorKind::UnknownFeature(name), tok.span))
            }
            TokenKind::Eof => Err(DslError::new(
                DslErrorKind::UnexpectedEof {
                    expected: "feature specification".to_owned(),
                },
                tok.span,
            )),
            other => Err(DslError::new(
                DslErrorKind::UnexpectedToken {
                    expected: "feature specification".to_owned(),
                    found: other.to_string(),
                },
                tok.span,
            )),
        }
    }
}

fn entries_to_pattern(entries: Vec<FeatureEntry>) -> Pattern {
    let mut fp = FeaturePattern::new();
    let mut stress = StressMatch::Any;
    let mut vars = Vec::new();
    for entry in entries {
        match (entry.field, entry.value) {
            (FeatureField::Manner, FeatureValue::Manner(m)) => fp.manner = Some(m),
            (FeatureField::Place, FeatureValue::Place(Some(p))) => {
                fp.place = FieldMatch::Is(p);
            }
            (FeatureField::Place, FeatureValue::Place(None)) => {
                fp.place = FieldMatch::IsNone;
            }
            (FeatureField::Height, FeatureValue::Height(Some(h))) => {
                fp.height = FieldMatch::Is(h);
            }
            (FeatureField::Height, FeatureValue::Height(None)) => {
                fp.height = FieldMatch::IsNone;
            }
            (FeatureField::Backness, FeatureValue::Backness(Some(b))) => {
                fp.backness = FieldMatch::Is(b);
            }
            (FeatureField::Backness, FeatureValue::Backness(None)) => {
                fp.backness = FieldMatch::IsNone;
            }
            (FeatureField::Voiced, FeatureValue::Bool(v)) => fp.voiced = Some(v),
            (FeatureField::Aspirated, FeatureValue::Bool(v)) => fp.aspirated = Some(v),
            (FeatureField::Palatalized, FeatureValue::Bool(v)) => fp.palatalized = Some(v),
            (FeatureField::Labialized, FeatureValue::Bool(v)) => fp.labialized = Some(v),
            (FeatureField::Rounded, FeatureValue::Bool(v)) => fp.rounded = Some(v),
            (FeatureField::Syllabic, FeatureValue::Bool(v)) => fp.syllabic = Some(v),
            (FeatureField::Long, FeatureValue::Bool(v)) => fp.long = Some(v),
            (FeatureField::Stress, FeatureValue::Stress(ps)) => stress = ps.to_match(),
            (field, FeatureValue::Var { name, polarity }) => vars.push(FeatureVar {
                field,
                name,
                polarity,
            }),
            _ => {}
        }
    }
    if stress == StressMatch::Any && vars.is_empty() {
        Pattern::Features(fp)
    } else {
        Pattern::Constrained(ConstrainedPattern {
            base: PatternBase::Features(fp),
            stress,
            vars,
        })
    }
}

fn entries_to_non_feature_constraints(
    entries: Vec<FeatureEntry>,
) -> Result<(StressMatch, Vec<FeatureVar>), DslError> {
    let mut stress = StressMatch::Any;
    let mut vars = Vec::new();
    for entry in entries {
        match (entry.field, entry.value) {
            (FeatureField::Stress, FeatureValue::Stress(ps)) => stress = ps.to_match(),
            (field, FeatureValue::Var { name, polarity }) => vars.push(FeatureVar {
                field,
                name,
                polarity,
            }),
            _ => {
                return Err(DslError::new(
                    DslErrorKind::FeaturesOnNonFeaturePattern,
                    entry.span,
                ));
            }
        }
    }
    Ok((stress, vars))
}

fn entries_to_literal_stress(entries: Vec<FeatureEntry>) -> Result<LiteralStress, DslError> {
    let mut stress = LiteralStress::Inherit;
    for entry in entries {
        match (entry.field, entry.value) {
            (FeatureField::Stress, FeatureValue::Stress(ps)) => {
                stress = LiteralStress::Set(ps.to_stress());
            }
            (FeatureField::Stress, FeatureValue::Var { name, .. }) => {
                stress = LiteralStress::Var(name);
            }
            _ => {
                return Err(DslError::new(
                    DslErrorKind::FeaturesOnLiteralReplacement,
                    entry.span,
                ));
            }
        }
    }
    Ok(stress)
}

fn parse_assigned_feature(
    field_name: &str,
    value_name: &str,
    field_span: Span,
    full_span: Span,
) -> Result<FeatureEntry, DslError> {
    match field_name {
        "manner" => match parse_manner(value_name) {
            Some(m) => Ok(FeatureEntry::new(
                FeatureField::Manner,
                FeatureValue::Manner(m),
                full_span,
            )),
            None => Err(DslError::new(
                DslErrorKind::InvalidFeatureValue {
                    field: "manner".to_owned(),
                    value: value_name.to_owned(),
                },
                full_span,
            )),
        },
        "place" => {
            if value_name == "none" {
                Ok(FeatureEntry::new(
                    FeatureField::Place,
                    FeatureValue::Place(None),
                    full_span,
                ))
            } else if let Some(p) = parse_place(value_name) {
                Ok(FeatureEntry::new(
                    FeatureField::Place,
                    FeatureValue::Place(Some(p)),
                    full_span,
                ))
            } else {
                Err(DslError::new(
                    DslErrorKind::InvalidFeatureValue {
                        field: "place".to_owned(),
                        value: value_name.to_owned(),
                    },
                    full_span,
                ))
            }
        }
        "height" => {
            if value_name == "none" {
                Ok(FeatureEntry::new(
                    FeatureField::Height,
                    FeatureValue::Height(None),
                    full_span,
                ))
            } else if let Some(h) = parse_height(value_name) {
                Ok(FeatureEntry::new(
                    FeatureField::Height,
                    FeatureValue::Height(Some(h)),
                    full_span,
                ))
            } else {
                Err(DslError::new(
                    DslErrorKind::InvalidFeatureValue {
                        field: "height".to_owned(),
                        value: value_name.to_owned(),
                    },
                    full_span,
                ))
            }
        }
        "backness" => {
            if value_name == "none" {
                Ok(FeatureEntry::new(
                    FeatureField::Backness,
                    FeatureValue::Backness(None),
                    full_span,
                ))
            } else if let Some(b) = parse_backness(value_name) {
                Ok(FeatureEntry::new(
                    FeatureField::Backness,
                    FeatureValue::Backness(Some(b)),
                    full_span,
                ))
            } else {
                Err(DslError::new(
                    DslErrorKind::InvalidFeatureValue {
                        field: "backness".to_owned(),
                        value: value_name.to_owned(),
                    },
                    full_span,
                ))
            }
        }
        "stress" => match value_name {
            "unstressed" => Ok(FeatureEntry::new(
                FeatureField::Stress,
                FeatureValue::Stress(ParsedStress::Exact(Stress::Unstressed)),
                full_span,
            )),
            "primary" => Ok(FeatureEntry::new(
                FeatureField::Stress,
                FeatureValue::Stress(ParsedStress::Exact(Stress::Primary)),
                full_span,
            )),
            "secondary" => Ok(FeatureEntry::new(
                FeatureField::Stress,
                FeatureValue::Stress(ParsedStress::Exact(Stress::Secondary)),
                full_span,
            )),
            _ => Err(DslError::new(
                DslErrorKind::InvalidFeatureValue {
                    field: "stress".to_owned(),
                    value: value_name.to_owned(),
                },
                full_span,
            )),
        },
        other => {
            if let Some(field) = parse_bool_field(other) {
                return match value_name {
                    "true" => Ok(FeatureEntry::new(
                        field,
                        FeatureValue::Bool(true),
                        full_span,
                    )),
                    "false" => Ok(FeatureEntry::new(
                        field,
                        FeatureValue::Bool(false),
                        full_span,
                    )),
                    _ => Err(DslError::new(
                        DslErrorKind::InvalidFeatureValue {
                            field: other.to_owned(),
                            value: value_name.to_owned(),
                        },
                        full_span,
                    )),
                };
            }
            Err(DslError::new(
                DslErrorKind::UnknownFeature(other.to_owned()),
                field_span,
            ))
        }
    }
}

/// Header spans of the `stage` declarations, used to position [`DialectError`]s.
#[derive(Default)]
struct StageSpans {
    /// Header span of the first declaration of each stage id.
    first: BTreeMap<String, Span>,
    /// Header span of the first repeated declaration of each duplicated stage id.
    repeated: BTreeMap<String, Span>,
}

fn dialect_error_span(err: &DialectError, spans: &StageSpans, fallback: Span) -> Span {
    let stage_spans = &spans.first;
    match err {
        DialectError::DuplicateStageId(id) => spans
            .repeated
            .get(id)
            .or_else(|| stage_spans.get(id))
            .copied()
            .unwrap_or(fallback),
        DialectError::UnknownParent { stage_id, .. }
        | DialectError::DuplicateRuleId { stage_id, .. } => {
            stage_spans.get(stage_id).copied().unwrap_or(fallback)
        }
        DialectError::MultipleRoots(roots) => roots
            .first()
            .and_then(|id| stage_spans.get(id))
            .copied()
            .unwrap_or(fallback),
        DialectError::CycleDetected(cycle) => cycle
            .first()
            .and_then(|id| stage_spans.get(id))
            .copied()
            .unwrap_or(fallback),
        DialectError::EmptyTree | DialectError::EmptyStageId | DialectError::MissingRoot => {
            fallback
        }
    }
}

const EOF_TOKEN: Token = Token {
    kind: TokenKind::Eof,
    span: Span::new(0, 0, 1, 1),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{RuleError, derive};
    use crate::phonology::{InventoryError, Stress};

    #[test]
    fn parses_segments_classes_and_n_to_m_rules() -> Result<(), Box<dyn std::error::Error>> {
        let dsl = r#"
            // Inventory declarations (multiline + assigned + short forms)
            segment p = [stop, bilabial, -voiced]
            segment b = [manner=stop, +bilabial, +voiced]
            segment t = [stop, alveolar, -voiced]
            segment d = [stop, alveolar, +voiced]
            segment r = [rhotic, alveolar, +voiced]
            segment s = [fricative, alveolar, -voiced]
            segment a = [
                vowel,
                height=low,
                backness=central,
            ]
            segment aː = [vowel, low, central, +long]
            segment e = [vowel, mid-high, front]
            segment i = [vowel, high, front]
            segment m = [nasal, bilabial, +voiced]

            class V = {
                a, aː, e, i,
            }
            class C = { p, b, t, d, r, s, m }

            rule R1 "Final -m deletion":
              m > ∅ / _ #

            rule R2 "Intervocalic voicing" (simultaneous):
              [-voiced, +stop] > [+voiced] / V _ V

            rule R3 "Metathesis" (left-to-right):
              r t > $2 $1

            rule R4 "Compensatory lengthening":
              V s > $1[+long] / _ C

            rule R5 "Prothesis":
              0 > e / # _ s

            rule R6 "Glide formation and lowering":
              i > $1[manner=approximant, +palatalized, height=none, -backness, -syllabic]

            rule R7 "Long non-local context":
              a > e / V* _ #
        "#;

        let program = parse_program(dsl)?;
        program.validate_against(&program.inventory)?;

        assert_eq!(program.inventory.len(), 11);
        assert_eq!(
            program.inventory.get("a"),
            Some(&FeatureBundle::vowel(Height::Low, Backness::Central, false))
        );
        assert_eq!(
            program.inventory.get("p"),
            Some(&FeatureBundle::consonant(
                Manner::Stop,
                Place::Bilabial,
                false
            ))
        );
        assert_eq!(program.classes.len(), 2);
        assert_eq!(program.rules.len(), 7);

        assert_eq!(program.rules[0].name(), "Final -m deletion");
        assert!(program.rules[0].replacement().is_empty());
        assert_eq!(program.rules[2].mode(), ApplicationMode::LeftToRight);
        assert_eq!(
            program.rules[2].replacement(),
            &[
                ReplacementItem::Mutate {
                    source_index: 1,
                    mutation: FeatureMutation::new().into(),
                },
                ReplacementItem::Mutate {
                    source_index: 0,
                    mutation: FeatureMutation::new().into(),
                },
            ]
        );
        assert_eq!(
            program.rules[3].replacement(),
            &[ReplacementItem::Mutate {
                source_index: 0,
                mutation: FeatureMutation::new().set_long(true).into(),
            }]
        );
        assert!(program.rules[4].target().is_empty());
        assert_eq!(
            program.rules[6].left_context(),
            &[ContextElement::ZeroOrMore(Pattern::Class(
                program.classes["V"].clone()
            ))]
        );
        Ok(())
    }

    #[test]
    fn separates_space_delimited_source_ref_and_feature_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let dsl = r#"
            segment i = [vowel, high, front]
            segment a = [vowel, low, central]
            rule split:
              i > $1 [+low]
        "#;
        let err = parse_program(dsl);
        // Because `i` has target_len == 1, `$1` followed by space and `[+low]` parses as TWO items!
        let program = err?;
        assert_eq!(program.rules[0].replacement().len(), 2);

        let ambiguous = parse_program("rule bad:\n  a i > $1 [+low]\n");
        assert!(matches!(
            ambiguous,
            Err(DslError {
                kind: DslErrorKind::AmbiguousImplicitMutation { target_len: 2 },
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn preserves_stress_on_bare_source_ref_and_executes_rule()
    -> Result<(), Box<dyn std::error::Error>> {
        let dsl = r#"
            segment a = [vowel, low, central]
            segment aː = [vowel, low, central, +long]
            segment s = [fricative, alveolar, -voiced]
            segment t = [stop, alveolar, -voiced]
            class V = { a, aː }
            rule lengthen:
              V s > $1[+long] / _ t
        "#;
        let program = parse_program(dsl)?;
        let word = crate::phonology::Word::new(vec![
            program.inventory.segment("a", Stress::Primary)?,
            program.inventory.segment("s", Stress::Unstressed)?,
            program.inventory.segment("t", Stress::Unstressed)?,
        ]);
        let trace = derive(&word, &program.rules, &program.inventory)?;
        assert_eq!(trace.final_word().to_spaced(), "ˈaː t");
        assert_eq!(trace.final_word().segments()[0].stress, Stress::Primary);
        Ok(())
    }

    #[test]
    fn rejects_semantic_and_syntax_errors_with_exact_kinds() {
        assert!(matches!(
            parse_program("segment p = [+voiced]"),
            Err(DslError {
                kind: DslErrorKind::MissingManner { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("segment p = [stop, -manner]"),
            Err(DslError {
                kind: DslErrorKind::InvalidFeatureValue { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("segment p = [stop, -stop]"),
            Err(DslError {
                kind: DslErrorKind::InvalidFeatureValue { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("segment p = [stop, +voiced, -voiced]"),
            Err(DslError {
                kind: DslErrorKind::DuplicateFeature(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("segment p = [stop, bilabial]\nsegment b = [stop, bilabial]"),
            Err(DslError {
                kind: DslErrorKind::InventoryError(InventoryError::DuplicateBundle { .. }),
                ..
            })
        ));
        assert!(matches!(
            parse_program("class V = {}"),
            Err(DslError {
                kind: DslErrorKind::EmptyClass(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("class V = { a }\nclass V = { e }"),
            Err(DslError {
                kind: DslErrorKind::DuplicateClass(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("class V = { a, a }"),
            Err(DslError {
                kind: DslErrorKind::DuplicateClassMember { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("class V = { a }\nrule r1:\n  a > V"),
            Err(DslError {
                kind: DslErrorKind::ClassInReplacement(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  ∅ a > b"),
            Err(DslError {
                kind: DslErrorKind::EmptyMixedWithItems,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  a > $0"),
            Err(DslError {
                kind: DslErrorKind::InvalidSourceIndex {
                    reference: 0,
                    target_len: 1
                },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  a > $2"),
            Err(DslError {
                kind: DslErrorKind::InvalidSourceIndex {
                    reference: 2,
                    target_len: 1
                },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1 (bogus):\n  a > e"),
            Err(DslError {
                kind: DslErrorKind::UnknownApplicationMode(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  a > e / V"),
            Err(DslError {
                kind: DslErrorKind::MissingContextFocus,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  a > e / _ V _"),
            Err(DslError {
                kind: DslErrorKind::DuplicateContextFocus,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  a > e\nrule r1:\n  e > i"),
            Err(DslError {
                kind: DslErrorKind::DuplicateRuleId(_),
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  ∅ > e"),
            Err(DslError {
                kind: DslErrorKind::InvalidRule(RuleError::UnconstrainedInsertion { .. }),
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r1:\n  ∅ > e / V* _"),
            Err(DslError {
                kind: DslErrorKind::InvalidRule(RuleError::UnconstrainedInsertion { .. }),
                ..
            })
        ));
        for misplaced in ["rule r1:\n  a > e / t # _", "rule r1:\n  a > e / _ # t"] {
            assert!(matches!(
                parse_program(misplaced),
                Err(DslError {
                    kind: DslErrorKind::InvalidRule(RuleError::MisplacedWordBoundary { .. }),
                    ..
                })
            ));
        }
    }

    #[test]
    fn validate_against_reports_unknown_symbols_with_span() -> Result<(), Box<dyn std::error::Error>>
    {
        let program = parse_program("class V = { a, x }\nrule r1:\n  a > o")?;
        let mut inv = Inventory::new();
        inv.insert(
            "a",
            FeatureBundle::vowel(Height::Low, Backness::Central, false),
        )?;
        inv.insert(
            "o",
            FeatureBundle::vowel(Height::MidHigh, Backness::Back, true),
        )?;

        let err = program.validate_against(&inv);
        assert!(matches!(
            err,
            Err(DslError {
                kind: DslErrorKind::UnknownSymbol(sym),
                span: Span { line: 1, .. }
            }) if sym == "x"
        ));
        Ok(())
    }

    #[test]
    fn parses_stage_dialect_trees_and_validates_per_stage_rule_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let dsl = r#"
            segment a = [vowel, low, central]
            segment e = [vowel, mid-high, front]
            segment o = [vowel, mid-high, back, +rounded]
            segment p = [stop, bilabial, -voiced]

            stage proto "Proto-Language" {}

            stage north from proto {
              rule raise:
                a > e
            }

            // Same rule id `raise` in sister stage is allowed.
            stage south from proto {
              rule raise:
                a > o
            }
        "#;

        let program = parse_program(dsl)?;
        program.validate_against(&program.inventory)?;
        assert!(program.rules.is_empty());
        let tree = program
            .dialect_tree
            .as_ref()
            .ok_or("expected dialect tree")?;
        assert_eq!(tree.root_id(), "proto");
        assert_eq!(tree.leaves(), vec!["north", "south"]);

        // Single-line stage with rule also parses cleanly.
        let one_line = parse_program("stage root { rule r1: a > e }")?;
        assert!(one_line.dialect_tree.is_some());

        // `validate_against` checks symbols inside stage rules too.
        let unknown_in_stage = parse_program("stage root {\n  rule r1:\n    a > z\n}")?;
        assert!(matches!(
            unknown_in_stage.validate_against(&program.inventory),
            Err(DslError {
                kind: DslErrorKind::UnknownSymbol(sym),
                span: Span { line: 3, .. }
            }) if sym == "z"
        ));

        // Duplicate rule id within the same stage is rejected.
        assert!(matches!(
            parse_program("stage root {\n  rule r1:\n    a > e\n  rule r1:\n    e > o\n}"),
            Err(DslError {
                kind: DslErrorKind::DuplicateRuleId(_),
                ..
            })
        ));

        // Mixing top-level rules and stages in either order is rejected.
        assert!(matches!(
            parse_program("rule r1:\n  a > e\nstage root {}"),
            Err(DslError {
                kind: DslErrorKind::MixedTopLevelRulesAndStages,
                ..
            })
        ));
        assert!(matches!(
            parse_program("stage root {}\nrule r1:\n  a > e"),
            Err(DslError {
                kind: DslErrorKind::MixedTopLevelRulesAndStages,
                ..
            })
        ));

        // Invalid dialect tree invariants surface as `InvalidDialectTree`.
        assert!(matches!(
            parse_program("stage child from missing {}"),
            Err(DslError {
                kind: DslErrorKind::InvalidDialectTree(DialectError::UnknownParent { .. }),
                ..
            })
        ));
        assert!(matches!(
            parse_program("stage a {}\nstage b {}"),
            Err(DslError {
                kind: DslErrorKind::InvalidDialectTree(DialectError::MultipleRoots(_)),
                ..
            })
        ));
        assert!(matches!(
            parse_program("stage root {}\nstage a from b {}\nstage b from a {}"),
            Err(DslError {
                kind: DslErrorKind::InvalidDialectTree(DialectError::CycleDetected(_)),
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn programs_with_segments_are_validated_automatically() {
        // A typo in a literal pattern no longer yields a rule that silently never fires.
        assert!(matches!(
            parse_program("segment a = [vowel]\nsegment t = [stop]\nrule r:\n  x > a / t _"),
            Err(DslError {
                kind: DslErrorKind::UnknownSymbol(sym),
                span: Span { line: 4, column: 3, .. }
            }) if sym == "x"
        ));

        // A class used before its declaration is reported as such.
        assert!(matches!(
            parse_program(
                "segment a = [vowel]\nsegment t = [stop]\nsegment p = [stop, bilabial]\n\
                 rule r:\n  t > p / W _ W\nclass W = { a }"
            ),
            Err(DslError {
                kind: DslErrorKind::ClassUsedBeforeDeclaration(name),
                span: Span { line: 5, column: 11, .. }
            }) if name == "W"
        ));

        // Class members are symbols, never class references.
        assert!(matches!(
            parse_program("segment a = [vowel]\nclass V = { a }\nclass W = { V }"),
            Err(DslError {
                kind: DslErrorKind::UnknownSymbol(sym),
                ..
            }) if sym == "V"
        ));

        // Without `segment` declarations the inventory is external: no automatic validation.
        assert!(parse_program("rule r:\n  x > a").is_ok());
    }

    #[test]
    fn diagnostics_point_at_the_offending_declaration() {
        // `V_V` is a single identifier: the error suggests separating the focus.
        assert!(matches!(
            parse_program("rule r:\n  t > p / V_V"),
            Err(DslError {
                kind: DslErrorKind::FocusInsideIdentifier(ident),
                span: Span { line: 2, column: 11, .. }
            }) if ident == "V_V"
        ));
        // Identifiers with `_` are fine when a standalone focus is present.
        assert!(parse_program("class final_v = { a }\nrule r:\n  t > p / final_v _").is_ok());

        // `DuplicateStageId` points at the repeated declaration, not the first one.
        assert!(matches!(
            parse_program("stage a {}\nstage b from a {}\nstage a from b {}"),
            Err(DslError {
                kind: DslErrorKind::InvalidDialectTree(DialectError::DuplicateStageId(id)),
                span: Span { line: 3, column: 1, .. }
            }) if id == "a"
        ));

        // A leading UTF-8 BOM is ignored.
        assert!(parse_program("\u{FEFF}segment a = [vowel]").is_ok());
    }

    #[test]
    fn parses_m5_stress_variables_multi_context_and_optional()
    -> Result<(), Box<dyn std::error::Error>> {
        let dsl = r#"
            segment p = [stop, bilabial, -voiced]
            segment t = [stop, alveolar, -voiced]
            segment k = [stop, velar, -voiced]
            segment d = [stop, alveolar, +voiced]
            segment m = [nasal, bilabial, +voiced]
            segment n = [nasal, alveolar, +voiced]
            segment ŋ = [nasal, velar, +voiced]
            segment r = [rhotic, alveolar, +voiced]
            segment j = [approximant, palatal, +voiced]
            segment a = [vowel, low, central]
            segment e = [vowel, mid-high, front]
            segment ɛ = [vowel, mid-low, front]

            class V = { a, e, ɛ }
            class C = { p, t, k, d, m, n, ŋ, r, j }

            rule diph:
              ɛ[+stressed] > j e[+stressed]

            rule assim:
              [nasal] > [αplace] / _ [stop, αplace]

            rule copy_stress_at:
              e > a[stress=@s] / _ [vowel, stress=@s]

            rule syncope:
              V[-stressed] > ∅ / C (r) _ C V |
                               V C _ C #

            rule leading_pipe:
              a > e / p _ #
                    | k _ #
        "#;
        let program = parse_program(dsl)?;
        program.validate_against(&program.inventory)?;

        assert_eq!(program.rules[0].target().len(), 1);
        assert_eq!(
            program.rules[0].replacement(),
            &[
                ReplacementItem::Literal("j".to_owned()),
                ReplacementItem::LiteralWithStress {
                    symbol: "e".to_owned(),
                    stress: LiteralStress::Set(Stress::Primary),
                },
            ]
        );
        assert_eq!(program.rules[3].environments().len(), 2);
        assert_eq!(program.rules[4].environments().len(), 2);

        // Space-delimited literal + feature block remains two separate target/replacement items.
        let spaced_target = parse_program("rule split_t:\n  a [+stressed] > a e\n")?;
        assert_eq!(spaced_target.rules[0].target().len(), 2);
        assert_eq!(
            spaced_target.rules[0].target()[0],
            Pattern::Literal("a".to_owned())
        );

        let spaced_repl = parse_program("rule split_r:\n  a > e [+stressed]\n")?;
        assert_eq!(spaced_repl.rules[0].replacement().len(), 2);
        assert_eq!(
            spaced_repl.rules[0].replacement()[0],
            ReplacementItem::Literal("e".to_owned())
        );

        // Multi-context and optional inside `stage` on one line (`allow_rbrace_end`).
        let staged = parse_program("stage s { rule r: a > e / (p) _ # | _ k }")?;
        let tree = staged.dialect_tree.as_ref().ok_or("expected tree")?;
        let node = tree.node("s").ok_or("expected stage s")?;
        assert_eq!(node.rules()[0].environments().len(), 2);

        // Positive and negated Greek/`@` variable forms parse symmetrically.
        let vars_ok = parse_program(
            "rule v1:\n  [+stop, +αvoiced] > [voiced=-α]\n\
             rule v2:\n  [+stop, place=@p, voiced=-@v] > [place=@p, voiced=@v]\n\
             rule v3:\n  [+stop, stress=secondary] > $1[stress=primary]\n\
             rule v4:\n  a > e[αstress] / _ [vowel, αstress]\n",
        )?;
        assert_eq!(vars_ok.rules.len(), 4);
        Ok(())
    }

    #[test]
    fn rejects_m5_dsl_and_semantic_errors_with_exact_kinds() {
        assert!(matches!(
            parse_program("segment a = [vowel, +stressed]"),
            Err(DslError {
                kind: DslErrorKind::StressInSegmentDecl,
                ..
            })
        ));
        assert!(matches!(
            parse_program("segment p = [stop, αplace]"),
            Err(DslError {
                kind: DslErrorKind::VariableInSegmentDecl(v),
                ..
            }) if v == "α"
        ));
        assert!(matches!(
            parse_program("segment p = [stop, place=@p]"),
            Err(DslError {
                kind: DslErrorKind::VariableInSegmentDecl(v),
                ..
            }) if v == "@p"
        ));
        assert!(matches!(
            parse_program("rule r:\n  a[+voiced] > e"),
            Err(DslError {
                kind: DslErrorKind::FeaturesOnNonFeaturePattern,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e[+long]"),
            Err(DslError {
                kind: DslErrorKind::FeaturesOnLiteralReplacement,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  [stop, -αplace] > a"),
            Err(DslError {
                kind: DslErrorKind::InvalidFeatureValue { field, .. },
                ..
            }) if field == "place"
        ));
        assert!(matches!(
            parse_program("rule r:\n  [stop, place=-@p] > a"),
            Err(DslError {
                kind: DslErrorKind::InvalidFeatureValue { field, .. },
                ..
            }) if field == "place"
        ));
        assert!(matches!(
            parse_program("rule r:\n  [+stressed, -stressed] > a"),
            Err(DslError {
                kind: DslErrorKind::DuplicateFeature(f),
                ..
            }) if f == "stress"
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e / (#) _"),
            Err(DslError {
                kind: DslErrorKind::UnexpectedToken { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e / () _"),
            Err(DslError {
                kind: DslErrorKind::UnexpectedToken { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e / (p t) _"),
            Err(DslError {
                kind: DslErrorKind::UnexpectedToken { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e / (p)* _"),
            Err(DslError {
                kind: DslErrorKind::UnexpectedToken { .. },
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  a > e / p _ | k"),
            Err(DslError {
                kind: DslErrorKind::MissingContextFocus,
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  ∅ > e / (p) _"),
            Err(DslError {
                kind: DslErrorKind::InvalidRule(RuleError::UnconstrainedInsertion { .. }),
                ..
            })
        ));
        assert!(matches!(
            parse_program("rule r:\n  [nasal] > [αplace] / _ ([stop, αplace])"),
            Err(DslError {
                kind: DslErrorKind::InvalidRule(RuleError::UnboundVariable { .. }),
                ..
            })
        ));

        // Automatic validate_against catches unknown symbols in M5 constructs too.
        for bad in [
            "segment a = [vowel]\nrule r:\n  x[+stressed] > a",
            "segment a = [vowel]\nrule r:\n  a > x[+stressed]",
            "segment a = [vowel]\nrule r:\n  a > a / (x) _",
            "segment a = [vowel]\nrule r:\n  a > a / a _ | x _",
        ] {
            assert!(matches!(
                parse_program(bad),
                Err(DslError {
                    kind: DslErrorKind::UnknownSymbol(sym),
                    ..
                }) if sym == "x"
            ));
        }
    }
}
