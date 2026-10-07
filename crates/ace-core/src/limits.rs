/// Hard decoder limits used before any attacker-controlled allocation is performed.
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
    /// Maximum embedded dictionary size accepted by the parser.
    pub max_dictionary_size: usize,
    /// Maximum number of serialized block-index entries.
    pub max_index_entries: usize,
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
            max_index_entries: 16_000_000,
        }
    }
}
