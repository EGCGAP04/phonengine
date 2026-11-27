# PhonEngine

Deterministic phonological evolution and comparative reconstruction engine written in Rust. Models how a proto-language evolves step by step through ordered sound laws into descendant dialects.

## Design principles

- **Segment-based representation**: words are modeled as `Vec<Segment>` (separating intrinsic phonological features from prosodic stress), never manipulated as raw text.
- **General N -> M rules**: supports deletion (`1 -> 0`), epenthesis (`0 -> 1`), diphthongization (`1 -> 2`), compensatory lengthening (`2 -> 1`), and metathesis (`2 -> 2`).
- **Noise-free traceability**: every derivation produces an immutable `DerivationTrace` recording only steps where the word actually changed.
- **Strict determinism**: core structures and serialization rely exclusively on `BTreeMap`, `BTreeSet`, and `Vec` (never `HashMap` or `HashSet`).
- **`std`-only core**: `phonology`, `engine`, and `dsl` depend solely on the Rust standard library. External crates are restricted to `clap` in the CLI (`src/cli/`).

See [`SPEC.md`](SPEC.md) for the full technical specification.

## Commands

Requires Rust 1.88 or later (the `rust-version` declared in `Cargo.toml`).

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### CLI usage

```bash
# Derive words through flat rules or a target dialect stage
cargo run -- run rules.phon vitam fatam
cargo run -- run greek.phon --stage attic -f json -w words.txt

# Trace every non-vacuous step (supports --spaced and Markdown export)
cargo run -- trace rules.phon --spaced -f markdown "v i t a m"

# Compare outcomes across leaf dialects or explicit stages
cargo run -- compare greek.phon --stages attic,ionic pʰˈaːtʰi
```

## Project structure

```text
.
├── .editorconfig
├── .gitattributes
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug.yml
│   │   ├── chore.yml
│   │   ├── config.yml
│   │   ├── docs.yml
│   │   ├── feature.yml
│   │   └── refactor.yml
│   ├── dependabot.yml
│   ├── pull_request_template.md
│   └── workflows/
│       └── ci.yml
├── .gitignore
├── AGENTS.md
├── CHANGELOG.md
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── SECURITY.md
├── SPEC.md
├── clippy.toml
├── src/
│   ├── cli/
│   │   ├── args.rs
│   │   ├── error.rs
│   │   ├── format.rs
│   │   └── mod.rs
│   ├── dsl/
│   │   ├── error.rs
│   │   ├── lexer.rs
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   └── tokenizer.rs
│   ├── engine/
│   │   ├── applier.rs
│   │   ├── dialect.rs
│   │   ├── error.rs
│   │   ├── mod.rs
│   │   ├── rule.rs
│   │   └── trace.rs
│   ├── lib.rs
│   ├── main.rs
│   └── phonology/
│       ├── features.rs
│       ├── inventory.rs
│       ├── mod.rs
│       ├── pattern.rs
│       └── segment.rs
└── tests/
    ├── cli.rs
    ├── dialect_tree.rs
    ├── dsl_mvp.rs
    ├── fixtures/
    │   └── golden_mvp.tsv
    └── golden_mvp.rs
```

## Roadmap

- [x] **M0**: Repository setup (CI, templates, license, and base documentation).
- [x] **M1**: Core phonology (`src/phonology/`) and rule engine (`src/engine/`) with golden MVP tests.
- [x] **M2**: DSL lexer/parser (`src/dsl/`) and maximal-munch tokenizer.
- [x] **M3**: Dialect trees, chronological stages, directional modes, and non-local contexts.
- [x] **M4**: CLI (`src/cli/`) with `clap` and Markdown/JSON export.
- [ ] **M5**: Phonological expressiveness (stress-aware rules, alpha feature variables, multi-context/optional environments, and historical grammar examples).
- [ ] **M6**: Comparative reconstruction (feature-based cognate alignment, sound correspondence tables, and proto-form verification).

## License

[MIT](LICENSE)
