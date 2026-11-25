//! Rule engine: rules, contexts, error types, execution, derivation traces, and dialect trees.

pub mod applier;
pub mod dialect;
pub mod error;
pub mod rule;
pub mod trace;

pub use applier::{apply_rule, derive};
pub use dialect::{DialectNode, DialectTrace, DialectTree, StageTrace, TreeDerivation};
pub use error::{DialectError, EngineError, RuleError};
pub use rule::{ApplicationMode, ContextElement, NaturalClass, Pattern, ReplacementItem, Rule};
pub use trace::{DerivationStep, DerivationTrace};
