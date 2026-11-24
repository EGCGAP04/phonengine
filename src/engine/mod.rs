//! Rule engine: rules, contexts, error types, execution, and derivation traces.

pub mod applier;
pub mod error;
pub mod rule;
pub mod trace;

pub use applier::{apply_rule, derive};
pub use error::{EngineError, RuleError};
pub use rule::{ApplicationMode, ContextElement, NaturalClass, Pattern, ReplacementItem, Rule};
pub use trace::{DerivationStep, DerivationTrace};
