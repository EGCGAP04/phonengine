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
  - `ZeroOrMore(Pattern)`: clausura de Kleene no local (`Patrón*`, p. ej. ley de Grassmann o armonía vocálica). En `matches_left_context` y `matches_right_context` se evalúa de forma determinista mediante alcanzabilidad DP / autómata finito sobre índices usando `Vec<bool>` en tiempo $O(|palabra| \cdot |contexto|)$ (sin recursión ni *backtracking* exponencial ante secuencias como `V* V*`), manteniendo compatibilidad exacta con `WordBoundary` (`#`) y `Segment(Pattern)`.
- **Validaciones obligatorias de regla (`RuleError`)**:
  - Rechazar inserciones sin contexto (`target.is_empty()` con ambos contextos vacíos) con `RuleError::UnconstrainedInsertion`.
  - Rechazar `Mutate { source_index, .. }` fuera de rango (`source_index >= target.len()`) con `RuleError::InvalidSourceIndex`.
- **Modos de ejecución (`ApplicationMode`)**:
  - **`Simultaneous`**: evalúa todas las coincidencias no solapadas de izquierda a derecha sobre un *snapshot* inmutable de la palabra antes de construir la salida (evita cascadas y bucles infinitos).
  - **`LeftToRight`**: pasada única de izquierda a derecha con mutación progresiva sobre la palabra `current` (permite *feeding* y *bleeding* deterministas con terminación garantizada en $O(|palabra|)$):
    - Para $N = target.len() \ge 1$: mantiene cursor `pos = 0` mientras `pos + N <= current.len()`. Evalúa target y contextos sobre `current`; si coincide, reemplaza `current[pos..pos + N]` por los $M$ segmentos producidos y avanza `pos += M` (no vuelve a inspeccionar lo recién emitido; `current.len() - pos` decrece estrictamente en $N$). Si no coincide, `pos += 1`.
    - Para epéntesis ($N = 0$): recorre las brechas `0..=current.len()` de izquierda a derecha; si inserta $M$ segmentos en `pos`, avanza saltando lo insertado y el siguiente segmento original (o termina si estaba al final `#`).
  - **`RightToLeft`**: espejo exacto de `LeftToRight` con pasada única de derecha a izquierda:
    - Para $N = target.len() \ge 1$: mantiene límite derecho exclusivo `end = current.len()` mientras `end >= N`, evalúa `current[end - N..end]` sobre `current`; si coincide, reemplaza ese tramo por los $M$ segmentos producidos y actualiza `end -= N` (lo recién emitido queda a la derecha y es visible en el `right_context` de posiciones anteriores). Si no coincide, `end -= 1`.
    - Para epéntesis ($N = 0$): recorre `pos` desde `current.len()` hasta `0`; cada inserción de $M$ segmentos en `pos` queda visible en el `right_context` de las posiciones anteriores.
- **Errores del motor (`EngineError`)**:
  - En el Hito 3 se implementan `LeftToRight`, `RightToLeft` y `ZeroOrMore`, por lo que se eliminan las variantes transitorias `UnsupportedMode` y `UnsupportedContext` del Hito 1/2.
  - Se agrega `EngineError::UnknownStage(String)` para consultas sobre etapas inexistentes en un `DialectTree`.
- **Alcance diferido**:
  - Las variables de asimilación (notación $\alpha$, ej. `[αplace]`) quedan explícitamente fuera de los Hitos 1, 2 y 3.

## Árbol de dialectos y etapas cronológicas (`src/engine/dialect.rs`)
- **Nodos y árbol (`DialectNode`, `DialectTree`)**:
  - `DialectNode`: `id: String`, `name: String`, `parent: Option<String>`, `rules: Vec<Rule>`.
  - `DialectTree`: almacena los nodos en `BTreeMap<String, DialectNode>` (determinismo estricto) y comparte un único `Inventory` para toda la familia lingüística.
  - Los `id` de regla deben ser únicos **dentro de cada etapa** (desambiguados por `stage_id` en `StageTrace`).
- **Validaciones (`DialectError`, implementa `Display` y `std::error::Error`)**:
  - `EmptyTree`: árbol sin etapas.
  - `EmptyStageId`: etapa con `id` vacío o solo espacios.
  - `DuplicateStageId(String)`: dos etapas con el mismo `id`.
  - `UnknownParent { stage_id: String, parent_id: String }`: referencia a una etapa padre inexistente.
  - `MissingRoot` / `MultipleRoots(Vec<String>)`: el árbol debe tener exactamente una raíz (`parent: None`).
  - `CycleDetected(Vec<String>)`: ciclo en las referencias `parent`.
- **Evaluación y trazabilidad (`StageTrace`, `DialectTrace`, `TreeDerivation`)**:
  - `StageTrace { stage_id: String, stage_name: String, trace: DerivationTrace }`: envuelve la derivación de una etapa (omitiendo pasos vacuos en `trace`).
  - `DialectTree::derive_stage(&self, initial: &Word, target_stage: &str, inventory: &Inventory) -> Result<DialectTrace, EngineError>`: ejecuta las etapas en orden cronológico a lo largo del camino raíz → `target_stage`, devolviendo `DialectTrace { initial, target_stage, stages: Vec<StageTrace>, final_word }` (o `Err(EngineError::UnknownStage)`).
  - `DialectTree::derive_tree(&self, initial: &Word, inventory: &Inventory) -> Result<TreeDerivation, EngineError>`: recorre todo el árbol en orden topológico/DFS determinista **reutilizando** el `final_word` de cada etapa padre al evaluar sus hijas (sin recalcular ancestros compartidos). Expone `root_id()`, `initial()`, `stages() -> &BTreeMap<String, StageTrace>`, `word_at(stage_id) -> Option<&Word>`, `path_to(stage_id) -> Option<Vec<&StageTrace>>` y `leaf_outcomes() -> BTreeMap<&str, &Word>` (base directa para comparación de dialectos).

