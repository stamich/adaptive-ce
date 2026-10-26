/// Hard decoder limits used before any attacker-controlled allocation is performed.
///
/// Every size, count and length read from a file is compared with these limits *before* the
/// buffer it describes is allocated (`docs/DECODER-AUDIT.md`). Which reader applies which
/// limit:
///
/// | Limit | Sequential (`decompress*`, streaming) | Indexed (`AceIndexedDecoder`) |
/// |---|---|---|
/// | [`max_output_size`](Self::max_output_size) | file header + running total | file header + range length |
/// | [`max_block_size`](Self::max_block_size) | every block header and decoder | every block header and decoder |
/// | [`max_encoded_block_size`](Self::max_encoded_block_size) | every block header | every block header |
/// | [`max_transforms`](Self::max_transforms) | every block header | every block header |
/// | [`max_index_entries`](Self::max_index_entries) | block count of the file header ([`max_block_count`](Self::max_block_count)) | index entry count, index byte size, block count |
/// | [`max_dictionary_size`](Self::max_dictionary_size) | dictionary section of the file header (Format 1.6), each dictionary | dictionary section |
/// | [`max_dictionaries`](Self::max_dictionaries) | dictionary section | dictionary section |
/// | [`max_segments`](Self::max_segments) | segment table of every `SEG1` block, before the table is read | same |
/// | [`max_columns`](Self::max_columns) | column table of every `COL1` block, before the table is read | same |
///
/// # Invariants
///
/// A limit only ever rejects input; it never changes decoded bytes. Raising a limit cannot make
/// a valid file decode differently.
#[derive(Debug, Clone)]
pub struct DecodeLimits {
    /// Maximum total reconstructed output accepted by the decoder.
    pub max_output_size: u64,
    /// Maximum reconstructed size of one block.
    pub max_block_size: usize,
    /// Maximum encoded metadata plus payload size of one block.
    pub max_encoded_block_size: usize,
    /// Maximum number of transforms attached to one block.
    pub max_transforms: usize,
    /// Maximum size of one dictionary accepted from the dictionary section of a file or from an
    /// external registry. The format itself never allows more than 65 535 bytes (the LZ window),
    /// so the smaller of the two applies.
    pub max_dictionary_size: usize,
    /// Maximum number of dictionaries in the dictionary section of one file (Format 1.6).
    pub max_dictionaries: usize,
    /// Maximum number of segments of one segmented block (Format 1.6).
    pub max_segments: usize,
    /// Maximum number of columns of one columnar block (Format 1.6).
    pub max_columns: usize,
    /// Maximum number of serialized block-index entries; also the maximum number of blocks
    /// any reader accepts (see [`Self::max_block_count`]).
    pub max_index_entries: usize,
}

/// Derived limits of [`DecodeLimits`].
impl DecodeLimits {
    /// Maximum number of blocks a file may declare, enforced by every reader before the first
    /// block is read.
    ///
    /// It equals [`Self::max_index_entries`], so the sequential and the indexed reader accept
    /// exactly the same files. (A separate field would be a breaking change of this public
    /// struct; it is planned with the 0.6 API facade.)
    pub fn max_block_count(&self) -> u64 {
        u64::try_from(self.max_index_entries).unwrap_or(u64::MAX)
    }
}

/// Provides the documented default values of [`DecodeLimits`].
impl Default for DecodeLimits {
    /// Returns conservative general-purpose limits suitable for files up to 64 GiB.
    fn default() -> Self {
        Self {
            max_output_size: 64 * 1024 * 1024 * 1024,
            max_block_size: 4 * 1024 * 1024,
            max_encoded_block_size: 8 * 1024 * 1024,
            max_transforms: 8,
            max_dictionary_size: 16 * 1024 * 1024,
            max_dictionaries: 16,
            max_segments: 1024,
            max_columns: 256,
            max_index_entries: 16_000_000,
        }
    }
}
