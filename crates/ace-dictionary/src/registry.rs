//! Deterministic in-memory [`DictionaryProvider`] implementation.

use std::collections::BTreeMap;
use std::sync::Arc;

use ace_core::{AceResult, DictionaryId};

use crate::{Dictionary, DictionaryProvider};

/// Deterministic in-memory dictionary registry keyed by stable identifier.
#[derive(Debug, Clone, Default)]
pub struct DictionaryRegistry {
    /// Registered dictionaries ordered by identifier (`BTreeMap` keeps iteration deterministic).
    dictionaries: BTreeMap<DictionaryId, Arc<Dictionary>>,
}

/// Inherent methods of [`DictionaryRegistry`].
impl DictionaryRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces a dictionary with the same stable identifier.
    pub fn insert(&mut self, dictionary: Dictionary) -> Option<Arc<Dictionary>> {
        self.dictionaries
            .insert(dictionary.id, Arc::new(dictionary))
    }

    /// Removes a dictionary and returns its immutable handle when present.
    pub fn remove(&mut self, id: DictionaryId) -> Option<Arc<Dictionary>> {
        self.dictionaries.remove(&id)
    }

    /// Returns the number of registered dictionaries.
    pub fn len(&self) -> usize {
        self.dictionaries.len()
    }

    /// Returns true when no dictionaries are registered.
    pub fn is_empty(&self) -> bool {
        self.dictionaries.is_empty()
    }
}

/// [`DictionaryProvider`] implementation for [`DictionaryRegistry`].
impl DictionaryProvider for DictionaryRegistry {
    /// Resolves a dictionary by identifier using deterministic `BTreeMap` lookup.
    fn get(&self, id: DictionaryId) -> AceResult<Option<Arc<Dictionary>>> {
        Ok(self.dictionaries.get(&id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_core::DictionaryScope;

    /// Insert, lookup, replace and remove behave like a map.
    #[test]
    fn registry_lifecycle() {
        let mut registry = DictionaryRegistry::new();
        assert!(registry.is_empty());
        let id = DictionaryId(7);
        assert!(registry
            .insert(Dictionary::new(id, DictionaryScope::Block, vec![1, 2, 3]))
            .is_none());
        assert_eq!(registry.get(id).unwrap().unwrap().len(), 3);
        assert!(registry
            .insert(Dictionary::new(id, DictionaryScope::Block, vec![9]))
            .is_some());
        assert_eq!(registry.len(), 1);
        assert!(registry.remove(id).is_some());
        assert!(registry.get(id).unwrap().is_none());
    }
}
