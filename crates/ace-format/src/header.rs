// Decoder hardening (ACE 0.5.1): arithmetic on values read from a file is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`); a proven-safe site carries a local
// `allow` with its proof.
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_core::{
    AceError, AceResult, CodecId, DecodingPlan, DictionaryId, DictionaryRef, DictionaryScope,
    EntropyCodecId, TransformId,
};

/// Four-byte ACE format-v1 magic preserved from milestone 0.1.
pub const MAGIC: [u8; 4] = *b"ACE1";
/// Current major format version (defined in `ace-core` so that errors can name it).
pub use ace_core::FORMAT_MAJOR;
/// Highest minor format version this crate reads and writes (1.4 adds the TS1 codec).
pub use ace_core::FORMAT_MINOR;
/// Minor version written for files that use no Format 1.4 feature, so they stay readable by
/// ACE 0.4.x and byte-identical to its output.
pub const FORMAT_MINOR_BASE: u8 = 3;
/// Serialized file-header size in bytes.
pub const FILE_HEADER_SIZE: usize = 32;
/// Fixed serialized block-header size before variable descriptors and metadata.
pub const BLOCK_HEADER_SIZE: usize = 32;
/// Serialized size of a dictionary descriptor (8-byte identifier + 1-byte scope).
const DICTIONARY_DESCRIPTOR_SIZE: usize = 9;
/// File flag indicating that a serialized block index and trailer are present.
pub const FILE_FLAG_HAS_INDEX: u16 = 0x0001;
/// File flag (Format 1.6): the dictionary section (`ADIC`) follows the file header and blocks may
/// reference its dictionaries (scope `File`).
pub const FILE_FLAG_HAS_DICTIONARIES: u16 = 0x0002;
/// Set of format-1.1…1.4 flags understood by ACE.
pub const SUPPORTED_FILE_FLAGS: u16 = FILE_FLAG_HAS_INDEX | FILE_FLAG_HAS_DICTIONARIES;
/// Block flag indicating that a nine-byte dictionary descriptor follows transform descriptors.
pub const BLOCK_FLAG_HAS_DICTIONARY: u8 = 0x01;

/// Fixed ACE file header shared by formats 1.0 – 1.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    /// Minor format version: read from disk, or chosen by the writer (`FORMAT_MINOR_BASE` up to
    /// [`FORMAT_MINOR`], see [`minimal_minor_version`]).
    pub minor_version: u8,
    /// File-level feature flags.
    pub flags: u16,
    /// Default block size used by the encoder.
    pub default_block_size: u32,
    /// Total reconstructed file size.
    pub original_size: u64,
    /// Number of independent ACE blocks.
    pub block_count: u64,
}

/// Fixed and variable decoder metadata for one ACE block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    /// Stable zero-based block identifier.
    pub block_id: u64,
    /// Reconstructed block byte count.
    pub original_size: u32,
    /// Entropy-coded payload byte count.
    pub encoded_size: u32,
    /// Entropy metadata byte count, excluding transform/dictionary descriptors.
    pub metadata_size: u32,
    /// Primary structural codec.
    pub codec: CodecId,
    /// Final entropy codec.
    pub entropy: EntropyCodecId,
    /// Ordered forward transforms.
    pub transforms: Vec<TransformId>,
    /// Optional dictionary reference introduced by format 1.2.
    pub dictionary: Option<DictionaryRef>,
    /// Raw block feature flags.
    pub flags: u8,
    /// CRC32C of reconstructed uncompressed block bytes.
    pub payload_crc32c: u32,
}

/// Minor version a writer must declare for a file whose blocks each need the minor version
/// listed in `block_minors` (the *minimal-version* rule).
///
/// A file stays Format 1.3 unless a block needs more: a TS1 block needs 1.4, a NUM1 or TS1
/// payload of mode 4 needs 1.5 (see `ace_engine::block_format_minor`). Files that need nothing
/// newer are byte-identical to ACE 0.4.x output and readable by 0.4.x decoders. The result is
/// at least [`FORMAT_MINOR_BASE`] and at most [`FORMAT_MINOR`].
pub fn minimal_minor_version(block_minors: impl IntoIterator<Item = u8>) -> u8 {
    block_minors
        .into_iter()
        .fold(FORMAT_MINOR_BASE, u8::max)
        .min(FORMAT_MINOR)
}

