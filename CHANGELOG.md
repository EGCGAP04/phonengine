# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Milestone 1 phonological model (`src/phonology/`): `Manner`, `Place`, `Height`, `Backness`, `Stress`, `FeatureBundle`, `FieldMatch`, `FieldUpdate`, `FeaturePattern`, `FeatureMutation`, `Segment`, `Word`, and bijective `Inventory` backed by `BTreeMap` with `InventoryError`.
- Milestone 1 rule engine (`src/engine/`): `Rule` validation (`RuleError::EmptyId`, `RuleError::UnconstrainedInsertion`, `RuleError::InvalidSourceIndex`), `Pattern` (literal, feature pattern, `NaturalClass`), `ReplacementItem`, `ContextElement`, `ApplicationMode::Simultaneous`, `apply_rule`, `derive`, `DerivationTrace` omitting vacuous steps, and `EngineError`.
- Public crate API in `src/lib.rs`, unit tests across all public modules, and `tests/golden_mvp.rs` validating `tests/fixtures/golden_mvp.tsv`.
- GitHub Actions CI workflow (`Build and test`) running `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
- Dependabot configuration for Cargo crates and GitHub Actions.
- GitHub issue forms (`feature`, `bug`, `chore`, `docs`, `refactor`) and pull request template.
- MIT `LICENSE`, `.editorconfig`, `.gitattributes`, `SECURITY.md`, and `README.md`.
