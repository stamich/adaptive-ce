//! Resolution of the dictionaries a block refers to (Format 1.6).
//!
//! A block names its dictionary by id and scope. `File` dictionaries come from the dictionary
//! section of the file being read, `External` ones from the registry of the decoder (or from the
//! dictionary of the engine's own configuration, so an engine can read back what it wrote).
//! Every reader funnels through [`DictionaryView::resolve`], so the rules exist once.

use std::sync::Arc;

use ace_core::{
    AceError, AceResult, DecodeLimits, DictionaryRef, DictionaryScope, EncoderDictionary,
    MAX_DICTIONARY_BYTES,
};
use ace_dictionary::{Dictionary, DictionaryProvider, DictionaryRegistry};
use ace_format::EmbeddedDictionary;

/// The dictionaries available while one file is decoded.
#[derive(Clone, Copy)]
pub(crate) struct DictionaryView<'a> {
    /// The dictionary section of the file (scope `File`).
    embedded: &'a [EmbeddedDictionary],
    /// The decoder's registry (scope `External`).
    external: &'a DictionaryRegistry,
    /// The dictionary of the engine configuration, also resolved as `External`.
    configured: Option<&'a EncoderDictionary>,
}

/// The bytes of a resolved dictionary.
pub(crate) enum ResolvedDictionary<'a> {
    /// Borrowed from the file or from the configuration.
    Borrowed(&'a [u8]),
    /// Shared with the registry.
    Shared(Arc<Dictionary>),
}

/// Inherent methods of [`ResolvedDictionary`].
impl ResolvedDictionary<'_> {
    /// The dictionary content.
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Shared(dictionary) => &dictionary.bytes,
        }
    }
}

/// Inherent methods of [`DictionaryView`].
impl<'a> DictionaryView<'a> {
    /// A view over the dictionaries of one file, a registry and an optional configured one.
    pub(crate) fn new(
        embedded: &'a [EmbeddedDictionary],
        external: &'a DictionaryRegistry,
        configured: Option<&'a EncoderDictionary>,
    ) -> Self {
        Self {
            embedded,
            external,
            configured,
        }
    }

    /// Finds the dictionary `reference` names.
    ///
    /// # Errors
    ///
    /// [`AceError::MissingDictionary`] when no dictionary with that id exists in the scope,
    /// [`AceError::Malformed`] for the reserved scopes (`Block`, `Segment`) and
    /// [`AceError::ResourceLimitExceeded`] for an external dictionary above
    /// `limits.max_dictionary_size` or the format cap.
    pub(crate) fn resolve(
        &self,
        reference: DictionaryRef,
        limits: &DecodeLimits,
    ) -> AceResult<ResolvedDictionary<'a>> {
        let id = reference.id;
        let resolved = match reference.scope {
            DictionaryScope::File => self
                .embedded
                .iter()
                .find(|dictionary| dictionary.id == id)
                .map(|dictionary| ResolvedDictionary::Borrowed(&dictionary.bytes)),
            DictionaryScope::External => match self.external.get(id)? {
                Some(dictionary) => Some(ResolvedDictionary::Shared(dictionary)),
                None => self
                    .configured
                    .filter(|dictionary| {
                        dictionary.id == id && dictionary.scope == DictionaryScope::External
                    })
                    .map(|dictionary| ResolvedDictionary::Borrowed(&dictionary.bytes)),
            },
            DictionaryScope::Block | DictionaryScope::Segment => {
                return Err(AceError::Malformed("reserved dictionary scope"))
            }
        };
        let resolved = resolved.ok_or(AceError::MissingDictionary(id.0))?;
        if resolved.as_slice().len() > limits.max_dictionary_size.min(MAX_DICTIONARY_BYTES) {
            return Err(AceError::ResourceLimitExceeded("dictionary size"));
        }
        Ok(resolved)
    }
}
