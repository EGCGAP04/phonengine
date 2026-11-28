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
   ingresan separados por espacios (`v i t a m`, con prefijo `ˈ`/`ˌ` opcional para el acento:
   `v ˈi t a m`); en el Hito 2 el tokenizador *maximal munch*
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
- **Coherencia de `Segment` con el inventario**: los campos de `Segment` son públicos.
  `Inventory::segment`, `Word::from_spaced` y `tokenize_word` construyen segmentos coherentes
  (`symbol` ↔ `bundle`); quien construya un `Segment` a mano es responsable de esa coherencia, porque
  `Pattern::Literal` y `NaturalClass` comparan `symbol` mientras que `FeaturePattern` compara `bundle`.
- **Formato espaciado y compacto con acento (`Word::to_spaced` / `Display`, `Word::to_compact` y `Word::from_spaced`)**:
  - `to_spaced` (y `Display`) escribe cada segmento como su símbolo, precedido de `ˈ` si tiene
    `Stress::Primary` o de `ˌ` si tiene `Stress::Secondary`, separados por espacios (`t e kʰ ˈeː s`).
  - `to_compact` escribe cada segmento concatenado sin espacios, precediendo cada segmento acentuado
    con `Stress::ipa_marker()` (`ˈ` o `ˌ`), p. ej. `tekʰˈeːs`. Cuando el acento recae solo en
    segmentos silábicos (`bundle.syllabic == true`), ninguna marca está registrada como símbolo en el
    inventario y el inventario no tiene ambigüedad *maximal munch*, es reversible con
    `tokenize_word`: `tokenize_word(&word.to_compact(), inventory) == Ok(word)`.
  - `from_spaced` acepta ese prefijo: si un token no es un símbolo registrado, empieza con una marca de
    acento que no está registrada como símbolo y el resto sí es un símbolo registrado, el segmento
    recibe el acento de la marca. En cualquier otro caso devuelve `InventoryError::UnknownSymbol` con el
    token completo.
  - Si ninguna marca está registrada como símbolo, el formato espaciado es reversible:
    `Word::from_spaced(&word.to_spaced(), inventory) == Ok(word)`.
- **Patrones y mutaciones explícitos**:
  - `FieldMatch<T>`: `Any`, `Is(T)`, `IsNone` (usado en `FeaturePattern` para consultar campos `Option<T>`).
  - `FieldUpdate<T>`: `Keep`, `Set(T)`, `Clear` (usado en `FeatureMutation` para actualizar campos `Option<T>`).
  - `FeaturePattern::matches(&self, bundle: &FeatureBundle) -> bool`.
  - `FeatureMutation::apply(&self, bundle: &FeatureBundle) -> FeatureBundle`.
- **Prosodia en patrones y mutaciones (Hito 5)**:
  - `FeatureBundle`, `FeaturePattern` y `FeatureMutation` permanecen estrictamente intrínsecos y `Copy`; el acento y las variables de asimilación operan al nivel de `Segment` y de regla.
  - `StressMatch` (`Copy`, `Eq`, `Ord`, `Default`): `Any` (por defecto), `Stressed` (`Primary | Secondary`), `Is(Stress)` (`Unstressed`, `Primary`, `Secondary`); expone `StressMatch::matches(&self, stress: Stress) -> bool`.
  - `StressUpdate` (`Copy`, `Eq`, `Ord`, `Default`): `Keep` (por defecto), `Set(Stress)`; expone `StressUpdate::apply(&self, stress: Stress) -> Stress`.
  - `LiteralStress` (`Clone`, `Eq`, `Default`): `Inherit` (herencia posicional de `v0.1.0`), `Set(Stress)` (asignación explícita) y `Var(String)` (copia el acento desde una variable ligada de dominio `Stress`).
- **Variables de rasgos y asimilación ($\alpha$, Hito 5)**:
  - `FeatureDomain` (`Copy`, `Eq`, `Ord`): `Manner`, `Place`, `Height`, `Backness`, `Bool`, `Stress`.
  - `FeatureField` (`Copy`, `Eq`, `Ord`): `Manner`, `Place`, `Height`, `Backness`, `Voiced`, `Aspirated`, `Palatalized`, `Labialized`, `Rounded`, `Syllabic`, `Long`, `Stress` (con métodos `domain(&self) -> FeatureDomain` e `is_bool(&self) -> bool`).
  - `VarPolarity` (`Copy`, `Eq`, `Ord`): `Same` (`α`) y `Opposite` (`-α`, solo válido para campos de dominio `Bool`).
  - `FeatureVar` (`Clone`, `Eq`, `Ord`): `{ pub field: FeatureField, pub name: String, pub polarity: VarPolarity }`.
  - `FeatureVarValue` (`Copy`, `Eq`, `Ord`): `Manner(Manner)`, `Place(Place)`, `Height(Height)`, `Backness(Backness)`, `Bool(bool)`, `Stress(Stress)`. Las dimensiones opcionales (`Place`, `Height`, `Backness`) solo ligan valores presentes (`Some(v)`): un segmento con `None` en ese campo no satisface una restricción de variable sobre ese campo.
  - `VarBindings`: mapa determinista (`BTreeMap<String, FeatureVarValue>`) que almacena los valores ligados durante el emparejamiento de una regla.
- **`Inventory` biyectivo**:
  Mantiene mapeo bidireccional determinista (`BTreeMap<String, FeatureBundle>` y
  `BTreeMap<FeatureBundle, String>`). `Inventory::insert()` valida unicidad en ambas direcciones
  y devuelve `Result<(), InventoryError>`. Toda mutación por rasgos se resuelve mediante
  `Inventory::resolve(&bundle)`; si el resultado no existe en el inventario, devuelve
  `Err(EngineError::UnmappedFeatureBundle { rule_id, bundle })`.

