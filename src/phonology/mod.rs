//! Phonological representation: features, patterns/mutations, segments, words, and inventory.

pub mod features;
pub mod inventory;
pub mod pattern;
pub mod segment;

pub use features::{Backness, FeatureBundle, Height, Manner, Place, Stress};
pub use inventory::{Inventory, InventoryError};
pub use pattern::{FeatureMutation, FeaturePattern, FieldMatch, FieldUpdate};
pub use segment::{Segment, Word};
