# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-27

First release of PhonEngine: deterministic phonological evolution and comparative reconstruction library and CLI in Rust (Milestones 0–4).

### Added

- **Repository setup and governance**: GitHub Actions CI (`Build and test`: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`), CodeQL, Dependabot (grouped Cargo and GitHub Actions updates), MSRV `1.88` in `Cargo.toml`, MIT `LICENSE`, `SECURITY.md`, `.editorconfig`, `.gitattributes`, issue forms, and pull request template.
- **Phonological model (`src/phonology/`)**:
  - `Copy`/`Eq`/`Ord` feature enums (`Manner`, `Place`, `Height`, `Backness`, `Stress` with `ipa_marker` / `from_ipa_marker`) and `FeatureBundle`.
  - Explicit feature queries and mutations (`FieldMatch`, `FieldUpdate`, `FeaturePattern`, `FeatureMutation`).
  - `Segment` (keeping `Stress` outside `FeatureBundle`) and `Word` (`from_spaced`, `to_spaced`, `to_compact`, `Display`) with reversible stress-marker formatting.
  - Strictly bijective `Inventory` backed by `BTreeMap` with `InventoryError`.
- **Rule engine and dialect trees (`src/engine/`)**:
  - Validated `Rule` (`RuleError::EmptyId`, `RuleError::UnconstrainedInsertion`, `RuleError::InvalidSourceIndex`, `RuleError::MisplacedWordBoundary`) supporting `Pattern` (literal, feature bundle, `NaturalClass`), general $N \to M$ `ReplacementItem` rewrites with positional stress inheritance, and `ContextElement` (`WordBoundary`, `Segment`, and $O(|word| \cdot |context|)$ `ZeroOrMore`).
  - Pure `apply_rule` and `derive` supporting `ApplicationMode::Simultaneous`, `LeftToRight`, and `RightToLeft`, returning immutable `DerivationTrace`s omitting vacuous steps.
  - Chronological `DialectNode` / `DialectTree` (`DialectError`, `StageTrace`, `DialectTrace`, `TreeDerivation`) with shared-ancestor reuse and `EngineError` carrying failing `rule_id` and `StageFailed` stage context.
- **DSL parser and maximal-munch tokenizer (`src/dsl/`)**:
  - Unicode-aware `lex` (with leading UTF-8 BOM handling) and manual recursive-descent `parse_program` supporting `segment`, `class`, `rule` (with 1-based `$k` references), and `stage` dialect-tree declarations.
  - Automatic symbol validation (`Program::validate_against`) with `Span`-positioned diagnostics (`DslError`, `DslErrorKind` including `ClassUsedBeforeDeclaration`, `FocusInsideIdentifier`, `MixedTopLevelRulesAndStages`, `InvalidDialectTree`).
  - Greedy maximal-munch `tokenize_word` with IPA stress markers (`ˈ`/`ˌ`) attached to the next syllabic segment and positioned `TokenizeError`.
- **CLI and export formats (`src/cli/`, `src/main.rs`)**:
  - `clap`-derived `run`, `trace`, and `compare` subcommands supporting positional words, `--words <PATH>` (stripping leading UTF-8 BOM and skipping blank/`//`/`#` lines), `stdin` rules (`-`) or words, `--spaced`, `--stage`, `--stages`, `-f <text|markdown|json>`, and `-o <PATH>`.
  - Deterministic hand-written RFC 8259 JSON formatter, GFM Markdown tables, and column-aligned plain-text comparison tables (zero `serde` dependencies).
  - `CliError` with path/stream context (`Io`, `Dsl`, `EmptyInventory`, `Tokenize`, `SpacedParse`, `Engine`, `MissingStage`, `UnexpectedStage`, `CompareRequiresDialectTree`, `DuplicateStageSelection`, `UnknownStage`, `StdinConflict`) and exit codes `0`, `1`, and `2`.
- **Test suites**: unit tests across all public modules plus external integration suites in `tests/golden_mvp.rs`, `tests/dsl_mvp.rs`, `tests/dialect_tree.rs`, and `tests/cli.rs`.

[Unreleased]: https://github.com/EGCGAP04/phonengine/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/EGCGAP04/phonengine/releases/tag/v0.1.0