## Estructura de módulos
```text
src/
├── lib.rs           # API pública del crate (consumida por tests/ y main.rs)
├── main.rs          # Punto de entrada mínimo (CLI completo en Hito 4)
├── phonology/       # Features, Pattern/Mutation, Segment, Inventory, Word
├── engine/          # Rule, Context (local y ZeroOrMore), Applier (3 modos), DialectTree, DerivationTrace, Errors
├── dsl/             # Lexer, Parser recursive descent, AST (incl. stage), diagnósticos y maximal munch
└── cli/             # (Hito 4) Comandos: run, trace, compare
tests/
├── fixtures/        # Casos de prueba golden (TSV)
├── golden_mvp.rs    # Suite de integración contra fixtures
└── dsl_mvp.rs       # Suite de integración DSL + maximal munch
```

## Hoja de ruta
- **Hito 1 (Completado)**: `phonology/` + `engine/` + `tests/golden_mvp.rs` pasando.
- **Hito 2 (Completado)**: `dsl/` (parser *recursive descent* manual, declaraciones `segment`/`class`/`rule`, diagnósticos con `Span`, tokenizador *maximal munch* con marcas de acento `ˈ`/`ˌ`).
- **Hito 3 (Actual)**: Árbol de dialectos y etapas cronológicas (`DialectTree`, bloques `stage` en DSL), modos direccionales (`LeftToRight`, `RightToLeft`) y contextos no locales (`ZeroOrMore`).
- **Hito 4**: CLI con `clap` y exportación a Markdown / JSON.

## Lenguaje DSL y tokenizador maximal munch (`src/dsl/`)

### Salida del parser (`Program`)
El parser *recursive descent* manual expone `parse_program(source: &str) -> Result<Program, DslError>` y devuelve:
- `inventory: Inventory`: construido a partir de las declaraciones `segment` (puede quedar vacío si el archivo solo declara clases y reglas/etapas).
- `classes: BTreeMap<String, NaturalClass>`: clases naturales declaradas, ordenadas de forma determinista.
- `rules: Vec<Rule>`: lista ordenada de reglas de nivel superior (vacía si el archivo define etapas `stage`).
- `dialect_tree: Option<DialectTree>`: `Some(tree)` si el archivo declara bloques `stage`, o `None` si usa reglas planas de nivel superior.

Adicionalmente, `Program::validate_against(&self, inventory: &Inventory) -> Result<(), DslError>` verifica que todos los símbolos literales usados en `class`, `Pattern::Literal` y `ReplacementItem::Literal` (tanto en reglas de nivel superior como dentro de etapas `stage`) existan en un `Inventory` dado.

### Léxico y comentarios
- Entrada en UTF-8 NFC.
- Comentarios de línea con `//` hasta el final de la línea (no usar `#`, que representa `ContextElement::WordBoundary`).
- Palabras reservadas de inicio de declaración: `segment`, `class`, `rule`, `stage` (`from` es palabra clave contextual en la cabecera de `stage`).
- Símbolos fonológicos e identificadores sin comillas: secuencias Unicode sin espacios que no contengan operadores ni delimitadores del DSL (`{ } [ ] ( ) , : = > / _ # * + - $ " ;`). Se permiten `-` y `_` internos en identificadores/rasgos compuestos (`mid-high`, `mid-low`, `left-to-right`, `right-to-left`, `final_m_loss`), mientras que un `-` inicial siempre se tokeniza como signo negativo y un `_` inicial o aislado siempre se tokeniza como marcador de foco del contexto (`_`).
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
  - Soporta `#` (`ContextElement::WordBoundary`), `<patrón>` (`ContextElement::Segment`) y `<patrón>*` (`ContextElement::ZeroOrMore`).
- **Validaciones finales**: rechazar IDs de regla duplicados en el mismo ámbito (nivel superior o misma etapa `stage`); cada regla se construye mediante `Rule::new` (propagando `RuleError` envuelto en `DslError` con el `Span` de la regla).

### Declaración de etapas y árbol de dialectos (`stage`)
```text
stage <id> ["<nombre>"] [from <padre_id>] {
  rule ...
}
```
- `segment` y `class` siguen siendo declaraciones de nivel superior compartidas por todas las etapas.
- Si se omite `"<nombre>"`, `name` toma el valor de `id`.
- Si se omite `from <padre_id>`, la etapa es raíz (`parent: None`).
- El cuerpo `{ ... }` admite cero o más declaraciones `rule` (los IDs de regla deben ser únicos dentro de cada etapa).
- No se pueden mezclar declaraciones `rule` sueltas de nivel superior y bloques `stage` en el mismo archivo (`DslErrorKind::MixedTopLevelRulesAndStages`).
- El árbol resultante se valida con `DialectTree::new`, envolviendo cualquier `DialectError` en `DslErrorKind::InvalidDialectTree(DialectError)`.

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

### Ejemplo de árbol de dialectos en DSL (Hito 3)
```text
class V = { a, aː, e, eː, i, o, u }

stage pie "Proto-Indo-European" {}

stage proto_greek "Proto-Greek" from pie {
  rule grassmann (left-to-right):
    [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]

  rule s_debuccalization:
    s > h / # _ V
}

stage attic "Attic Greek" from proto_greek {
  rule eta_reversion:
    eː > aː / r _
}

stage ionic "Ionic Greek" from proto_greek {
  rule a_raising:
    aː > eː
}
```