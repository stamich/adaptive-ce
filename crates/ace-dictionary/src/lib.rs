//! Dictionary abstractions introduced in ACE 0.2.
//!
//! Milestone 0.2 intentionally provides registry, identity and validation infrastructure only.
//! Training, adaptive reuse and GraphNet/AdaptiveDB semantic dictionaries are deferred.

use ace_core::{AceResult, DictionaryId, DictionaryScope};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Immutable dictionary bytes identified by a stable ACE dictionary identifier.
#[derive(Debug, Clone)]
pub struct Dictionary {
    /// Stable identifier referenced by physical block plans.
    pub id: DictionaryId,
    /// Lifetime and reuse scope of the dictionary.
    pub scope: DictionaryScope,
    /// Raw dictionary bytes available to a compatible codec implementation.
    pub bytes: Arc<[u8]>,
}

impl Dictionary {
    /// Creates an immutable dictionary from owned bytes.
    pub fn new(id: DictionaryId, scope: DictionaryScope, bytes: Vec<u8>) -> Self {
        Self {
            id,
            scope,
            bytes: Arc::<[u8]>::from(bytes),
        }
    }

    /// Returns the dictionary size in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns true when the dictionary contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Resolves dictionaries for encoders and decoders without coupling ACE to a storage backend.
pub trait DictionaryProvider: Send + Sync {
    /// Returns the dictionary with the requested identifier, if available.
    fn get(&self, id: DictionaryId) -> AceResult<Option<Arc<Dictionary>>>;
}

/// Deterministic in-memory dictionary registry keyed by stable identifier.
#[derive(Debug, Clone, Default)]
pub struct DictionaryRegistry {
    dictionaries: BTreeMap<DictionaryId, Arc<Dictionary>>,
}

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

impl DictionaryProvider for DictionaryRegistry {
    /// Resolves a dictionary by identifier using deterministic `BTreeMap` lookup.
    fn get(&self, id: DictionaryId) -> AceResult<Option<Arc<Dictionary>>> {
        Ok(self.dictionaries.get(&id).cloned())
    }
}
