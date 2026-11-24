# PhonEngine — Especificación Técnica

## Objetivo
Motor determinista de evolución fonológica y reconstrucción comparativa escrito en Rust.
Modela cómo una protolengua (ej. latín clásico o protoindoeuropeo) evoluciona paso a paso
mediante leyes fonéticas ordenadas hacia uno o más dialectos hijos.

## Principios de diseño
1. **No operar sobre texto plano (`String`/`char`)**: Toda palabra se representa como
   `Vec<Segment>`, separando identidad fonológica intrínseca y prosodia.
2. **Reglas $N \to M$ basadas en rasgos y literales**: El motor soporta borrado ($1 \to 0$),
   epéntesis ($0 \to 1$), diptongación ($1 \to 2$), alargamiento compensatorio ($2 \to 1$)
   y metátesis ($2 \to 2$) desde el Hito 1.
3. **Trazabilidad sin ruido**: Toda derivación devuelve un `DerivationTrace` inmutable con
   la forma inicial, los pasos donde `word_after != word_before` (los emparejamientos vacuos
   no se registran) y la forma final.
4. **Determinismo total**: Prohibido iterar sobre estructuras no ordenadas (`HashMap`, `HashSet`)
   en el núcleo o en serialización. Usar `BTreeMap`, `BTreeSet` o `Vec`.
5. **Núcleo `std`-only**: `phonology`, `engine` y `dsl` usan exclusivamente la librería estándar
   de Rust. Crates externos solo en `cli` (Hito 4).
6. **Convención Unicode**: Toda entrada debe estar en UTF-8 NFC. En el Hito 1 los segmentos se
   ingresan separados por espacios (`v i t a m`); en el Hito 2 el tokenizador de palabras sin
   espacios usará *maximal munch* (símbolos del inventario ordenados por longitud descendente).

## Modelo fonológico (`src/phonology/`)
- **Enums de rasgos (`Copy`, `Eq`, `Ord`)**:
  - `Manner`: `Stop`, `Fricative`, `Affricate`, `Nasal`, `Lateral`, `Rhotic`, `Approximant`, `Vowel`.
  - `Place`: `Bilabial`, `Labiodental`, `Dental`, `Alveolar`, `Postalveolar`, `Palatal`, `Velar`,
    `Uvular`, `Pharyngeal`, `Glottal`.
  - `Height`: `High`, `MidHigh`, `MidLow`, `Low`.
  - `Backness`: `Front`, `Central`, `Back`.
  - `Stress`: `Unstressed`, `Primary`, `Secondary`.
- **`FeatureBundle` (`Copy`, `Eq`, `Ord`)**:
  Combina `manner`, `place: Option<Place>`, `height: Option<Height>`, `backness: Option<Backness>`
  y booleanos `voiced`, `aspirated`, `palatalized`, `labialized`, `rounded`, `syllabic`, `long`.
- **Separación prosódica en `Segment`**:
  ```rust
  pub struct Segment {
      pub symbol: String,
      pub bundle: FeatureBundle,
      pub stress: Stress,
  }
  ```
  El acento (`Stress`) queda fuera de `FeatureBundle` para evitar explosión combinatoria en `Inventory`.
- **Patrones y mutaciones explícitos**:
  - `FieldMatch<T>`: `Any`, `Is(T)`, `IsNone` (usado en `FeaturePattern` para consultar campos `Option<T>`).
  - `FieldUpdate<T>`: `Keep`, `Set(T)`, `Clear` (usado en `FeatureMutation` para actualizar campos `Option<T>`).
  - `FeaturePattern::matches(&self, bundle: &FeatureBundle) -> bool`.
  - `FeatureMutation::apply(&self, bundle: &FeatureBundle) -> FeatureBundle`.
- **`Inventory` biyectivo**:
  Mantiene mapeo bidireccional determinista (`BTreeMap<String, FeatureBundle>` y
  `BTreeMap<FeatureBundle, String>`). `Inventory::insert()` valida unicidad en ambas direcciones
  y devuelve `Result<(), InventoryError>`. Toda mutación por rasgos se resuelve mediante
  `Inventory::resolve(&bundle)`; si el resultado no existe en el inventario, devuelve
  `Err(EngineError::UnmappedFeatureBundle(bundle))`.

## Semántica del motor de reglas (`src/engine/`)
- **Estructura de `Rule`**:
  - `id: String` y `name: String`
  - `target: Vec<Pattern>` (donde `Pattern` soporta literal por símbolo, `FeaturePattern` o clase natural)
  - `replacement: Vec<ReplacementItem>` (`Literal(String)` o `Mutate { source_index: usize, mutation: FeatureMutation }`)
  - `left_context: Vec<ContextElement>` y `right_context: Vec<ContextElement>`
  - `mode: ApplicationMode` (`Simultaneous`, `LeftToRight`, `RightToLeft`)
- **`ContextElement`**:
  - `WordBoundary` (`#`)
  - `Segment(Pattern)`
  - `ZeroOrMore(Pattern)` *(reservado para reglas no locales como la ley de Grassmann en Hito 2/3)*
- **Validaciones obligatorias de regla (`RuleError`)**:
  - Rechazar inserciones sin contexto (`target.is_empty()` con ambos contextos vacíos) con `RuleError::UnconstrainedInsertion`.
  - Rechazar `Mutate { source_index, .. }` fuera de rango (`source_index >= target.len()`) con `RuleError::InvalidSourceIndex`.
- **Modo de ejecución en Hito 1**:
  - Implementar `ApplicationMode::Simultaneous` (evalúa todas las coincidencias no solapadas sobre un *snapshot* de la palabra antes de construir la salida, evitando bucles infinitos).
  - `LeftToRight` y `RightToLeft` se definen en el enum y devuelven `Err(EngineError::UnsupportedMode)` hasta que se implementen en Hito 2/3.
- **Alcance diferido**:
  - Las variables de asimilación (notación $\alpha$, ej. `[αplace]`) quedan explícitamente fuera del Hito 1 y del Hito 2.

## Estructura de módulos
```text
src/
├── lib.rs           # API pública del crate (consumida por tests/ y main.rs)
├── main.rs          # Punto de entrada mínimo (CLI completo en Hito 4)
├── phonology/       # Features, Pattern/Mutation, Segment, Inventory, Word
├── engine/          # Rule, Context, Applier, DerivationTrace, EngineError
├── dsl/             # (Hito 2) Lexer, Parser recursive descent, AST, diagnósticos
└── cli/             # (Hito 4) Comandos: run, trace, compare
tests/
├── fixtures/        # Casos de prueba golden (TSV)
└── golden_mvp.rs    # Suite de integración contra fixtures
```

## Hoja de ruta
- **Hito 1 (Actual)**: `phonology/` + `engine/` + `tests/golden_mvp.rs` pasando.
- **Hito 2**: `dsl/` (parser *recursive descent* manual, tokenizador *maximal munch*).
- **Hito 3**: Árbol de dialectos y etapas cronológicas (ej. PIE -> protogriego -> ático / jónico), modos direccionales y contextos no locales.
- **Hito 4**: CLI con `clap` y exportación a Markdown / JSON.

## Borrador de sintaxis DSL (referencia para Hito 2)
```text
class V = { a, e, i, o, u }

rule final_m_loss:
  m > ∅ / _ #

rule intervocalic_voicing:
  [-voiced, +stop] > [+voiced] / V _ V
```