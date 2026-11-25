# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Defined Milestone 3 specification in `SPEC.md`: $O(|word| \cdot |context|)$ reachability matching for `ZeroOrMore`, single-pass `LeftToRight` / `RightToLeft` directional modes, `DialectTree` / `DialectError` / `TreeDerivation`, removal of transitional `UnsupportedMode` / `UnsupportedContext` errors in favor of `UnknownStage`, and `stage` DSL syntax (`Program::dialect_tree`).
- Updated `AGENTS.md` to mark Milestone 3 as active (`std`-only through M3) and removed closed `M2` from `.github/ISSUE_TEMPLATE/*.yml`.
- Specified the complete Milestone 2 DSL (`segment`, `class`, `rule` with 1-based `$k` references), `Span`-based diagnostics (`DslError`), and maximal-munch word tokenizer (`tokenize_word` with IPA stress markers `ˈ`/`ˌ`) in `SPEC.md`.
- Updated `AGENTS.md` to mark Milestone 2 as active and document the repository's Git/GitHub workflow (protected `main`, SSH-signed commits, squash merge, templates, labels, milestones, and project #8).
- Removed closed `M0` and `M1` milestone options from `.github/ISSUE_TEMPLATE/*.yml`.

### Added

- Milestone 3 rule engine & DSL support:
  - $O(|word| \cdot |context|)$ deterministic DP/NFA reachability matching for `ContextElement::ZeroOrMore`.
  - Single-pass `ApplicationMode::LeftToRight` and `ApplicationMode::RightToLeft` rule execution for both $N \ge 1$ and $N = 0$ (epenthesis).
  - Chronological dialect trees (`src/engine/dialect.rs`: `DialectNode`, `DialectTree`, `DialectError`, `StageTrace`, `DialectTrace`, `TreeDerivation`), `EngineError::UnknownStage`, and public crate re-exports in `src/lib.rs`.
  - DSL `stage <id> ["<name>"] [from <parent>] { ... }` declarations, `Program::dialect_tree`, `DslErrorKind::MixedTopLevelRulesAndStages`, and `DslErrorKind::InvalidDialectTree` in `src/dsl/`, plus unit tests and `tests/dialect_tree.rs`.
- Milestone 2 DSL and maximal-munch tokenizer (`src/dsl/`): `Span`, `DslError`, `DslErrorKind`, `TokenizeError`, `TokenizeErrorKind`, Unicode-aware `lex`, manual recursive-descent `parse_program` (`segment`, `class`, `rule` with 1-based `$k` references), `Program::validate_against`, and `tokenize_word` supporting IPA stress markers (`ˈ` / `ˌ`), re-exported in `src/lib.rs` with unit tests and `tests/dsl_mvp.rs`.
- Milestone 1 phonological model (`src/phonology/`): `Manner`, `Place`, `Height`, `Backness`, `Stress`, `FeatureBundle`, `FieldMatch`, `FieldUpdate`, `FeaturePattern`, `FeatureMutation`, `Segment`, `Word`, and bijective `Inventory` backed by `BTreeMap` with `InventoryError`.
- Milestone 1 rule engine (`src/engine/`): `Rule` validation (`RuleError::EmptyId`, `RuleError::UnconstrainedInsertion`, `RuleError::InvalidSourceIndex`), `Pattern` (literal, feature pattern, `NaturalClass`), `ReplacementItem`, `ContextElement`, `ApplicationMode::Simultaneous`, `apply_rule`, `derive`, `DerivationTrace` omitting vacuous steps, and `EngineError`.
- Public crate API in `src/lib.rs`, unit tests across all public modules, and `tests/golden_mvp.rs` validating `tests/fixtures/golden_mvp.tsv`.
- GitHub Actions CI workflow (`Build and test`) running `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
- Dependabot configuration for Cargo crates and GitHub Actions.
- GitHub issue forms (`feature`, `bug`, `chore`, `docs`, `refactor`) and pull request template.
- MIT `LICENSE`, `.editorconfig`, `.gitattributes`, `SECURITY.md`, and `README.md`.

### Removed

- Transitional Milestone 1/2 errors `EngineError::UnsupportedMode` and `EngineError::UnsupportedContext`, now that `LeftToRight`, `RightToLeft`, and `ZeroOrMore` are implemented in Milestone 3.
