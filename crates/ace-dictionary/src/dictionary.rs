//! Immutable dictionary value type.

use std::sync::Arc;

use ace_core::{DictionaryId, DictionaryScope};

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

/// Inherent methods of [`Dictionary`].
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
