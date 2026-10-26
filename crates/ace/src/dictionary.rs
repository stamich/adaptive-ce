//! Preset dictionaries.

use std::sync::Arc;

use ace_core::{DictionaryId, DictionaryScope, EncoderDictionary, MAX_DICTIONARY_BYTES};
use ace_dictionary::{dictionary_id_of, train_dictionary, TrainerConfig};

use crate::{Error, ErrorKind, Result};

/// A preset dictionary: bytes that the compressor treats as history before every block, so the
/// start of a block can already be matched against them.
///
/// A dictionary pays off for many small, similar inputs (records, messages, log lines). Train
/// one from samples with [`Dictionary::train`], hand it to the configuration
/// ([`ConfigBuilder::dictionary`](crate::ConfigBuilder::dictionary) embeds it in the file,
/// [`ConfigBuilder::external_dictionary`](crate::ConfigBuilder::external_dictionary) stores only
/// its id) and give external ones to readers with [`Engine::with_dictionaries`](crate::Engine::with_dictionaries).
/// A block uses the dictionary only when that makes it smaller, so a dictionary that does not fit
/// the data costs time, never space.
///
/// The [`id`](Self::id) is derived from the content, so two dictionaries with the same bytes are
/// the same dictionary. Files that use a dictionary are Format 1.6 and need ACE 0.8.0 or later.
///
/// ```
/// use ace::{Config, Dictionary, Engine};
///
/// let samples: Vec<Vec<u8>> = (0..200)
///     .map(|i| format!("{{\"unit\":\"celsius\",\"device\":{i},\"firmware\":\"release-2.7.1\"}}\n").into_bytes())
///     .collect();
/// let dictionary = Dictionary::train(&samples, 512)?;
///
/// let config = Config::builder().block_size(4096).dictionary(dictionary).build()?;
/// let engine = Engine::new(config)?;
/// let input: Vec<u8> = (500..1500)
///     .flat_map(|i| format!("{{\"unit\":\"celsius\",\"device\":{i},\"firmware\":\"release-2.7.1\"}}\n").into_bytes())
///     .collect();
/// let packed = engine.compress(&input)?;
/// assert_eq!(Engine::default().decompress(&packed)?, input, "embedded: self-contained");
/// # Ok::<(), ace::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dictionary {
    /// Identifier written into the blocks that use the dictionary.
    id: DictionaryId,
    /// The content.
    bytes: Arc<[u8]>,
}

impl Dictionary {
    /// A dictionary with the given content; its id is derived from the bytes.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`] when `bytes` is empty or longer than 65 535 bytes (the LZ
    /// window; more could never be matched).
    pub fn new(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let bytes = bytes.into();
        let id = dictionary_id_of(&bytes);
        Self::with_id(id.0, bytes)
    }

    /// A dictionary with an id you manage yourself, for example a version number in your own
    /// dictionary store. The id must be non-zero and must name these bytes only: a reader that
    /// finds other bytes under it fails the block checksum instead of returning wrong data.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`] for a zero id or the sizes [`new`](Self::new) rejects.
    pub fn with_id(id: u64, bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let bytes = bytes.into();
        if id == 0 {
            return Err(Error::new(
                ErrorKind::InvalidConfig,
                "a dictionary id must be non-zero",
            ));
        }
        if bytes.is_empty() || bytes.len() > MAX_DICTIONARY_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidConfig,
                "dictionary size must be 1 to 65535 bytes",
            ));
        }
        Ok(Self {
            id: DictionaryId(id),
            bytes: Arc::from(bytes),
        })
    }

    /// Trains a dictionary of at most `size` bytes from `samples`.
    ///
    /// The samples should be typical inputs of one data source (100 to 1 000 of them are usually
    /// enough). The result depends only on the samples, in order, and `size`: the same samples
    /// give the same dictionary on every machine. At most 8 MiB of samples are read.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`] for a `size` outside `64 ..= 65 535`, fewer than two
    /// samples, or samples that share no content.
    pub fn train<S: AsRef<[u8]>>(samples: &[S], size: usize) -> Result<Self> {
        let config = TrainerConfig {
            dictionary_size: size,
            segment_len: 64.min(size),
            ..TrainerConfig::default()
        };
        if size < 64 {
            return Err(Error::new(
                ErrorKind::InvalidConfig,
                "a trained dictionary needs at least 64 bytes",
            ));
        }
        let borrowed: Vec<&[u8]> = samples.iter().map(AsRef::as_ref).collect();
        let bytes = train_dictionary(&borrowed, &config).map_err(Error::from_core)?;
        Self::new(bytes)
    }

    /// The identifier written into blocks that use this dictionary.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id.0
    }

    /// The content.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Size of the content in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Always `false`: a dictionary has at least one byte.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The encoder-side form with the given scope.
    pub(crate) fn to_core(&self, scope: DictionaryScope) -> EncoderDictionary {
        EncoderDictionary {
            id: self.id,
            scope,
            bytes: Arc::clone(&self.bytes),
        }
    }

    /// The facade form of an encoder dictionary.
    pub(crate) fn from_core(dictionary: &EncoderDictionary) -> Self {
        Self {
            id: dictionary.id,
            bytes: Arc::clone(&dictionary.bytes),
        }
    }
}