## Semántica del motor de reglas (`src/engine/`)
- **Estructura de `Rule`**:
  - `id: String` y `name: String`
  - `target: Vec<Pattern>` donde `Pattern` soporta literal por símbolo, `FeaturePattern`, clase natural o `Constrained(ConstrainedPattern)` (`ConstrainedPattern { pub base: PatternBase, pub stress: StressMatch, pub vars: Vec<FeatureVar> }`, con `PatternBase`: `Literal(String)`, `Features(FeaturePattern)`, `Class(NaturalClass)`).
  - `replacement: Vec<ReplacementItem>` (`Literal(String)`, `LiteralWithStress { symbol: String, stress: LiteralStress }`, o `Mutate { source_index: usize, mutation: SegmentMutation }` donde `SegmentMutation { pub features: FeatureMutation, pub stress: StressUpdate, pub vars: Vec<FeatureVar> }` e implementa `From<FeatureMutation>`).
  - `environments: Vec<Environment>` donde `Environment { pub left: Vec<ContextElement>, pub right: Vec<ContextElement> }` (`vec![]` = regla incondicional; 1 o más entornos = disyunción evaluada de izquierda a derecha, ganando el primer entorno que coincida). `Rule::new(...)` construye una regla de entorno único `(left_context, right_context)` (o incondicional si ambos lados son vacíos), y `Rule::with_environments(...)` acepta múltiples ramas.
  - `mode: ApplicationMode` (`Simultaneous`, `LeftToRight`, `RightToLeft`)
- **Acento en el reemplazo** (sea `matched` el tramo de $N$ segmentos que coincidió con `target`):
  - `Mutate { source_index: k, mutation }` aplica `mutation.stress` (o la variable de dominio `Stress` indicada en `mutation.vars`) sobre `matched[k].stress` (`StressUpdate::Keep` conserva el acento de origen; `Set(s)` lo reemplaza).
  - `LiteralWithStress { symbol, stress }` con `LiteralStress::Set(s)` o `LiteralStress::Var(name)` emite ese acento explícitamente (sin heredar).
  - `Literal(symbol)` y `LiteralWithStress { symbol, stress: LiteralStress::Inherit }` en la posición `i` del reemplazo heredan el acento de `matched[i]` si `i < N`, ningún `Mutate` del mismo reemplazo tiene `source_index == i` y ningún otro ítem del mismo reemplazo asigna el acento explícitamente mediante `LiteralStress::Set(Primary | Secondary)` o `LiteralStress::Var(_)` (ese segmento o reemplazo ya gestiona el acento por otra vía). En cualquier otro caso, incluida la epéntesis ($N = 0$), emiten `Stress::Unstressed`.
  - Ejemplos: `o > u` sobre `pˈo` da `pˈu`; `ɛ[+stressed] > j e[+stressed]` sobre `pˈɛ` da `pjˈe`; `r t > t r` mantiene el acento por posición, mientras que `r t > $2 $1` lo mueve con cada segmento; `e > i e` acentúa `i` (posición 0), mientras que `e > i $1` acentúa solo `e`.
- **`ContextElement`**:
  - `WordBoundary` (`#`)
  - `Segment(Pattern)`
  - `Optional(Pattern)` (`(<patrón>)`, Hito 5): elemento opcional (0 o 1 segmento). Se evalúa deterministamente dentro del autómata DP junto con `ZeroOrMore`. Al poder consumir 0 segmentos, `Optional` tampoco ancla inserciones ni cuenta como posición de ligadura garantizada para variables $\alpha$.
  - `ZeroOrMore(Pattern)`: clausura de Kleene no local (`Patrón*`, p. ej. ley de Grassmann o armonía vocálica). En `matches_left_context` y `matches_right_context` se evalúa de forma determinista mediante alcanzabilidad DP / autómata finito sobre índices usando `Vec<bool>` en tiempo $O(|palabra| \cdot |contexto|)$ (sin recursión ni *backtracking* exponencial ante secuencias como `V* V*`), manteniendo compatibilidad exacta con `WordBoundary` (`#`), `Segment(Pattern)` y `Optional(Pattern)`.
- **Validaciones obligatorias de regla (`RuleError`)**:
  - Rechazar un `id` vacío o formado solo por espacios con `RuleError::EmptyId`.
  - Rechazar inserciones sin anclaje con `RuleError::UnconstrainedInsertion`: cuando `target.is_empty()`, `environments` debe ser no vacío y **cada** rama debe contener al menos un `WordBoundary` o un `Segment(Pattern)`. Un contexto formado solo por elementos `ZeroOrMore` y/u `Optional` coincide en cualquier brecha, así que no restringe la inserción.
  - Rechazar `Mutate { source_index, .. }` fuera de rango (`source_index >= target.len()`) con `RuleError::InvalidSourceIndex`.
  - Rechazar un `WordBoundary` fuera del borde exterior de cada rama del contexto con `RuleError::MisplacedWordBoundary { rule_id }`: en `left` solo puede ser el primer elemento y en `right` solo el último (como máximo uno por lado en cada rama). En otra posición nunca podría coincidir.
  - Validar variables $\alpha$ y restricciones en patrones/mutaciones (Hito 5):
    - `InvalidVariableName { rule_id, var }`: nombre de variable vacío o formado solo por espacios.
    - `InvalidVariablePolarity { rule_id, var, field }`: `VarPolarity::Opposite` (`-α`) sobre un campo cuyo dominio no es `Bool`.
    - `IncompatibleVariableDomain { rule_id, var, first, second }`: una misma variable usada en dos dominios distintos dentro de la misma regla (campos booleanos distintos sí pueden compartir una variable de dominio `Bool`, p. ej. `[+stop, αvoiced] > [αaspirated]`).
    - `ConflictingFeatureConstraint { rule_id, field }`: un mismo campo restringido o mutado más de una vez en el mismo patrón o mutación (incluyendo concreto + variable, dos variables sobre el mismo campo, o `stress` concreto + variable de `stress`).
    - `UnboundVariable { rule_id, var }`: variable referenciada en `replacement` que no aparece en ninguna posición de ligadura garantizada (`target` o `ContextElement::Segment` en **todas** las ramas de `environments`; las apariciones dentro de `ZeroOrMore` u `Optional` no garantizan ligadura).
