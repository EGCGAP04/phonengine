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
   ingresan separados por espacios (`v i t a m`); en el Hito 2 el tokenizador *maximal munch*
   (`tokenize_word`) consume palabras sin espacios usando los símbolos del inventario ordenados por
   longitud descendente en bytes y soporta marcas IPA de acento `ˈ` (U+02C8) y `ˌ` (U+02CC).

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
├── dsl/             # (Hito 2) Lexer, Parser recursive descent, AST, diagnósticos y maximal munch
└── cli/             # (Hito 4) Comandos: run, trace, compare
tests/
├── fixtures/        # Casos de prueba golden (TSV)
└── golden_mvp.rs    # Suite de integración contra fixtures
```

## Hoja de ruta
- **Hito 1 (Completado)**: `phonology/` + `engine/` + `tests/golden_mvp.rs` pasando.
- **Hito 2 (Actual)**: `dsl/` (parser *recursive descent* manual, declaraciones `segment`/`class`/`rule`, diagnósticos con `Span`, tokenizador *maximal munch* con marcas de acento `ˈ`/`ˌ`).
- **Hito 3**: Árbol de dialectos y etapas cronológicas (ej. PIE -> protogriego -> ático / jónico), modos direccionales y contextos no locales.
- **Hito 4**: CLI con `clap` y exportación a Markdown / JSON.

## Lenguaje DSL y tokenizador maximal munch (`src/dsl/`)

### Salida del parser (`Program`)
El parser *recursive descent* manual expone `parse_program(source: &str) -> Result<Program, DslError>` y devuelve:
- `inventory: Inventory`: construido a partir de las declaraciones `segment` (puede quedar vacío si el archivo solo declara clases y reglas).
- `classes: BTreeMap<String, NaturalClass>`: clases naturales declaradas, ordenadas de forma determinista.
- `rules: Vec<Rule>`: lista ordenada de reglas validadas.

Adicionalmente, `Program::validate_against(&self, inventory: &Inventory) -> Result<(), DslError>` verifica que todos los símbolos literales usados en `class`, `Pattern::Literal` y `ReplacementItem::Literal` existan en un `Inventory` dado (útil cuando el inventario se define fuera del archivo DSL o en combinación con el declarado en `Program`).

### Léxico y comentarios
- Entrada en UTF-8 NFC.
- Comentarios de línea con `//` hasta el final de la línea (no usar `#`, que representa `ContextElement::WordBoundary`).
- Palabras reservadas de inicio de declaración: `segment`, `class`, `rule`.
- Símbolos fonológicos e identificadores sin comillas: secuencias Unicode sin espacios que no contengan operadores ni delimitadores del DSL (`{ } [ ] ( ) , : = > / _ # * + - $ " ;`). Se permite `-` interno en identificadores/rasgos compuestos (`mid-high`, `mid-low`, `left-to-right`, `right-to-left`), mientras que un `-` inicial siempre se tokeniza como signo negativo.
- Literales entre comillas dobles `"..."` para nombres legibles de reglas, con soporte de escapes `\"` y `\\`.

### Declaración de inventario (`segment`)
```text
segment <símbolo> = [ <lista_de_rasgos> ]
```
- Puede extenderse en múltiples líneas dentro de `[...]`.
- El rasgo `manner` es obligatorio (en forma corta `stop` / `+stop` / `vowel` / `+vowel` o explícita `manner=stop`).
- Valores por defecto coherentes con `FeatureBundle::vowel` y `FeatureBundle::consonant`:
  - Si `manner == Vowel`: `voiced = true`, `syllabic = true`, resto de booleanos `false`, y `place = None`, `height = None`, `backness = None`.
  - Si `manner != Vowel`: todos los booleanos en `false`, y `place = None`, `height = None`, `backness = None`.
  - Cualquier rasgo especificado en `[...]` sobrescribe el valor por defecto.
- Se registra mediante `Inventory::insert(...)`: colisiones de símbolo (`DuplicateSymbol`), de `FeatureBundle` (`DuplicateBundle`, violación de biyectividad) o símbolos inválidos devuelven `DslError` con el `Span` de la declaración.

### Declaración de clases naturales (`class`)
```text
class <nombre> = { <símbolo>, <símbolo>, ... }
```
- Admite coma final opcional y saltos de línea dentro de `{...}`.
- Requiere al menos un miembro; rechaza nombres de clase duplicados y miembros repetidos dentro de la misma declaración.

### Especificación de rasgos `[...]`
La misma sintaxis de lista de rasgos separada por comas se aplica en `segment`, `FeaturePattern` y `FeatureMutation`:
- **Booleanos**: `+voiced` / `-voiced`, `+aspirated` / `-aspirated`, `+palatalized` / `-palatalized`, `+labialized` / `-labialized`, `+rounded` / `-rounded`, `+syllabic` / `-syllabic`, `+long` / `-long`.
- **Enumerados** (los nombres de todas las variantes son disjuntos entre sí):
  - `Manner`: `stop`, `fricative`, `affricate`, `nasal`, `lateral`, `rhotic`, `approximant`, `vowel` (forma corta `stop` / `+stop` o `manner=stop`).
  - `Place`: `bilabial`, `labiodental`, `dental`, `alveolar`, `postalveolar`, `palatal`, `velar`, `uvular`, `pharyngeal`, `glottal` (forma corta `velar` / `+velar` o `place=velar`).
  - `Height`: `high`, `mid-high`, `mid-low`, `low` (forma corta `mid-high` / `+mid-high` o `height=mid-high`).
  - `Backness`: `front`, `central`, `back` (forma corta `front` / `+front` o `backness=front`).
