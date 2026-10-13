//! Dictionary lookup abstraction (dependency-inversion seam for codecs and storage backends).

use std::sync::Arc;

use ace_core::{AceResult, DictionaryId};

use crate::Dictionary;

/// Resolves dictionaries for encoders and decoders without coupling ACE to a storage backend.
pub trait DictionaryProvider: Send + Sync {
    /// Returns the dictionary with the requested identifier, if available.
    ///
    /// # Errors
    ///
    /// Implementation-defined (for example a storage failure); an unknown identifier is
    /// `Ok(None)`, not an error.
    fn get(&self, id: DictionaryId) -> AceResult<Option<Arc<Dictionary>>>;
}
