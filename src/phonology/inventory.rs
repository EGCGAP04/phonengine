//! Bijective phonological segment inventory.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use super::features::{FeatureBundle, Stress};
use super::segment::Segment;

/// Errors returned while constructing or querying a phonological [`Inventory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    /// Symbol is empty or contains whitespace.
    InvalidSymbol(String),
    /// Symbol is already registered in the inventory.
    DuplicateSymbol(String),
    /// Feature bundle is already mapped to another symbol, violating bijectivity.
    DuplicateBundle {
        /// Symbol that already owns this feature bundle.
        existing_symbol: String,
        /// New symbol whose insertion was rejected.
        new_symbol: String,
        /// Colliding feature bundle.
        bundle: FeatureBundle,
    },
    /// Requested symbol does not exist in the inventory.
    UnknownSymbol(String),
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSymbol(sym) => {
                write!(
                    f,
                    "invalid inventory symbol {sym:?}: must be non-empty without whitespace"
                )
            }
            Self::DuplicateSymbol(sym) => {
                write!(f, "duplicate inventory symbol {sym:?}")
            }
            Self::DuplicateBundle {
                existing_symbol,
                new_symbol,
                bundle,
            } => write!(
                f,
                "cannot map symbol {new_symbol:?} to {bundle}: already mapped to {existing_symbol:?}"
            ),
            Self::UnknownSymbol(sym) => write!(f, "unknown inventory symbol {sym:?}"),
        }
    }
}

impl Error for InventoryError {}

/// Deterministic bidirectional mapping between segment symbols and [`FeatureBundle`]s.
///
/// Guarantees strict bijectivity: each symbol maps to a unique [`FeatureBundle`] and each
/// [`FeatureBundle`] maps back to a unique symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inventory {
    by_symbol: BTreeMap<String, FeatureBundle>,
    by_bundle: BTreeMap<FeatureBundle, String>,
}

impl Inventory {
    /// Creates an empty phonological inventory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a `symbol` <-> `bundle` pair in both directions.
    ///
    /// # Errors
    ///
    /// - [`InventoryError::InvalidSymbol`] if `symbol` is empty or contains whitespace.
    /// - [`InventoryError::DuplicateSymbol`] if `symbol` is already registered.
    /// - [`InventoryError::DuplicateBundle`] if `bundle` is already mapped to another symbol.
    pub fn insert(
        &mut self,
        symbol: impl Into<String>,
        bundle: FeatureBundle,
    ) -> Result<(), InventoryError> {
        let symbol = symbol.into();
        if symbol.is_empty() || symbol.chars().any(char::is_whitespace) {
            return Err(InventoryError::InvalidSymbol(symbol));
        }
        if self.by_symbol.contains_key(&symbol) {
            return Err(InventoryError::DuplicateSymbol(symbol));
        }
        if let Some(existing_symbol) = self.by_bundle.get(&bundle) {
            return Err(InventoryError::DuplicateBundle {
                existing_symbol: existing_symbol.clone(),
                new_symbol: symbol,
                bundle,
            });
        }
        self.by_symbol.insert(symbol.clone(), bundle);
        self.by_bundle.insert(bundle, symbol);
        Ok(())
    }

    /// Looks up the [`FeatureBundle`] for `symbol`.
    #[must_use]
    pub fn get(&self, symbol: &str) -> Option<&FeatureBundle> {
        self.by_symbol.get(symbol)
    }

    /// Resolves a [`FeatureBundle`] back to its canonical symbol.
    #[must_use]
    pub fn resolve(&self, bundle: &FeatureBundle) -> Option<&str> {
        self.by_bundle.get(bundle).map(String::as_str)
    }

    /// Builds a [`Segment`] for `symbol` with the given [`Stress`].
    ///
    /// # Errors
    ///
    /// Returns [`InventoryError::UnknownSymbol`] if `symbol` is not registered.
    pub fn segment(&self, symbol: &str, stress: Stress) -> Result<Segment, InventoryError> {
        let bundle = self
            .get(symbol)
            .copied()
            .ok_or_else(|| InventoryError::UnknownSymbol(symbol.to_owned()))?;
        Ok(Segment::new(symbol, bundle, stress))
    }

    /// Returns the number of registered segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_symbol.len()
    }

    /// Returns `true` if no segments are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_symbol.is_empty()
    }

    /// Iterates over all registered `(symbol, bundle)` pairs in deterministic symbol order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &FeatureBundle)> {
        self.by_symbol.iter().map(|(s, b)| (s.as_str(), b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonology::features::{Backness, Height, Manner, Place};

    #[test]
    fn bijective_insert_and_bidirectional_lookup() -> Result<(), InventoryError> {
        let mut inv = Inventory::new();
        assert!(inv.is_empty());

        let p = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false);
        let b = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, true);
        let a = FeatureBundle::vowel(Height::Low, Backness::Central, false);

        inv.insert("p", p)?;
        inv.insert("b", b)?;
        inv.insert("a", a)?;

        assert_eq!(inv.len(), 3);
        assert_eq!(inv.get("p"), Some(&p));
        assert_eq!(inv.resolve(&p), Some("p"));
        assert_eq!(inv.resolve(&b), Some("b"));
        assert_eq!(inv.resolve(&a), Some("a"));

        let symbols: Vec<&str> = inv.iter().map(|(s, _)| s).collect();
        assert_eq!(symbols, vec!["a", "b", "p"]);
        Ok(())
    }

    #[test]
    fn rejects_duplicate_symbol_without_mutating_inventory() -> Result<(), InventoryError> {
        let mut inv = Inventory::new();
        let p = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false);
        let t = FeatureBundle::consonant(Manner::Stop, Place::Alveolar, false);

        inv.insert("p", p)?;
        let err = inv.insert("p", t);
        assert_eq!(err, Err(InventoryError::DuplicateSymbol("p".to_owned())));
        assert_eq!(inv.len(), 1);
        assert_eq!(inv.get("p"), Some(&p));
        assert_eq!(inv.resolve(&t), None);
        Ok(())
    }

    #[test]
    fn rejects_duplicate_bundle_to_preserve_bijectivity() -> Result<(), InventoryError> {
        let mut inv = Inventory::new();
        let stop = FeatureBundle::consonant(Manner::Stop, Place::Bilabial, false);

        inv.insert("p", stop)?;
        let err = inv.insert("p_alt", stop);
        assert_eq!(
            err,
            Err(InventoryError::DuplicateBundle {
                existing_symbol: "p".to_owned(),
                new_symbol: "p_alt".to_owned(),
                bundle: stop,
            })
        );
        assert_eq!(inv.len(), 1);
        assert_eq!(inv.get("p_alt"), None);
        assert_eq!(inv.resolve(&stop), Some("p"));
        Ok(())
    }

    #[test]
    fn rejects_empty_or_whitespace_symbols_and_formats_errors() {
        let mut inv = Inventory::new();
        let a = FeatureBundle::vowel(Height::Low, Backness::Central, false);

        let empty_err = inv.insert("", a);
        assert_eq!(empty_err, Err(InventoryError::InvalidSymbol(String::new())));
        if let Err(err) = empty_err {
            assert!(!err.to_string().is_empty());
        }

        let space_err = inv.insert("a b", a);
        assert_eq!(
            space_err,
            Err(InventoryError::InvalidSymbol("a b".to_owned()))
        );

        let unknown_err = inv.segment("z", Stress::Unstressed);
        assert_eq!(
            unknown_err,
            Err(InventoryError::UnknownSymbol("z".to_owned()))
        );
    }
}
