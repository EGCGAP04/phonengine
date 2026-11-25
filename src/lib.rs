//! PhonEngine: deterministic phonological evolution and comparative reconstruction engine.

pub mod dsl;
pub mod engine;
pub mod phonology;

pub use dsl::{
    DslError, DslErrorKind, Program, Span, TokenizeError, TokenizeErrorKind, parse_program,
    tokenize_word,
};
pub use engine::{
    ApplicationMode, ContextElement, DerivationStep, DerivationTrace, DialectError, DialectNode,
    DialectTrace, DialectTree, EngineError, NaturalClass, Pattern, ReplacementItem, Rule,
    RuleError, StageTrace, TreeDerivation, apply_rule, derive,
};
pub use phonology::{
    Backness, FeatureBundle, FeatureMutation, FeaturePattern, FieldMatch, FieldUpdate, Height,
    Inventory, InventoryError, Manner, Place, Segment, Stress, Word,
};