- **Modos de ejecución (`ApplicationMode`)**:
  - **`Simultaneous`**: evalúa todas las coincidencias no solapadas de izquierda a derecha sobre un *snapshot* inmutable de la palabra antes de construir la salida (evita cascadas y bucles infinitos).
  - **`LeftToRight`**: pasada única de izquierda a derecha con mutación progresiva sobre la palabra `current` (permite *feeding* y *bleeding* deterministas con terminación garantizada en $O(|palabra|)$):
    - Para $N = target.len() \ge 1$: mantiene cursor `pos = 0` mientras `pos + N <= current.len()`. Evalúa target y contextos sobre `current`; si coincide, reemplaza `current[pos..pos + N]` por los $M$ segmentos producidos y avanza `pos += M` (no vuelve a inspeccionar lo recién emitido; `current.len() - pos` decrece estrictamente en $N$). Si no coincide, `pos += 1`.
    - Para epéntesis ($N = 0$): recorre las brechas `0..=current.len()` de izquierda a derecha; si inserta $M$ segmentos en `pos`, avanza saltando lo insertado y el siguiente segmento original (o termina si estaba al final `#`).
  - **`RightToLeft`**: espejo exacto de `LeftToRight` con pasada única de derecha a izquierda:
    - Para $N = target.len() \ge 1$: mantiene límite derecho exclusivo `end = current.len()` mientras `end >= N`, evalúa `current[end - N..end]` sobre `current`; si coincide, reemplaza ese tramo por los $M$ segmentos producidos y actualiza `end -= N` (lo recién emitido queda a la derecha y es visible en el `right` de posiciones anteriores). Si no coincide, `end -= 1`.
    - Para epéntesis ($N = 0$): recorre `pos` desde `current.len()` hasta `0`; cada inserción de $M$ segmentos en `pos` queda visible en el `right` de las posiciones anteriores.
- **Errores del motor (`EngineError`)**:
  - Variantes:
    - `UnmappedFeatureBundle { rule_id: String, bundle: FeatureBundle }`: una mutación produjo un bundle ausente del inventario.
    - `UnknownSymbol { rule_id: String, symbol: String }`: un `ReplacementItem::Literal` o `LiteralWithStress` no está registrado en el inventario.
    - `UnknownStage(String)`: consulta sobre una etapa inexistente en un `DialectTree`.
    - `InvalidRule(RuleError)`: defensiva, si se violan invariantes de `Rule` durante la evaluación (p. ej. una variable sin ligar al evaluar un reemplazo).
    - `StageFailed { stage_id: String, source: Box<EngineError> }`: `derive_stage` y `derive_tree` envuelven así cualquier error producido al evaluar las reglas de una etapa.
  - Todo error de aplicación identifica la regla (`rule_id`) y, dentro de un árbol, la etapa. `Error::source()` de `StageFailed` devuelve el error envuelto, y `Display` encadena el contexto (`stage "attic": rule "a_raising": ...`).
- **Variables de asimilación y disimilación (notación $\alpha$, Hito 5)**:
  - Orden determinista de ligadura por candidato de `target`: primero se evalúa `target` de izquierda a derecha (posiciones fijas, ligadura única en `VarBindings`). Si `environments` está vacío, la coincidencia es válida con esas ligaduras; de lo contrario se evalúan las ramas en el orden declarado hasta encontrar la primera que coincida.
  - Dentro de cada rama se propaga `VarBindings` primero por `left` (desde el foco `_` hacia la izquierda) y luego por `right` (desde el foco `_` hacia la derecha). Si múltiples caminos a través de `ZeroOrMore` / `Optional` satisfacen la misma rama con distintas asignaciones, gana deterministamente la asignación ligada más cercana al foco (`_`, menor distancia al foco). Para reglas sin variables (o cuando no quedan variables sin ligar al entrar en un contexto), el emparejador usa el camino rápido `Vec<bool>`.

## Árbol de dialectos y etapas cronológicas (`src/engine/dialect.rs`)
- **Nodos y árbol (`DialectNode`, `DialectTree`)**:
  - `DialectNode`: `id: String`, `name: String`, `parent: Option<String>`, `rules: Vec<Rule>`.
  - `DialectTree`: almacena los nodos en `BTreeMap<String, DialectNode>` (determinismo estricto). Todas las etapas se evalúan contra un mismo `Inventory`, que se pasa en cada llamada a `derive_stage` / `derive_tree` (el árbol no lo almacena).
  - Los `id` de regla deben ser únicos **dentro de cada etapa** (desambiguados por `stage_id` en `StageTrace`); `DialectTree::new` lo valida.
- **Validaciones (`DialectError`, implementa `Display` y `std::error::Error`)**:
  - `EmptyTree`: árbol sin etapas.
  - `EmptyStageId`: etapa con `id` vacío o solo espacios.
  - `DuplicateStageId(String)`: dos etapas con el mismo `id`.
  - `UnknownParent { stage_id: String, parent_id: String }`: referencia a una etapa padre inexistente.
  - `MissingRoot` / `MultipleRoots(Vec<String>)`: el árbol debe tener exactamente una raíz (`parent: None`).
  - `CycleDetected(Vec<String>)`: ciclo en las referencias `parent`.
  - `DuplicateRuleId { stage_id: String, rule_id: String }`: dos reglas con el mismo `id` dentro de una etapa.
