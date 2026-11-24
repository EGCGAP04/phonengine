# Instrucciones para el agente

## Reglas de oro
- Leé `SPEC.md` completo antes de crear módulos o proponer cambios de arquitectura.
- Trabajá estrictamente dentro del hito activo. No implementes `src/dsl/` (Hito 2),
  bifurcación de dialectos (Hito 3) ni `src/cli/` (Hito 4) mientras el Hito 1 no esté cerrado.
- Cero dependencias externas en `Cargo.toml` durante los Hitos 1 y 2.
- Determinismo estricto: usá `BTreeMap` / `BTreeSet` / `Vec`; nunca `HashMap` / `HashSet`.
- Respetá los lints de `Cargo.toml`: prohibido `unsafe`, `.unwrap()`, `.expect()` y `panic!()`
  en `src/`. Propagá errores mediante `Result` y enums propios que implementen `Display` y `Error`.

## Convenciones de Rust
- Código idiomático, tipos explícitos y enums en lugar de strings mágicos para rasgos fonéticos.
- Toda función de aplicación de reglas debe ser pura (sin I/O ni estado global mutable).
- `src/lib.rs` debe exponer una API pública limpia; `tests/` debe consumir el crate como cliente externo.
- Cada módulo público debe incluir tests unitarios (`#[cfg(test)]`) cubriendo tanto casos válidos
  como errores esperados (`InventoryError`, `RuleError`, `EngineError`).

## Verificación obligatoria antes de cerrar cualquier tarea
Ejecutá siempre este comando único y no des la tarea por terminada si falla algún paso:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```