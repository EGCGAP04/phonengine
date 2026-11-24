//! PhonEngine: deterministic phonological evolution and comparative reconstruction engine.

pub mod engine;
pub mod phonology;

pub use engine::{
    ApplicationMode, ContextElement, DerivationStep, DerivationTrace, EngineError, NaturalClass,
    Pattern, ReplacementItem, Rule, RuleError, apply_rule, derive,
};
pub use phonology::{
    Backness, FeatureBundle, FeatureMutation, FeaturePattern, FieldMatch, FieldUpdate, Height,
    Inventory, InventoryError, Manner, Place, Segment, Stress, Word,
};
