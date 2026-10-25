use std::sync::Arc;

use crate::{AceError, AceResult};

/// Largest dictionary the format can reference: the LZ window (`docs/FORMAT-1.6.md`).
pub const MAX_DICTIONARY_BYTES: usize = 65_535;

/// Stable dictionary identifier stored in ACE format metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct DictionaryId(pub u64);

/// Lifetime and reuse scope of one compression dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DictionaryScope {
    /// Dictionary is useful only for one logical block.
    Block,
    /// Dictionary is shared by a logical segment of blocks.
    Segment,
    /// Dictionary belongs to the current ACE file.
    File,
    /// Dictionary is managed outside the ACE file and resolved by identifier.
    External,
}

/// Decoder-facing dictionary reference embedded into one block plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DictionaryRef {
    /// Stable identifier of the dictionary.
    pub id: DictionaryId,
    /// Scope in which the dictionary is expected to be resolved.
    pub scope: DictionaryScope,
}

/// A dictionary the encoder may use, with how blocks refer to it (ACE 0.8, Format 1.6).
///
/// The bytes are LZ preset history: a block compressed with the dictionary can match against
/// them as if they preceded the block. With scope [`DictionaryScope::File`] the engine embeds the
/// dictionary in the file (the file is then self-contained); with [`DictionaryScope::External`]
/// the file stores only the id and the reader must supply the same bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderDictionary {
    /// Identifier written into every block that uses the dictionary.
    pub id: DictionaryId,
    /// [`DictionaryScope::File`] (embedded) or [`DictionaryScope::External`] (by reference).
    pub scope: DictionaryScope,
    /// The dictionary content, `1 ..= MAX_DICTIONARY_BYTES` bytes.
    pub bytes: Arc<[u8]>,
}

/// Inherent methods of [`EncoderDictionary`].
impl EncoderDictionary {
    /// Checks the scope and the size.
    ///
    /// # Errors
    ///
    /// [`AceError::InvalidConfig`] for a scope other than `File` or `External` (the others are
    /// reserved), for an empty dictionary and for one longer than [`MAX_DICTIONARY_BYTES`].
    pub fn validate(&self) -> AceResult<()> {
        if !matches!(self.scope, DictionaryScope::File | DictionaryScope::External) {
            return Err(AceError::InvalidConfig(
                "a dictionary scope must be File or External",
            ));
        }
        if self.bytes.is_empty() || self.bytes.len() > MAX_DICTIONARY_BYTES {
            return Err(AceError::InvalidConfig(
                "dictionary size must be 1 to 65535 bytes",
            ));
        }
        Ok(())
    }
}
