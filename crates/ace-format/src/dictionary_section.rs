//! The dictionary section (`ADIC`) of a Format 1.6 file.
//!
//! A file whose header has the `HAS_DICTIONARIES` flag carries the dictionaries that its blocks
//! name with scope `File` right after the 32-byte file header, before the first block, so a
//! reader has them before it meets a block that needs one. The layout is in
//! `docs/FORMAT-1.6.md`:
//!
//! ```text
//! "ADIC" · count u32 · count × (id u64 · length u32 · crc32c u32 · bytes) · crc32c u32
//! ```
//!
//! The section CRC covers every byte from the magic to the last dictionary. Ids are strictly
//! increasing, every dictionary is `1 ..= 65 535` bytes (the LZ window; a longer dictionary could
//! never be referenced) and not longer than [`DecodeLimits::max_dictionary_size`].

// Decoder hardening: arithmetic on values read from a file is checked and every narrowing cast
// is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use std::io::Read;

use ace_core::{AceError, AceResult, DecodeLimits, DictionaryId};

use crate::checksum;

/// Magic bytes that open the dictionary section.
pub const DICTIONARY_SECTION_MAGIC: [u8; 4] = *b"ADIC";

/// Largest dictionary the format allows: the LZ window. Bytes before the last 65 535 of a longer
/// dictionary could never be matched. (Defined in `ace-core`, re-exported here.)
pub use ace_core::MAX_DICTIONARY_BYTES;

/// Size of the fixed part of a dictionary entry: id (8), length (4), CRC-32C (4).
const ENTRY_HEADER_SIZE: usize = 16;

/// A dictionary stored in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedDictionary {
    /// Identity that blocks reference in their dictionary descriptor.
    pub id: DictionaryId,
    /// The dictionary bytes (LZ history; `1 ..= MAX_DICTIONARY_BYTES`).
    pub bytes: Vec<u8>,
}

/// Serializes a dictionary section.
///
/// # Errors
/// [`AceError::InvalidConfig`] when `dictionaries` is empty, the ids are not strictly increasing,
/// or a dictionary is empty or longer than [`MAX_DICTIONARY_BYTES`].
pub fn encode_dictionary_section(dictionaries: &[EmbeddedDictionary]) -> AceResult<Vec<u8>> {
    if dictionaries.is_empty() {
        return Err(AceError::InvalidConfig("empty dictionary section"));
    }
    let count = u32::try_from(dictionaries.len())
        .map_err(|_| AceError::InvalidConfig("too many dictionaries"))?;
    let mut out = Vec::new();
    out.extend_from_slice(&DICTIONARY_SECTION_MAGIC);
    out.extend_from_slice(&count.to_le_bytes());
    let mut previous: Option<DictionaryId> = None;
    for dictionary in dictionaries {
        if previous.is_some_and(|last| dictionary.id <= last) {
            return Err(AceError::InvalidConfig(
                "dictionary ids must be strictly increasing",
            ));
        }
        previous = Some(dictionary.id);
        if dictionary.bytes.is_empty() || dictionary.bytes.len() > MAX_DICTIONARY_BYTES {
            return Err(AceError::InvalidConfig(
                "dictionary size must be 1 to 65535 bytes",
            ));
        }
        // `len <= 65_535` was checked above, so the conversion cannot fail.
        let length = u32::try_from(dictionary.bytes.len())
            .map_err(|_| AceError::InvalidConfig("dictionary size must be 1 to 65535 bytes"))?;
        out.extend_from_slice(&dictionary.id.0.to_le_bytes());
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&checksum(&dictionary.bytes).to_le_bytes());
        out.extend_from_slice(&dictionary.bytes);
    }
    let crc = checksum(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}

/// Reads and validates a dictionary section from `reader`.
///
/// Every count and length is compared with `limits` before the buffer it describes is allocated,
/// and a dictionary is checked against its own CRC-32C and the whole section against the section
/// CRC-32C.
///
/// # Errors
/// [`AceError::Malformed`] for a wrong magic, an empty or unordered entry or a checksum
/// mismatch; [`AceError::ResourceLimitExceeded`] when the count or a length exceeds the limits;
/// [`AceError::Io`] when the stream ends early.
pub fn read_dictionary_section<R: Read>(
    reader: &mut R,
    limits: &DecodeLimits,
) -> AceResult<Vec<EmbeddedDictionary>> {
    let mut head = [0u8; 8];
    reader.read_exact(&mut head)?;
    let (magic, count) = head.split_at(4);
    if magic != DICTIONARY_SECTION_MAGIC {
        return Err(AceError::Malformed("missing dictionary section"));
    }
    let count = usize::try_from(read_u32(count)?)
        .map_err(|_| AceError::ResourceLimitExceeded("dictionary count"))?;
    if count == 0 {
        return Err(AceError::Malformed("empty dictionary section"));
    }
    if count > limits.max_dictionaries {
        return Err(AceError::ResourceLimitExceeded("dictionary count"));
    }
    let size_limit = limits.max_dictionary_size.min(MAX_DICTIONARY_BYTES);
    let mut covered = head.to_vec();
    let mut dictionaries = Vec::with_capacity(count);
    for _ in 0..count {
        let mut entry = [0u8; ENTRY_HEADER_SIZE];
        reader.read_exact(&mut entry)?;
        covered.extend_from_slice(&entry);
        let (id, rest) = entry.split_at(8);
        let (length, crc) = rest.split_at(4);
        let id = DictionaryId(read_u64(id)?);
        let length = usize::try_from(read_u32(length)?)
            .map_err(|_| AceError::ResourceLimitExceeded("dictionary size"))?;
        let crc = read_u32(crc)?;
        if length == 0 {
            return Err(AceError::Malformed("empty dictionary"));
        }
        if length > size_limit {
            return Err(AceError::ResourceLimitExceeded("dictionary size"));
        }
        if dictionaries
            .last()
            .is_some_and(|last: &EmbeddedDictionary| id <= last.id)
        {
            return Err(AceError::Malformed(
                "dictionary ids are not strictly increasing",
            ));
        }
        let mut bytes = vec![0u8; length];
        reader.read_exact(&mut bytes)?;
        if checksum(&bytes) != crc {
            return Err(AceError::Malformed("dictionary checksum mismatch"));
        }
        covered.extend_from_slice(&bytes);
        dictionaries.push(EmbeddedDictionary { id, bytes });
    }
    let mut section_crc = [0u8; 4];
    reader.read_exact(&mut section_crc)?;
    if checksum(&covered) != u32::from_le_bytes(section_crc) {
        return Err(AceError::Malformed("dictionary section checksum mismatch"));
    }
    Ok(dictionaries)
}

/// Little-endian `u32` from exactly four bytes.
fn read_u32(bytes: &[u8]) -> AceResult<u32> {
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        AceError::Malformed("truncated dictionary field")
    })?))
}

/// Little-endian `u64` from exactly eight bytes.
fn read_u64(bytes: &[u8]) -> AceResult<u64> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| {
        AceError::Malformed("truncated dictionary field")
    })?))
}
