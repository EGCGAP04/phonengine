# Instrucciones para el agente

## Reglas de oro
- Leé `SPEC.md` completo antes de crear módulos o proponer cambios de arquitectura.
- Trabajá estrictamente dentro del hito activo (**Hito 4**: `src/cli/`, `src/main.rs`, `Word::to_compact` y tests `tests/cli.rs`). Los Hitos 1, 2 y 3 están cerrados.
- `phonology`, `engine` y `dsl` siguen estrictamente `std`-only; únicamente `clap` (con feature `derive`) se permite en `Cargo.toml` y solo se importa desde `src/cli/`. La serialización JSON se hace a mano en `src/cli/` (sin `serde` ni `serde_json`).
- Determinismo estricto: usá `BTreeMap` / `BTreeSet` / `Vec`; nunca `HashMap` / `HashSet`.
- Respetá los lints de `Cargo.toml`: prohibido `unsafe`, `.unwrap()`, `.expect()` y `panic!()` en `src/`. Propagá errores mediante `Result` y enums propios que implementen `Display` y `Error`.
- **Todo cambio de especificación va primero a `SPEC.md`** y después al código.

## Cómo trabajamos
- Antes de editar un archivo, releelo: puede tener cambios manuales de Enzo. Respetalos; si alguno parece erróneo, avisalo en lugar de pisarlo.
- Antes de un cambio grande (un módulo nuevo o más de ~3 archivos), mostrá un plan corto y esperá el OK.
- Trabajá en pasos chicos y revisables; al terminar, resumí qué cambió y explicá cualquier decisión no obvia.
- El agente puede commitear, pushear, crear Issues y abrir PRs por su cuenta (autorizado por Enzo el 2026-11-24). **El merge** requiere siempre pedido explícito de Enzo para ese PR.

## Git y GitHub
- Repo: `github.com/EGCGAP04/phonengine` (público, licencia MIT). Usá `gh` para todo lo de GitHub y verificá el estado con `gh` en lugar de confiar en las ramas locales. Si `gh` tiene más de una cuenta logueada, la activa debe ser `EGCGAP04` (`gh auth switch`).
- `main` está protegida por ruleset: **sin push directo**, sin force-push y sin borrado. Todo entra por PR (sin aprobaciones requeridas) y el check obligatorio `Build and test` debe pasar.
- **Commits firmados en todas las ramas** (ruleset `signed-commits`): configurados por SSH en `~/.gitconfig` (`commit.gpgsign true`). Nunca uses `--no-gpg-sign`; si un commit sale sin firmar, rehacelo firmado antes de pushear.
- **Sin atribución**: los commits no llevan `Co-Authored-By` ni los PRs pies de tipo "Generated with...".
- **CI** (`.github/workflows/ci.yml`): el job `Build and test` corre `cargo test`, `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` en cada PR y en `main`. Si cambia el `name:` del job, actualizá el ruleset. Las actions van fijadas por SHA completo con la versión en comentario (`@<sha> # v7.0.1`).
- **Dependabot** (`.github/dependabot.yml`): abre PRs agrupados para Cargo y GitHub Actions; los revisa y mergea Enzo (no tocarlos salvo que lo pida). Dependabot no actualiza `CHANGELOG.md`: sumá sus cambios en la entrada del próximo PR.
- **Flujo**: Issue → rama desde `main` actualizada → commits firmados (`Refs #N`) → `git push -u origin <rama>` → `gh pr create` → squash merge (solo cuando Enzo lo pide). La rama remota se borra sola al mergear; después ejecutá `git switch main && git pull --ff-only` y borrá la rama local (`git branch -D <rama>`).
- **Nombre de rama**: `<tipo>/<número-de-issue>-<descripción-corta>` en inglés (ej. `feat/6-dsl-parser`, `docs/5-m2-dsl-spec-and-agents`).
- **Squash merge y Conventional Commits**: solo está habilitado squash merge; el **título del PR** pasa a ser el mensaje del commit en `main`, en inglés siguiendo Conventional Commits con el Issue al final (ej. `feat(dsl): implement lexer, parser and maximal munch (Issue #6)`). Tipos: `feat`, `fix`, `docs`, `test`, `refactor`, `build`, `ci`, `chore`. Scopes: `phonology`, `engine`, `dsl`, `cli`.
- **Cuerpo del PR**: sigue `.github/pull_request_template.md` e incluye `Closes #N`. Marcá las casillas con honestidad y documentá decisiones extra en *Additional notes*.
- **Actualizaciones obligatorias por PR**:
  - Actualizá `CHANGELOG.md` bajo `[Unreleased]` (Keep a Changelog, Semantic Versioning, en inglés).
  - Si se agregan, borran o renombran archivos o carpetas, actualizá el árbol *Project structure* en `README.md` contrastándolo con `git ls-files` (más `git ls-files --others --exclude-standard`), no a ojo.
- **Issues, etiquetas, milestones y proyecto**:
  - Formularios en `.github/ISSUE_TEMPLATE/` (`feature`, `bug`, `chore`, `docs`, `refactor`; issues en blanco deshabilitados). Al usar `gh issue create --body-file`, replicá las secciones del formulario. Al crear o cerrar una milestone, actualizá el desplegable de milestone en los cinco formularios.
  - Cada Issue y PR va asignado a `EGCGAP04` (`--assignee @me`), con etiquetas de tipo (`feature`, `bug`, `chore`, `documentation`, `refactor`, `blocked`) y área (`phonology`, `engine`, `dsl`, `cli`), milestone activa y en el proyecto [#8 `phonengine`](https://github.com/users/EGCGAP04/projects/8) (`In Progress` al arrancar, `Done` al cerrar/mergear).
  - Milestones abiertas: `M4: CLI and export formats`. Las cierra Enzo (o el agente cuando Enzo lo pida).
- **Merge**: solo cuando Enzo lo pide para ese PR. Verificá antes que esté `MERGEABLE` y que pasen los checks, luego ejecutá `gh pr merge <N> --squash --match-head-commit <SHA completo>`.

## Convenciones de Rust e idioma
- Código, identificadores, docstrings (`///`), mensajes `Display` de errores, commits, Issues, PRs, `README.md` y `CHANGELOG.md` en **inglés**; `SPEC.md` y `AGENTS.md` en **castellano**.
- Código idiomático, tipos explícitos y enums en lugar de strings mágicos para rasgos fonéticos.
- Toda función de aplicación de reglas debe ser pura (sin I/O ni estado global mutable).
- `src/lib.rs` debe exponer una API pública limpia; `tests/` debe consumir el crate como cliente externo.
- Cada módulo público debe incluir tests unitarios (`#[cfg(test)]`) cubriendo tanto casos válidos como errores esperados (`InventoryError`, `RuleError`, `EngineError`, `DialectError`, `DslError`, `TokenizeError`, `CliError`).

## Verificación obligatoria antes de cerrar cualquier tarea
Ejecutá siempre este comando único y no des la tarea por terminada si falla algún paso:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