- **Evaluación y trazabilidad (`StageTrace`, `DialectTrace`, `TreeDerivation`)**:
  - `StageTrace { stage_id: String, stage_name: String, trace: DerivationTrace }`: envuelve la derivación de una etapa (omitiendo pasos vacuos en `trace`).
  - `DialectTree::derive_stage(&self, initial: &Word, target_stage: &str, inventory: &Inventory) -> Result<DialectTrace, EngineError>`: ejecuta las etapas en orden cronológico a lo largo del camino raíz → `target_stage`, devolviendo `DialectTrace { initial, target_stage, stages: Vec<StageTrace>, final_word }` (o `Err(EngineError::UnknownStage)` si `target_stage` no existe; los errores de una etapa llegan envueltos en `EngineError::StageFailed`).
  - `DialectTree::derive_tree(&self, initial: &Word, inventory: &Inventory) -> Result<TreeDerivation, EngineError>`: recorre todo el árbol en orden topológico/DFS determinista **reutilizando** el `final_word` de cada etapa padre al evaluar sus hijas (sin recalcular ancestros compartidos). Expone `root_id()`, `initial()`, `stages() -> &BTreeMap<String, StageTrace>`, `word_at(stage_id) -> Option<&Word>`, `path_to(stage_id) -> Option<Vec<&StageTrace>>` y `leaf_outcomes() -> BTreeMap<&str, &Word>` (base directa para comparación de dialectos).

## Estructura de módulos
```text
src/
├── lib.rs           # API pública del crate (consumida por tests/ y main.rs)
├── main.rs          # Punto de entrada mínimo; declara `mod cli;` y despacha a `src/cli/`
├── phonology/       # Features, Pattern/Mutation, Segment, Inventory, Word
├── engine/          # Rule, Context (local y ZeroOrMore), Applier (3 modos), DialectTree, DerivationTrace, Errors
├── dsl/             # Lexer, parser recursive descent (`Program`, incl. `stage`), diagnósticos y maximal munch
└── cli/             # Comandos `run`, `trace`, `compare`, `CliError` y exportación a texto / Markdown / JSON
tests/
├── fixtures/        # Casos de prueba golden (TSV)
├── golden_mvp.rs    # Suite de integración contra fixtures
├── dsl_mvp.rs       # Suite de integración DSL + maximal munch
├── dialect_tree.rs  # Suite de integración de árboles de dialectos, bloques stage y modos direccionales
├── cli.rs           # Suite de integración end-to-end del binario CLI
└── examples.rs      # Suite de integración de gramáticas históricas reales (Hito 5)
examples/            # Gramáticas históricas reales en DSL (Hito 5)
├── latin_to_romance.phon
└── pie_to_greek.phon
```

## Hoja de ruta
- **Hito 1 (Completado)**: `phonology/` + `engine/` + `tests/golden_mvp.rs` pasando.
- **Hito 2 (Completado)**: `dsl/` (parser *recursive descent* manual, declaraciones `segment`/`class`/`rule`, diagnósticos con `Span`, tokenizador *maximal munch* con marcas de acento `ˈ`/`ˌ`).
- **Hito 3 (Completado)**: Árbol de dialectos y etapas cronológicas (`DialectTree`, bloques `stage` en DSL), modos direccionales (`LeftToRight`, `RightToLeft`) y contextos no locales (`ZeroOrMore`). Incluye las correcciones de la revisión de los Hitos 0–3: herencia de acento en reemplazos literales, validación de `#` y de inserciones, errores con contexto de regla y etapa, acento en el formato espaciado y diagnósticos del DSL.
- **Hito 4 (Completado)**: CLI con `clap` (`run`, `trace`, `compare`), `Word::to_compact` y exportación determinista a texto plano, Markdown y JSON (serializado a mano, sin `serde`).
- **Hito 5 (Completado)**: Expresividad fonológica — restricciones y mutaciones de acento (`StressMatch`, `StressUpdate`, `LiteralStress`), variables de asimilación/disimilación $\alpha$, entornos múltiples (`|`) y elementos opcionales (`(X)`), más gramáticas históricas reales en `examples/` verificadas por integración.
- **Hito 6 (Planificado)**: Reconstrucción comparativa — alineamiento determinista de cognados por distancia sobre `FeatureBundle`, extracción de tablas de correspondencias sonoras regulares y verificación de protoformas contra un `DialectTree`.

## Lenguaje DSL y tokenizador maximal munch (`src/dsl/`)

### Salida del parser (`Program`)
El parser *recursive descent* manual expone `parse_program(source: &str) -> Result<Program, DslError>` y devuelve:
- `inventory: Inventory`: construido a partir de las declaraciones `segment` (puede quedar vacío si el archivo solo declara clases y reglas/etapas).
- `classes: BTreeMap<String, NaturalClass>`: clases naturales declaradas, ordenadas de forma determinista.
- `rules: Vec<Rule>`: lista ordenada de reglas de nivel superior (vacía si el archivo define etapas `stage`).
- `dialect_tree: Option<DialectTree>`: `Some(tree)` si el archivo declara bloques `stage`, o `None` si usa reglas planas de nivel superior.