/// Serializes a file header, including a CRC32C over its first 28 bytes.
///
/// The minor version byte is `header.minor_version` capped at [`FORMAT_MINOR`].
pub fn encode_file_header(header: &FileHeader) -> [u8; FILE_HEADER_SIZE] {
    let mut bytes = [0u8; FILE_HEADER_SIZE];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4] = FORMAT_MAJOR;
    bytes[5] = header.minor_version.min(FORMAT_MINOR);
    bytes[6..8].copy_from_slice(&header.flags.to_le_bytes());
    bytes[8..12].copy_from_slice(&header.default_block_size.to_le_bytes());
    bytes[12..20].copy_from_slice(&header.original_size.to_le_bytes());
    bytes[20..28].copy_from_slice(&header.block_count.to_le_bytes());
    let crc = crate::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// Parses and validates a format 1.0 – 1.6 file header.
///
/// # Errors
///
/// [`AceError::InvalidMagic`], [`AceError::UnsupportedVersion`],
/// [`AceError::UnsupportedFeature`] or [`AceError::Malformed`] (wrong length, CRC or field
/// value).
pub fn decode_file_header(bytes: &[u8]) -> AceResult<FileHeader> {
    if bytes.len() != FILE_HEADER_SIZE {
        return Err(AceError::Malformed("truncated file header"));
    }
    if bytes[0..4] != MAGIC {
        return Err(AceError::InvalidMagic);
    }
    if bytes[4] != FORMAT_MAJOR || bytes[5] > FORMAT_MINOR {
        return Err(AceError::UnsupportedVersion {
            major: bytes[4],
            minor: bytes[5],
        });
    }
    let stored = u32::from_le_bytes(
        bytes[28..32]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid file header CRC"))?,
    );
    if crate::checksum(&bytes[..28]) != stored {
        return Err(AceError::Malformed("file header checksum mismatch"));
    }
    let flags = u16::from_le_bytes(
        bytes[6..8]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid file flags"))?,
    );
    if bytes[5] >= 1 && flags & !SUPPORTED_FILE_FLAGS != 0 {
        return Err(AceError::UnsupportedFeature(flags & !SUPPORTED_FILE_FLAGS));
    }
    // The flag announces the dictionary section (`ADIC`) that follows the header; the section
    // exists only in Format 1.6, so an older version with the flag set is not a file we know.
    if flags & FILE_FLAG_HAS_DICTIONARIES != 0 && bytes[5] < ace_core::FORMAT_MINOR_STORAGE {
        return Err(AceError::UnsupportedFeature(FILE_FLAG_HAS_DICTIONARIES));
    }
    Ok(FileHeader {
        minor_version: bytes[5],
        flags,
        default_block_size: u32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block size"))?,
        ),
        original_size: u64::from_le_bytes(
            bytes[12..20]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid original size"))?,
        ),
        block_count: u64::from_le_bytes(
            bytes[20..28]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block count"))?,
        ),
    })
}

/// Returns the variable descriptor byte count implied by a fixed block header.
pub fn block_descriptor_size(fixed: &[u8; BLOCK_HEADER_SIZE], minor_version: u8) -> usize {
    let transforms = usize::from(fixed[22]);
    let dictionary = if minor_version >= 1 && fixed[23] & BLOCK_FLAG_HAS_DICTIONARY != 0 {
        DICTIONARY_DESCRIPTOR_SIZE
    } else {
        0
    };
    // At most 255 + 9 bytes: cannot overflow.
    transforms.saturating_add(dictionary)
}

