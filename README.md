# PhonEngine

Deterministic phonological evolution and comparative reconstruction engine written in Rust. Models how a proto-language evolves step by step through ordered sound laws into descendant dialects.

## Design principles

- **Segment-based representation**: words are modeled as `Vec<Segment>` (separating intrinsic phonological features from prosodic stress), never manipulated as raw text.
- **General N -> M rules**: supports deletion (`1 -> 0`), epenthesis (`0 -> 1`), diphthongization (`1 -> 2`), compensatory lengthening (`2 -> 1`), and metathesis (`2 -> 2`).
- **Noise-free traceability**: every derivation produces an immutable `DerivationTrace` recording only steps where the word actually changed.
- **Strict determinism**: core structures and serialization rely exclusively on `BTreeMap`, `BTreeSet`, and `Vec` (never `HashMap` or `HashSet`).
- **`std`-only core**: `phonology`, `engine`, and `dsl` depend solely on the Rust standard library. External crates are deferred to the CLI (`clap`) in Milestone 4.

See [`SPEC.md`](SPEC.md) for the full technical specification.

## Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
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
│   └── main.rs
└── tests/
    └── fixtures/
        └── golden_mvp.tsv
```

## Roadmap

- [x] **M0**: Repository setup (CI, templates, license, and base documentation).
- [ ] **M1**: Core phonology (`src/phonology/`) and rule engine (`src/engine/`) with golden MVP tests.
- [ ] **M2**: DSL lexer/parser (`src/dsl/`) and maximal-munch tokenizer.
- [ ] **M3**: Dialect trees, chronological stages, directional modes, and non-local contexts.
- [ ] **M4**: CLI (`src/cli/`) with `clap` and Markdown/JSON export.

## License

[MIT](LICENSE)