Adicionalmente, `Program::validate_against(&self, inventory: &Inventory) -> Result<(), DslError>` verifica que todos los símbolos literales usados en `class`, `Pattern::Literal`, `PatternBase::Literal`, `ReplacementItem::Literal` y `ReplacementItem::LiteralWithStress` (tanto en reglas de nivel superior como dentro de etapas `stage`) existan en un `Inventory` dado.
- **Validación automática**: si el programa declara al menos un `segment`, `parse_program` ejecuta `validate_against(&inventory)` sobre ese inventario antes de devolverlo. Si no declara ninguno (inventario externo), quien llama debe ejecutar `validate_against` con el inventario que vaya a usar.
- **Clases usadas antes de declararse**: si un símbolo desconocido de un patrón (`target` o contexto) coincide con el nombre de una `class` declarada más adelante, el error es `DslErrorKind::ClassUsedBeforeDeclaration(String)` en lugar de `UnknownSymbol`.
- `validate_against` revisa los símbolos registrados durante el parseo: las reglas que se agreguen o modifiquen después en `Program` no se revalidan.

### Léxico y comentarios
- Entrada en UTF-8 NFC. Un BOM UTF-8 (U+FEFF) al inicio de la fuente se ignora: los offsets en bytes de `Span` siguen contando desde el inicio real del texto, y la columna del primer carácter es 1.
- Comentarios de línea con `//` hasta el final de la línea (no usar `#`, que representa `ContextElement::WordBoundary`).
- Palabras reservadas de inicio de declaración: `segment`, `class`, `rule`, `stage` (`from` es palabra clave contextual en la cabecera de `stage`).
- Símbolos fonológicos e identificadores sin comillas: secuencias Unicode sin espacios que no contengan operadores ni delimitadores del DSL (`{ } [ ] ( ) , : = > / _ # * + - $ | " ;`). Se permiten `-` y `_` internos en identificadores/rasgos compuestos (`mid-high`, `mid-low`, `left-to-right`, `right-to-left`, `final_m_loss`), mientras que un `-` inicial siempre se tokeniza como signo negativo y un `_` inicial o aislado siempre se tokeniza como marcador de foco del contexto (`_`). `|` se tokeniza como `TokenKind::Pipe` para separar entornos alternativos.
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
- **Acento prosódico (Hito 5, solo en patrones, mutaciones y literales de regla)**:
  - En `segment`, cualquier especificación de acento se rechaza con `DslErrorKind::StressInSegmentDecl`.
  - En patrones (`target` y contextos): `+stressed` (`StressMatch::Stressed`), `-stressed` / `stress=unstressed` (`StressMatch::Is(Stress::Unstressed)`), `stress=primary` (`StressMatch::Is(Stress::Primary)`), `stress=secondary` (`StressMatch::Is(Stress::Secondary)`).
  - En mutaciones (`[...]` y `$k[...]`) y sufijos de literales de reemplazo (`e[...]`): `+stressed` / `stress=primary` (`StressUpdate::Set(Stress::Primary)`), `-stressed` / `stress=unstressed` (`StressUpdate::Set(Stress::Unstressed)`), `stress=secondary` (`StressUpdate::Set(Stress::Secondary)`).
- **Variables $\alpha$ de asimilación/disimilación (Hito 5, solo en reglas)**:
  - En `segment`, cualquier variable se rechaza con `DslErrorKind::VariableInSegmentDecl`.
  - **Notación griega prefija**: una letra griega minúscula (`α`..`ω`) inmediatamente pegada al nombre del campo (`αplace`, `αbackness`, `αvoiced`, `+αvoiced`, `-αvoiced`, `αstress`).
  - **Notación explícita `campo=var`**: acepta letras griegas minúsculas (`place=α`, `voiced=-α`) y nombres ASCII prefijados con `@` (`place=@p`, `voiced=-@v`, donde `@` va seguido de uno o más caracteres ASCII alfanuméricos o `_`).
  - La polaridad negativa (`-α...` / `=-α` / `=-@v`) es válida únicamente sobre campos booleanos (`voiced`, `aspirated`, `palatalized`, `labialized`, `rounded`, `syllabic`, `long`); en cualquier otro campo devuelve un diagnóstico `InvalidFeatureValue`.
- **Validaciones**: rechazar `-manner` / `manner=none`, la negación de variantes enumeradas (`-stop`, `-velar`, etc.) y campos repetidos dentro del mismo `[...]` (incluyendo concreto + variable o múltiples variables sobre el mismo campo).

### Declaración de reglas (`rule`)
```text
rule <id> ["<nombre>"] [(simultaneous | left-to-right | right-to-left)]:
  <target> > <replacement> [/ <left_context> _ <right_context> [| <left_2> _ <right_2> ...]]
```
- Tras `:`, se permiten saltos de línea y comentarios opcionales; los entornos alternativos separados por `|` también admiten saltos de línea opcionales antes o después de `|` para alinear ramas largas, mientras que cada rama permanece en una única línea lógica.
- Si se omite `"<nombre>"`, `name` toma el valor de `id`. Si se omite el modo, se usa `ApplicationMode::Simultaneous`.
- **Vacío en `target` o `replacement`**: `∅` (U+2205) o alias ASCII `0` (epéntesis $0 \to M$ y borrado $N \to 0$).
- **Patrones en `target` y contextos**:
  - Un identificador que coincide con una `class` declarada previamente se resuelve como `Pattern::Class`; en caso contrario, como `Pattern::Literal`.
  - Un bloque `[...]` se resuelve como `Pattern::Features(FeaturePattern)` (o `Pattern::Constrained` si incluye acento o variables $\alpha$).
  - Sufijo `[...]` adyacente (sin espacio) tras un identificador literal o clase (`V[+stressed]`, `ɛ[+stressed]`, `N[αplace]`): produce `Pattern::Constrained` manteniendo el literal o clase como `base` y aplicando únicamente restricciones de acento y/o variables $\alpha$. Incluir rasgos intrínsecos de `FeatureBundle` en este sufijo se rechaza con `DslErrorKind::FeaturesOnNonFeaturePattern`.