- **Ausencia / limpieza de dimensiones opcionales**: `-place` / `place=none`, `-height` / `height=none`, `-backness` / `backness=none` (mapean a `FieldMatch::IsNone` en patrones, `FieldUpdate::Clear` en mutaciones, y `None` en `segment`).
- **Validaciones**: rechazar `-manner` / `manner=none`, la negación de variantes enumeradas (`-stop`, `-velar`, etc.) y campos repetidos dentro del mismo `[...]`.

### Declaración de reglas (`rule`)
```text
rule <id> ["<nombre>"] [(simultaneous | left-to-right | right-to-left)]:
  <target> > <replacement> [/ <left_context> _ <right_context>]
```
- Tras `:`, se permiten saltos de línea y comentarios opcionales; el cuerpo de la regla ocupa una única línea lógica (terminada por salto de línea o EOF).
- Si se omite `"<nombre>"`, `name` toma el valor de `id`. Si se omite el modo, se usa `ApplicationMode::Simultaneous`.
- **Vacío en `target` o `replacement`**: `∅` (U+2205) o alias ASCII `0` (epéntesis $0 \to M$ y borrado $N \to 0$).
- **Patrones en `target` y contextos**:
  - Un identificador que coincide con una `class` declarada previamente se resuelve como `Pattern::Class`; en caso contrario, como `Pattern::Literal`.
  - Un bloque `[...]` se resuelve como `Pattern::Features(FeaturePattern)`.
- **Reemplazos ($N \to M$)**:
  - Un símbolo literal produce `ReplacementItem::Literal` (usar el nombre de una `class` en `replacement` es error).
  - Mutación implícita `[...]`: permitida únicamente cuando `target.len() == 1` (`source_index: 0`); si `target.len() != 1`, produce error de ambigüedad.
  - Referencia indexada 1-based `$1`, `$2`, ... (mapeada a `source_index = k - 1`):
    - `$k` sin corchetes equivale a `ReplacementItem::Mutate { source_index: k - 1, mutation: FeatureMutation::new() }` (copia el segmento preservando su `stress`).
    - `$k[...]` sin espacio entre `$k` y `[` aplica la mutación especificada al segmento `k - 1` (si hay espacio entre `$k` y `[`, se interpretan como dos ítems distintos).
    - `$0` o `$k > target.len()` producen error de diagnóstico.
- **Contexto (`/ <left_context> _ <right_context>`)**:
  - Opcional; si aparece `/`, exige exactamente un `_`.
  - Soporta `#` (`ContextElement::WordBoundary`), `<patrón>` (`ContextElement::Segment`) y `<patrón>*` (`ContextElement::ZeroOrMore`, parseado al AST en Hito 2; su ejecución en `apply_rule` sigue devolviendo `Err(EngineError::UnsupportedContext)` hasta el Hito 3).
- **Validaciones finales**: rechazar IDs de regla duplicados en el mismo archivo; cada regla se construye mediante `Rule::new` (propagando `RuleError` envuelto en `DslError` con el `Span` de la regla).

### Diagnósticos (`Span` y `DslError`)
- `Span { start: usize, end: usize, line: usize, column: usize }`: offsets en bytes `[start, end)` y posición inicial 1-based en escalares Unicode (`line`, `column`).
- `DslError { kind: DslErrorKind, span: Span }`: implementa `Display` (formato `<line>:<column>: <mensaje>`) y `std::error::Error`, cubriendo errores léxicos, sintácticos, semánticos (clases duplicadas/vacías, referencias `$k` inválidas, colisiones de `Inventory`, IDs de regla duplicados) y `RuleError`.

### Tokenizador *maximal munch* (`tokenize_word`)
- Firma pública: `tokenize_word(input: &str, inventory: &Inventory) -> Result<Word, TokenizeError>`.
- Orden determinista de candidatos del inventario: `(Reverse(symbol.len()), symbol)` (longitud descendente en bytes y desempate lexicográfico).
- Rechaza espacios en blanco dentro de la palabra (`UnexpectedWhitespace`).
- En cada posición intenta primero el prefijo más largo registrado en `inventory`; si ninguno coincide, reconoce las marcas IPA de acento `'ˈ'` (U+02C8 → `Stress::Primary`) y `'ˌ'` (U+02CC → `Stress::Secondary`), aplicándolas al segmento inmediatamente siguiente (salvo que la marca estuviera registrada explícitamente en `inventory`).
- Errores estructurados en `TokenizeError` (con `offset` en bytes y `column` 1-based en escalares Unicode, implementando `Display` y `std::error::Error`): `UnknownSegment`, `UnexpectedWhitespace`, `DuplicateStressMarker`, `DanglingStressMarker`.
- La entrada vacía `""` produce `Ok(Word::new(vec![]))`.

### Ejemplo completo en DSL
```text
// Inventario mínimo
segment p = [stop, bilabial, -voiced]
segment b = [stop, bilabial, +voiced]
segment t = [stop, alveolar, -voiced]
segment r = [rhotic, alveolar, +voiced]
segment s = [fricative, alveolar, -voiced]
segment a = [vowel, low, central]
segment aː = [vowel, low, central, +long]
segment e = [vowel, mid-high, front]
segment m = [nasal, bilabial, +voiced]

class V = { a, aː, e }
class C = { p, b, t, r, s, m }

rule final_m_loss "Final -m deletion":
  m > ∅ / _ #

rule intervocalic_voicing (simultaneous):
  [-voiced, +stop] > [+voiced] / V _ V

// Regla 2 -> 2 (metátesis)
rule metathesis_rt:
  r t > $2 $1

// Regla 2 -> 1 (alargamiento compensatorio)
rule comp_lengthening:
  V s > $1[+long] / _ C
```