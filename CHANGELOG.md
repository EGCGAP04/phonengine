# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-27

Second release of PhonEngine covering Milestone 5 (`M5: Phonological expressiveness`): prosodic stress constraints and mutations, alpha feature assimilation/dissimilation variables, multi-context/optional environments, and real-world historical grammar examples.

### Added

- Milestone 5 specification in `SPEC.md` defining stress-aware rule patterns and mutations (`StressMatch`, `StressUpdate`, `LiteralStress`), alpha feature assimilation/dissimilation variables (`FeatureDomain`, `FeatureField`, `VarPolarity`, `FeatureVar`, `FeatureVarValue`, `VarBindings`), multi-context `|` environments, optional `(X)` context elements, and planned `examples/` historical grammars.
- Prosodic stress matching/mutations (`StressMatch`, `StressUpdate`, `LiteralStress`) and alpha feature variable types (`FeatureDomain`, `FeatureField`, `VarPolarity`, `FeatureVar`, `FeatureVarValue`, `VarBindings`) in `src/phonology/pattern.rs` and `src/lib.rs`.
- Multi-context disjunctive `Environment`s (`Rule::with_environments`), `ContextElement::Optional`, `Pattern::Constrained` / `ConstrainedPattern` / `PatternBase`, `SegmentMutation`, `ReplacementItem::LiteralWithStress`, extended `RuleError` static validation, and deterministic closest-to-focus variable-binding DP matching in `src/engine/`.
- DSL lexer, parser, and diagnostic support (`TokenKind::Pipe`, `StressInSegmentDecl`, `VariableInSegmentDecl`, `FeaturesOnNonFeaturePattern`, `FeaturesOnLiteralReplacement`) for prosodic stress specs, Greek and `@` alpha variables, postfix `[...]` modifiers, multi-context `|` branches, and optional `(<pattern>)` context elements in `src/dsl/`.
- Historical dialect-tree grammar examples (`examples/latin_to_romance.phon` and `examples/pie_to_greek.phon`) and integration tests (`tests/examples.rs`) covering golden derivations and CLI invocation across all Milestone 5 features.
- Opened GitHub milestone `M6: Comparative reconstruction` and updated `.github/ISSUE_TEMPLATE/*.yml`, `AGENTS.md`, and `README.md` for `M5` completion and `v0.2.0`.

### Changed

- Minor post-`v0.1.0` documentation wording cleanups in `README.md` and `SPEC.md`, plus rustdoc and `SPEC.md` alignment updates on `apply_rule`, `LiteralStress::Inherit`, `RuleError::UnconstrainedInsertion`, and `Program::validate_against`.
- Updated `README.md` CLI usage examples to run against the bundled `examples/*.phon` grammars, updated the Project structure tree, and marked Milestone 5 completed in `README.md` and `SPEC.md`.
- Bumped crate version to `0.2.0` in `Cargo.toml` / `Cargo.lock` and closed Milestone 5 in `AGENTS.md` and `.github/ISSUE_TEMPLATE/*.yml`.

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

[Unreleased]: https://github.com/EGCGAP04/phonengine/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/EGCGAP04/phonengine/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EGCGAP04/phonengine/releases/tag/v0.1.0