- **Reemplazos ($N \to M$)**:
  - Un símbolo literal produce `ReplacementItem::Literal` (usar el nombre de una `class` en `replacement` es error).
  - Un símbolo literal con sufijo `[...]` adyacente (sin espacio, p. ej. `e[+stressed]`, `j[-stressed]`, `e[stress=secondary]`, `e[αstress]`) produce `ReplacementItem::LiteralWithStress`. Solo admite acento o variable de dominio `Stress`; incluir rasgos intrínsecos de `FeatureBundle` se rechaza con `DslErrorKind::FeaturesOnLiteralReplacement`.
  - Mutación implícita `[...]`: permitida únicamente cuando `target.len() == 1` (`source_index: 0`); si `target.len() != 1`, produce error de ambigüedad. Admite rasgos, acento y variables $\alpha$ mediante `SegmentMutation`.
  - Referencia indexada 1-based `$1`, `$2`, ... (mapeada a `source_index = k - 1`):
    - `$k` sin corchetes equivale a `ReplacementItem::Mutate { source_index: k - 1, mutation: SegmentMutation::default() }` (copia el segmento preservando su `stress`).
    - `$k[...]` sin espacio entre `$k` y `[` aplica la `SegmentMutation` especificada al segmento `k - 1` (si hay espacio entre `$k` y `[`, se interpretan como dos ítems distintos).
    - `$0` o `$k > target.len()` producen error de diagnóstico.
- **Contexto (`/ <left_context> _ <right_context> [| ...]`)**:
  - Opcional; si aparece `/`, exige al menos una rama y exactamente un `_` en cada rama. Si una rama no tiene un `_` aislado pero algún identificador del contexto contiene `_` (p. ej. `V_V`, que se lexea como un solo identificador), el error es `DslErrorKind::FocusInsideIdentifier(String)` en lugar de `MissingContextFocus`, para sugerir separar el foco con espacios (`V _ V`).
  - Soporta `#` (`ContextElement::WordBoundary`), `<patrón>` (`ContextElement::Segment`), `<patrón>*` (`ContextElement::ZeroOrMore`) y `(<patrón>)` (`ContextElement::Optional`, exactamente un patrón dentro de los paréntesis; rechaza `(#)` y `(<patrón>)*`). Un `#` fuera del borde exterior de cualquier rama se rechaza con `RuleError::MisplacedWordBoundary`.
- **Validaciones finales**: rechazar IDs de regla duplicados en el mismo ámbito (nivel superior o misma etapa `stage`); cada regla se construye mediante `Rule::with_environments` (propagando `RuleError` envuelto en `DslError` con el `Span` de la regla).

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
- El árbol resultante se valida con `DialectTree::new`, envolviendo cualquier `DialectError` en `DslErrorKind::InvalidDialectTree(DialectError)`. El `Span` apunta a la cabecera de la etapa involucrada; para `DuplicateStageId`, a la declaración repetida (no a la primera).

### Diagnósticos (`Span` y `DslError`)
- `Span { start: usize, end: usize, line: usize, column: usize }`: offsets en bytes `[start, end)` y posición inicial 1-based en escalares Unicode (`line`, `column`).
- `DslError { kind: DslErrorKind, span: Span }`: implementa `Display` (formato `<line>:<column>: <mensaje>`) y `std::error::Error`, cubriendo errores léxicos, sintácticos, semánticos (clases duplicadas/vacías, referencias `$k` inválidas, colisiones de `Inventory`, IDs de regla duplicados, símbolos desconocidos y clases usadas antes de declararse) y `RuleError`.

### Tokenizador *maximal munch* (`tokenize_word`)
- Firma pública: `tokenize_word(input: &str, inventory: &Inventory) -> Result<Word, TokenizeError>`.
- Orden determinista de candidatos del inventario: `(Reverse(symbol.len()), symbol)` (longitud descendente en bytes y desempate lexicográfico).
- Rechaza espacios en blanco dentro de la palabra (`UnexpectedWhitespace`).
- En cada posición intenta primero el prefijo más largo registrado en `inventory`; si ninguno coincide, reconoce las marcas IPA de acento `'ˈ'` (U+02C8 → `Stress::Primary`) y `'ˌ'` (U+02CC → `Stress::Secondary`), salvo que la marca estuviera registrada explícitamente en `inventory`.
- Como en IPA la marca precede a la sílaba, el acento se asigna al **siguiente segmento silábico** (`bundle.syllabic == true`), salteando los no silábicos intermedios: `ˈvita` acentúa `i` y `tʰeˈkʰaːs` acentúa `aː`. Poner la marca justo antes de la vocal (`vˈita`) da el mismo resultado.
- Errores estructurados en `TokenizeError` (con `offset` en bytes y `column` 1-based en escalares Unicode, implementando `Display` y `std::error::Error`): `UnknownSegment`, `UnexpectedWhitespace`, `DuplicateStressMarker` (otra marca antes de llegar a un segmento silábico) y `DanglingStressMarker` (la palabra termina sin un segmento silábico después de la marca).
- La entrada vacía `""` produce `Ok(Word::new(vec![]))`.
- **Limitación conocida**: el *munch* es voraz y sin *backtracking*. Con el inventario `{a, ab, bc}`, `abc` falla en `c` aunque `a` + `bc` sería una segmentación válida.

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
// Inventario omitido (p, pʰ, t, tʰ, k, kʰ, s, h, r, n, a, aː, e, eː, i, o, u):
// la versión completa está en tests/dialect_tree.rs.
class V = { a, aː, e, eː, i, o, u }

stage pie "Proto-Indo-European" {}

stage proto_greek "Proto-Greek" from pie {
  rule grassmann (left-to-right):
    [+stop, +aspirated] > [-aspirated] / _ [-aspirated]* [+aspirated]

  rule s_debuccalization:
    s > h / # _ V
}

