use thiserror::Error;

/// Unified error type returned by ACE 0.4 components.
#[derive(Debug, Error)]
pub enum AceError {
    /// ACE file magic does not match the expected signature.
    #[error("invalid ACE magic")]
    InvalidMagic,
    /// The file format version is not supported.
    #[error("unsupported ACE format version {major}.{minor}")]
    UnsupportedVersion {
        /// Major version found in the header.
        major: u8,
        /// Minor version found in the header.
        minor: u8,
    },
    /// A required format feature is not supported by this decoder.
    #[error("unsupported required feature flag 0x{0:04x}")]
    UnsupportedFeature(u16),
    /// A codec identifier is unknown.
    #[error("unsupported codec id {0}")]
    UnsupportedCodec(u8),
    /// An entropy codec identifier is unknown.
    #[error("unsupported entropy codec id {0}")]
    UnsupportedEntropyCodec(u8),
    /// A transform identifier is unknown.
    #[error("unsupported transform id {0}")]
    UnsupportedTransform(u8),
    /// A dictionary identifier referenced by a block cannot be resolved.
    #[error("missing dictionary {0}")]
    MissingDictionary(u64),
    /// A serialized stream is structurally invalid.
    #[error("malformed stream: {0}")]
    Malformed(&'static str),
    /// A block checksum does not match the reconstructed bytes.
    #[error("checksum mismatch for block {0}")]
    ChecksumMismatch(u64),
    /// The serialized block index or trailer checksum is invalid.
    #[error("block index checksum mismatch")]
    IndexChecksumMismatch,
    /// Decoding would exceed a configured hard resource limit.
    #[error("resource limit exceeded: {0}")]
    ResourceLimitExceeded(&'static str),
    /// An LZ reference cannot be satisfied by already reconstructed bytes.
    #[error("invalid LZ reference")]
    InvalidLzReference,
    /// Huffman metadata or payload is invalid.
    #[error("invalid Huffman stream: {0}")]
    InvalidHuffman(&'static str),
    /// rANS metadata or payload is invalid.
    #[error("invalid rANS stream: {0}")]
    InvalidRans(&'static str),
    /// A requested configuration value is not valid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(&'static str),
    /// A requested block identifier is not present in the index.
    #[error("block {0} was not found")]
    BlockNotFound(u64),
    /// Underlying I/O operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Convenience result type used throughout ACE.
pub type AceResult<T> = Result<T, AceError>;