/// Serializes one block header and its variable transform/dictionary descriptors.
///
/// # Invariants
///
/// `header.transforms.len() <= 255`; [`crate::AceWriter::write_block`] rejects longer lists
/// before calling this function, so the count byte never truncates.
#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)] // encoder: see Invariants
pub fn encode_block_header(header: &BlockHeader) -> Vec<u8> {
    let dictionary_bytes = if header.dictionary.is_some() {
        DICTIONARY_DESCRIPTOR_SIZE
    } else {
        0
    };
    let mut bytes = vec![0u8; BLOCK_HEADER_SIZE + header.transforms.len() + dictionary_bytes];
    bytes[0..8].copy_from_slice(&header.block_id.to_le_bytes());
    bytes[8..12].copy_from_slice(&header.original_size.to_le_bytes());
    bytes[12..16].copy_from_slice(&header.encoded_size.to_le_bytes());
    bytes[16..20].copy_from_slice(&header.metadata_size.to_le_bytes());
    bytes[20] = header.codec as u8;
    bytes[21] = header.entropy as u8;
    bytes[22] = header.transforms.len() as u8;
    bytes[23] = if header.dictionary.is_some() {
        header.flags | BLOCK_FLAG_HAS_DICTIONARY
    } else {
        header.flags & !BLOCK_FLAG_HAS_DICTIONARY
    };
    bytes[24..28].copy_from_slice(&header.payload_crc32c.to_le_bytes());
    for (i, transform) in header.transforms.iter().enumerate() {
        bytes[BLOCK_HEADER_SIZE + i] = *transform as u8;
    }
    if let Some(dictionary) = header.dictionary {
        let offset = BLOCK_HEADER_SIZE + header.transforms.len();
        bytes[offset..offset + 8].copy_from_slice(&dictionary.id.0.to_le_bytes());
        bytes[offset + 8] = scope_to_u8(dictionary.scope);
    }
    let crc = crate::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// Parses one fixed block header and its caller-supplied variable descriptors.
///
/// # Errors
///
/// [`AceError::Malformed`] for a wrong length, CRC, size or flag combination; unsupported
/// codec, entropy-codec or transform identifiers; features newer than `minor_version` (for
/// example TS1 below Format 1.4).
pub fn decode_block_header(
    fixed: &[u8],
    descriptors: &[u8],
    minor_version: u8,
) -> AceResult<BlockHeader> {
    if fixed.len() != BLOCK_HEADER_SIZE {
        return Err(AceError::Malformed("truncated block header"));
    }
    let stored = u32::from_le_bytes(
        fixed[28..32]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid block header CRC"))?,
    );
    if crate::checksum(&fixed[..28]) != stored {
        return Err(AceError::Malformed("block header checksum mismatch"));
    }
    let dictionary_present = minor_version >= 1 && fixed[23] & BLOCK_FLAG_HAS_DICTIONARY != 0;
    // The caller sized `descriptors` with `block_descriptor_size`; anything else is malformed.
    let fixed_array: &[u8; BLOCK_HEADER_SIZE] = fixed
        .try_into()
        .map_err(|_| AceError::Malformed("truncated block header"))?;
    if descriptors.len() != block_descriptor_size(fixed_array, minor_version) {
        return Err(AceError::Malformed("block descriptor count mismatch"));
    }
    let (transform_bytes, dictionary_bytes) = descriptors
        .split_at_checked(usize::from(fixed[22]))
        .ok_or(AceError::Malformed("block descriptor count mismatch"))?;
    let transforms = transform_bytes
        .iter()
        .map(|&v| TransformId::try_from(v))
        .collect::<AceResult<Vec<_>>>()?;
    let dictionary = if dictionary_present {
        let (id, scope) = dictionary_bytes
            .split_first_chunk::<8>()
            .ok_or(AceError::Malformed("invalid dictionary id"))?;
        let scope = scope
            .first()
            .ok_or(AceError::Malformed("invalid dictionary scope"))?;
        Some(DictionaryRef {
            id: DictionaryId(u64::from_le_bytes(*id)),
            scope: scope_from_u8(*scope)?,
        })
    } else {
        None
    };
    let entropy = EntropyCodecId::try_from(fixed[21])?;
    if minor_version < 2 && matches!(entropy, EntropyCodecId::Rans4x) {
        return Err(AceError::UnsupportedVersion {
            major: FORMAT_MAJOR,
            minor: minor_version,
        });
    }
    let codec = CodecId::try_from(fixed[20])?;
    if minor_version < 3 && matches!(codec, CodecId::Numeric) {
        return Err(AceError::UnsupportedVersion {
            major: FORMAT_MAJOR,
            minor: minor_version,
        });
    }
    // TS1 blocks only exist in Format 1.4 files (the minimal-version writer guarantees it).
    if minor_version < 4 && matches!(codec, CodecId::TimeSeries) {
        return Err(AceError::UnsupportedVersion {
            major: FORMAT_MAJOR,
            minor: minor_version,
        });
    }
    // Segmented and columnar blocks exist only in Format 1.6 files, and (like TS1) are complete
    // payloads: transforms, an entropy stage or a dictionary on the outer block are rejected.
    if codec.is_container() {
        if minor_version < ace_core::FORMAT_MINOR_STORAGE {
            return Err(AceError::UnsupportedVersion {
                major: FORMAT_MAJOR,
                minor: minor_version,
            });
        }
        if !transforms.is_empty()
            || !matches!(entropy, EntropyCodecId::None)
            || dictionary.is_some()
        {
            return Err(AceError::Malformed(
                "SEG1/COL1 block with transforms, entropy stage or dictionary",
            ));
        }
    }
    // A TS1 payload is a complete bitstream: Format 1.4 defines it without transforms, entropy
    // stage or dictionary, so any of them is rejected rather than given ad-hoc semantics.
    if matches!(codec, CodecId::TimeSeries)
        && (!transforms.is_empty()
            || !matches!(entropy, EntropyCodecId::None)
            || dictionary.is_some())
    {
        return Err(AceError::Malformed(
            "TS1 block with transforms, entropy stage or dictionary",
        ));
    }
    Ok(BlockHeader {
        block_id: u64::from_le_bytes(
            fixed[0..8]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block id"))?,
        ),
        original_size: u32::from_le_bytes(
            fixed[8..12]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block original size"))?,
        ),
        encoded_size: u32::from_le_bytes(
            fixed[12..16]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block encoded size"))?,
        ),
        metadata_size: u32::from_le_bytes(
            fixed[16..20]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid block metadata size"))?,
        ),
        codec,
        entropy,
        transforms,
        dictionary,
        flags: fixed[23],
        payload_crc32c: u32::from_le_bytes(
            fixed[24..28]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid payload CRC"))?,
        ),
    })
}

/// Inherent methods of [`BlockHeader`].
impl BlockHeader {
    /// Returns the decoder-relevant physical plan represented by this block header.
    pub fn decoding_plan(&self) -> DecodingPlan {
        DecodingPlan {
            transforms: self.transforms.clone(),
            codec: self.codec,
            dictionary: self.dictionary,
            entropy: self.entropy,
        }
    }
}

/// Converts a dictionary scope into its stable format-1.1 byte representation.
fn scope_to_u8(scope: DictionaryScope) -> u8 {
    match scope {
        DictionaryScope::Block => 0,
        DictionaryScope::Segment => 1,
        DictionaryScope::File => 2,
        DictionaryScope::External => 3,
    }
}

/// Parses a dictionary scope byte from format-1.1 metadata.
fn scope_from_u8(value: u8) -> AceResult<DictionaryScope> {
    match value {
        0 => Ok(DictionaryScope::Block),
        1 => Ok(DictionaryScope::Segment),
        2 => Ok(DictionaryScope::File),
        3 => Ok(DictionaryScope::External),
        _ => Err(AceError::Malformed("unknown dictionary scope")),
    }
}