// El ático y el jónico comparten la elevación aː > eː; el ático la revierte tras r.
stage attic "Attic Greek" from proto_greek {
  rule a_raising:
    aː > eː

  rule alpha_purum "Attic alpha purum":
    eː > aː / r _
}

stage ionic "Ionic Greek" from proto_greek {
  rule a_raising:
    aː > eː
}
```

### Ejemplo de extensiones en DSL (Hito 5)
```text
// Diptongación sensible al acento con preservación de foco prosódico
rule open_mid_diphthongization:
  ɛ[+stressed] > j e[+stressed]

// Asimilación nasal homorgánica de punto de articulación con variable α
rule nasal_place_assimilation:
  [nasal] > [αplace] / _ [stop, αplace]

// Síncopa / debilitamiento con entornos múltiples y elemento opcional
rule unstressed_syncope:
  V[-stressed] > ∅ / C (r) _ C V | V C _ C #
```

## Interfaz CLI y formatos de exportación (`src/cli/`)

### Dependencias y arquitectura
- `clap` (4.x con la feature `derive`, compatible con MSRV 1.88) es la única dependencia externa en `Cargo.toml`.
- `src/phonology/`, `src/engine/` y `src/dsl/` siguen siendo 100 % `std`-only y nunca importan `clap`.
- `src/cli/` vive como módulo del binario (`mod cli;` en `src/main.rs`) para que la librería (`src/lib.rs`) no exponga tipos de `clap`.
- Los lints de `Cargo.toml` (`unsafe_code = "forbid"`, `unwrap_used = "deny"`, `expect_used = "deny"`, `panic = "deny"`) aplican también a `src/cli/` y `src/main.rs`.
- Submódulos en `src/cli/`:
  - `args.rs`: definición de comandos y flags con `clap::Parser` / `clap::Subcommand` / `clap::ValueEnum`.
  - `error.rs`: enum `CliError` que implementa `Display` y `std::error::Error`.
  - `format.rs`: formateadores puros para `text`, `markdown` y JSON RFC 8259 determinista escrito a mano (sin `serde` ni `serde_json`).
  - `mod.rs`: lectura de entradas, despacho de subcomandos y escritura de salida.

### Subcomandos y argumentos

#### Opciones y entradas comunes (`run`, `trace`, `compare`)
- `<RULES>`: ruta al archivo DSL en UTF-8 NFC (o `-` para leer el archivo DSL desde `stdin`). Debe declarar al menos un `segment`; si el inventario resultante está vacío, devuelve `CliError::EmptyInventory`.
- `[WORDS]...`: palabras posicionales a derivar.
- `-w, --words <PATH>`: archivo de texto UTF-8 con una palabra por línea (se elimina un eventual BOM UTF-8 `U+FEFF` inicial, se recortan espacios iniciales/finales y se ignoran líneas vacías y comentarios que comienzan con `//` o `#` tras recortar espacios). Es mutuamente excluyente con `[WORDS]...` (`clap` `conflicts_with`).
- **Entrada por `stdin`**: si no se pasan ni `[WORDS]...` ni `--words`, la CLI lee las palabras desde `stdin` aplicando las mismas reglas de filtrado por línea (salvo que `<RULES>` sea `-`, en cuyo caso devuelve `CliError::StdinConflict`).
- `--spaced`: parsea cada palabra con `Word::from_spaced` y formatea todas las palabras de salida (`input`, `output`, `before`, `after`, `outcomes`) con `Word::to_spaced`. Sin `--spaced`, parsea con `tokenize_word` y formatea con `Word::to_compact`.
- `-f, --format <text|markdown|json>` (por defecto: `text`): formato de exportación.
- `-o, --output <PATH>`: escribe el resultado UTF-8 al archivo indicado en lugar de `stdout`.

#### `phonengine run <RULES> [--stage <STAGE_ID>] [--spaced] [-f <FORMAT>] [-o <PATH>] [-w <PATH> | WORDS...]`
- Ejecuta las reglas sobre cada palabra y muestra la forma final (en `json` incluye además `rules_fired`, el arreglo ordenado de `rule_id` disparados).
- Si `<RULES>` define un `DialectTree` (bloques `stage`), `--stage <STAGE_ID>` es obligatorio (`CliError::MissingStage`); deriva la ruta raíz → `<STAGE_ID>` mediante `DialectTree::derive_stage`.
- Si `<RULES>` define reglas planas de nivel superior y se pasa `--stage`, devuelve `CliError::UnexpectedStage`.

#### `phonengine trace <RULES> [--stage <STAGE_ID>] [--spaced] [-f <FORMAT>] [-o <PATH>] [-w <PATH> | WORDS...]`
- Misma validación de `--stage` que `run` (`MissingStage` / `UnexpectedStage`).
- Muestra los pasos efectivos (`word_after != word_before`, omitiendo emparejamientos vacuos). Cuando el programa define bloques `stage`, agrupa los pasos por etapa cronológica (`StageTrace`) a lo largo de la ruta raíz → `<STAGE_ID>`.

#### `phonengine compare <RULES> [--stages <ID1,ID2,...>] [--spaced] [-f <FORMAT>] [-o <PATH>] [-w <PATH> | WORDS...]`
- Exige que `<RULES>` defina bloques `stage` (`CliError::CompareRequiresDialectTree` si solo hay reglas planas).
- `--stages` acepta nombres separados por comas y puede repetirse (`--stages attic,ionic` o `--stages attic --stages ionic`). Rechaza duplicados con `CliError::DuplicateStageSelection(String)` y etapas inexistentes en el árbol (incluido un identificador vacío como `--stages ""`) con `CliError::UnknownStage(String)`.
- Si se omite `--stages`, selecciona todas las etapas hoja del árbol (`TreeDerivation::leaf_outcomes()`) en orden lexicográfico determinista por `stage_id`.
- Evalúa cada palabra una única vez con `DialectTree::derive_tree` y proyecta los resultados en el orden exacto de las etapas seleccionadas.

