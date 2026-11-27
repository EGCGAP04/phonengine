//! PhonEngine: deterministic phonological evolution and comparative reconstruction engine.

pub mod dsl;
pub mod engine;
pub mod phonology;

pub use dsl::{
    DslError, DslErrorKind, Program, Span, TokenizeError, TokenizeErrorKind, parse_program,
    tokenize_word,
};
pub use engine::{
    ApplicationMode, ConstrainedPattern, ContextElement, DerivationStep, DerivationTrace,
    DialectError, DialectNode, DialectTrace, DialectTree, EngineError, Environment, NaturalClass,
    Pattern, PatternBase, ReplacementItem, Rule, RuleError, SegmentMutation, StageTrace,
    TreeDerivation, apply_rule, derive,
};
pub use phonology::{
    Backness, FeatureBundle, FeatureDomain, FeatureField, FeatureMutation, FeaturePattern,
    FeatureVar, FeatureVarValue, FieldMatch, FieldUpdate, Height, Inventory, InventoryError,
    LiteralStress, Manner, Place, Segment, Stress, StressMatch, StressUpdate, VarBindings,
    VarPolarity, Word,
};