### Formatos de salida (`text`, `markdown`, `json`)

#### 1. `text`
- **`run`**: una línea por palabra `<input> -> <output>` (si no hay palabras, salida vacía).
- **`trace` (reglas planas)**: para cada palabra (separadas por una línea en blanco si hay más de una):
  ```text
  <input> -> <output>
    [<rule_id>] <rule_name>: <before> -> <after>
  ```
  Si ningún paso cambió la palabra, imprime `  (no changes)` debajo de `<input> -> <output>`.
- **`trace` (con `--stage`)**: para cada palabra:
  ```text
  <input> -> <output> (stage: <target_stage>)
    stage <stage_id> (<stage_name>): <stage_input> -> <stage_output>
      [<rule_id>] <rule_name>: <before> -> <after>
  ```
  Si una etapa no tuvo cambios efectivos, imprime `    (no changes)` debajo de su cabecera.
- **`compare`**: tabla de texto alineada por columnas con cabecera `Input` seguida de cada `<stage_id>` seleccionado y un separador de guiones ajustado al ancho de cada columna (mínimo 3 guiones `---`), sin espacios finales sobrantes por línea aunque la última celda esté vacía (o salida vacía si no hay palabras).

#### 2. `markdown`
Las celdas de tabla escapan `|` como `\|`.
- **`run`**: tabla GFM `| Input | Output |` seguida de `| --- | --- |` y una fila por palabra.
- **`trace` (reglas planas)**: por cada palabra:
  - Encabezado `## <input> -> <output>`
  - Si hay pasos: tabla `| Rule ID | Rule | Before | After |`; si no hay pasos: `_No changes._`.
- **`trace` (con `--stage`)**: por cada palabra:
  - Encabezado `## <input> -> <output> (stage: <target_stage>)`
  - Por cada etapa en la ruta: encabezado `### <stage_id> — <stage_name> (<stage_input> -> <stage_output>)`, seguido de la tabla `| Rule ID | Rule | Before | After |` o `_No changes._`.
- **`compare`**: tabla GFM `| Input | <stage_1> | <stage_2> | ... |` con una fila por palabra.

#### 3. `json`
JSON válido conforme a RFC 8259 generado a mano de forma determinista (indentación de 2 espacios, salto de línea final `\n` y escape completo de caracteres de control, comillas `\"` y barras `\\`). El orden de claves de cada objeto es fijo:
- **`run`**:
  ```json
  {
    "command": "run",
    "stage": null,
    "results": [
      {
        "input": "vitam",
        "output": "vida",
        "rules_fired": ["R1", "R3"]
      }
    ]
  }
  ```
  (`"stage"` toma `null` para reglas planas o el `"<stage_id>"` indicado cuando se usa `--stage`).
- **`trace` (reglas planas)**:
  ```json
  {
    "command": "trace",
    "stage": null,
    "results": [
      {
        "input": "vitam",
        "output": "vida",
        "steps": [
          {
            "rule_id": "R1",
            "rule_name": "Final -m deletion",
            "before": "vitam",
            "after": "vita"
          }
        ]
      }
    ]
  }
  ```
- **`trace` (con `--stage`)**:
  ```json
  {
    "command": "trace",
    "stage": "attic",
    "results": [
      {
        "input": "tʰeːs",
        "output": "tʰeːs",
        "stages": [
          {
            "stage_id": "pie",
            "stage_name": "Proto-Indo-European",
            "input": "tʰeːs",
            "output": "tʰeːs",
            "steps": []
          }
        ]
      }
    ]
  }
  ```
- **`compare`**:
  ```json
  {
    "command": "compare",
    "stages": ["attic", "ionic"],
    "results": [
      {
        "input": "maːtʰeːs",
        "outcomes": {
          "attic": "meːtʰeːs",
          "ionic": "meːtʰeːs"
        }
      }
    ]
  }
  ```
  (Las claves de `"outcomes"` respetan el mismo orden que el arreglo `"stages"`).

### Errores (`CliError`) y códigos de salida
- **`0`**: ejecución exitosa (incluye el caso válido de 0 palabras de entrada).
- **`2`**: error de sintaxis o conflicto de argumentos de línea de comandos (manejado por `clap`).
- **`1`**: error de ejecución (`CliError`), impreso en `stderr` con el prefijo `error: <mensaje>`:
  - `Io { path: String, source: std::io::Error }`: fallo de lectura/escritura indicando la ruta del archivo o `"stdin"`/`"stdout"`.
  - `Dsl(DslError)`: error de léxico, sintaxis o validación en el archivo de reglas.
  - `EmptyInventory`: el archivo DSL no declara ninguna sentencia `segment`.
  - `Tokenize { word: String, source: TokenizeError }`: error al tokenizar una palabra sin `--spaced`.
  - `SpacedParse { word: String, source: InventoryError }`: símbolo desconocido al parsear con `--spaced`.
  - `Engine { word: String, source: EngineError }`: fallo durante la evaluación de reglas o derivación en árbol.
  - `MissingStage`: el archivo define bloques `stage` pero `run`/`trace` se invocó sin `--stage`.
  - `UnexpectedStage(String)`: se pasó `--stage` en `run`/`trace` sobre un archivo de reglas planas.
  - `CompareRequiresDialectTree`: `compare` se invocó sobre un archivo sin bloques `stage`.
  - `DuplicateStageSelection(String)`: etapa repetida en `--stages`.
  - `UnknownStage(String)`: etapa solicitada en `--stage` o `--stages` inexistente en el árbol.
  - `StdinConflict`: `<RULES>` es `-` y no se especificaron palabras por argumentos ni por `--words`.
