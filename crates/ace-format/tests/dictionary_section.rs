//! The dictionary section (`ADIC`) of Format 1.6: round trip, hostile input, and the way the
//! writer and the sequential reader carry it between the file header and the first block.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{
    AceError, CodecId, DecodeLimits, DictionaryId, EntropyCodecId, FORMAT_MINOR_STORAGE,
};
use ace_format::{
    checksum, encode_dictionary_section, read_dictionary_section, AceReader, AceWriter,
    BlockHeader, EmbeddedDictionary, FileHeader, DICTIONARY_SECTION_MAGIC,
    FILE_FLAG_HAS_DICTIONARIES, FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE, MAX_DICTIONARY_BYTES,
};

/// A dictionary of `len` pseudo-random bytes.
fn dictionary(id: u64, len: usize) -> EmbeddedDictionary {
    let mut state = id.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    EmbeddedDictionary {
        id: DictionaryId(id),
        bytes: (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect(),
    }
}

/// Parses `bytes` with the default limits.
fn parse(bytes: &[u8]) -> Result<Vec<EmbeddedDictionary>, AceError> {
    read_dictionary_section(&mut Cursor::new(bytes), &DecodeLimits::default())
}

#[test]
fn section_round_trips_and_reports_its_size() {
    let dictionaries = vec![
        dictionary(3, 1),
        dictionary(10, 4096),
        dictionary(11, MAX_DICTIONARY_BYTES),
    ];
    let bytes = encode_dictionary_section(&dictionaries).unwrap();
    assert_eq!(&bytes[..4], &DICTIONARY_SECTION_MAGIC);
    // magic + count + entries + section CRC
    assert_eq!(
        bytes.len(),
        8 + 3 * 16 + (1 + 4096 + MAX_DICTIONARY_BYTES) + 4
    );
    let mut cursor = Cursor::new(bytes.as_slice());
    assert_eq!(
        read_dictionary_section(&mut cursor, &DecodeLimits::default()).unwrap(),
        dictionaries
    );
    assert_eq!(
        cursor.position() as usize,
        bytes.len(),
        "the section is consumed exactly"
    );
}

#[test]
fn writer_rejects_invalid_lists() {
    assert!(encode_dictionary_section(&[]).is_err());
    assert!(encode_dictionary_section(&[dictionary(5, 8), dictionary(5, 8)]).is_err());
    assert!(encode_dictionary_section(&[dictionary(6, 8), dictionary(5, 8)]).is_err());
    assert!(encode_dictionary_section(&[dictionary(1, 0)]).is_err());
    assert!(encode_dictionary_section(&[dictionary(1, MAX_DICTIONARY_BYTES + 1)]).is_err());
}

/// Flipping any single byte of a section is detected (per-dictionary CRC, section CRC, or a
/// structural check) and never produces a different dictionary list.
#[test]
fn every_single_byte_flip_is_rejected() {
    let dictionaries = vec![dictionary(1, 100), dictionary(2, 200)];
    let bytes = encode_dictionary_section(&dictionaries).unwrap();
    for index in 0..bytes.len() {
        let mut forged = bytes.clone();
        forged[index] ^= 0x40;
        assert!(parse(&forged).is_err(), "flip at {index} was accepted");
    }
}

#[test]
fn truncation_is_an_error_at_every_length() {
    let bytes = encode_dictionary_section(&[dictionary(1, 64), dictionary(2, 64)]).unwrap();
    for len in 0..bytes.len() {
        assert!(
            parse(&bytes[..len]).is_err(),
            "prefix of {len} bytes was accepted"
        );
    }
}

/// A forged length or count fails with a limit error *before* the stream is asked for that many
/// bytes: the stream below has only a few bytes, so reading the claimed length would be an I/O
/// error instead.
#[test]
fn forged_sizes_fail_before_they_are_read() {
    let mut forged = DICTIONARY_SECTION_MAGIC.to_vec();
    forged.extend_from_slice(&1u32.to_le_bytes());
    forged.extend_from_slice(&7u64.to_le_bytes());
    forged.extend_from_slice(&u32::MAX.to_le_bytes());
    forged.extend_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        parse(&forged),
        Err(AceError::ResourceLimitExceeded("dictionary size"))
    ));
    // 65 536 exceeds the format cap even though the default limit is 16 MiB.
    let mut over_cap = forged.clone();
    over_cap[16..20].copy_from_slice(&65_536u32.to_le_bytes());
    assert!(matches!(
        parse(&over_cap),
        Err(AceError::ResourceLimitExceeded("dictionary size"))
    ));
    // A caller limit below the format cap applies.
    let tight = DecodeLimits {
        max_dictionary_size: 100,
        ..DecodeLimits::default()
    };
    let bytes = encode_dictionary_section(&[dictionary(1, 101)]).unwrap();
    assert!(matches!(
        read_dictionary_section(&mut Cursor::new(bytes.as_slice()), &tight),
        Err(AceError::ResourceLimitExceeded("dictionary size"))
    ));
    // Count above the limit, and a count of zero.
    let mut many = DICTIONARY_SECTION_MAGIC.to_vec();
    many.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        parse(&many),
        Err(AceError::ResourceLimitExceeded("dictionary count"))
    ));
    let mut none = DICTIONARY_SECTION_MAGIC.to_vec();
    none.extend_from_slice(&0u32.to_le_bytes());
    assert!(matches!(parse(&none), Err(AceError::Malformed(_))));
}

#[test]
fn magic_ordering_and_empty_entries_are_malformed() {
    let good = encode_dictionary_section(&[dictionary(1, 16), dictionary(2, 16)]).unwrap();
    let mut wrong_magic = good.clone();
    wrong_magic[0] = b'X';
    assert!(matches!(parse(&wrong_magic), Err(AceError::Malformed(_))));
    // Swap the two ids (second entry starts after 8 + 16 + 16 bytes) and fix the section CRC so
    // that only the ordering rule can reject it.
    let mut unordered = good.clone();
    unordered[8..16].copy_from_slice(&9u64.to_le_bytes());
    let end = unordered.len() - 4;
    let crc = checksum(&unordered[..end]);
    unordered[end..].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(parse(&unordered), Err(AceError::Malformed(_))));
}

/// The writer puts the section after the file header, the blocks' file offsets (and with them the
/// index) include it, and the sequential reader returns the dictionaries from `read_file_header`.
#[test]
fn writer_and_reader_carry_the_section_before_the_first_block() {
    let dictionaries = vec![dictionary(4, 300), dictionary(8, 77)];
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: FORMAT_MINOR_STORAGE,
            flags: FILE_FLAG_HAS_INDEX | FILE_FLAG_HAS_DICTIONARIES,
            default_block_size: 4096,
            original_size: 5,
            block_count: 1,
        })
        .unwrap();
    writer.write_dictionary_section(&dictionaries).unwrap();
    let section_len = encode_dictionary_section(&dictionaries).unwrap().len();
    assert_eq!(
        writer.bytes_written() as usize,
        FILE_HEADER_SIZE + section_len
    );
    let header = BlockHeader {
        block_id: 0,
        original_size: 5,
        encoded_size: 5,
        metadata_size: 0,
        codec: CodecId::Raw,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(b"hello"),
    };
    writer.write_block(&header, &[], b"hello").unwrap();
    let (bytes, _) = writer.finish(true).unwrap();

    let mut reader = AceReader::new(Cursor::new(bytes), DecodeLimits::default());
    assert!(reader.dictionaries().is_empty());
    let file_header = reader.read_file_header().unwrap();
    assert_eq!(
        file_header.flags & FILE_FLAG_HAS_DICTIONARIES,
        FILE_FLAG_HAS_DICTIONARIES
    );
    assert_eq!(reader.dictionaries(), dictionaries.as_slice());
    let (block, _, payload) = reader.read_block().unwrap();
    assert_eq!(
        (block.block_id, payload.as_slice()),
        (0, b"hello".as_slice())
    );
}

/// Without the flag nothing is read between the header and the first block.
#[test]
fn files_without_the_flag_have_no_section() {
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: 3,
            flags: 0,
            default_block_size: 4096,
            original_size: 0,
            block_count: 0,
        })
        .unwrap();
    let (bytes, _) = writer.finish(false).unwrap();
    let mut reader = AceReader::new(Cursor::new(bytes), DecodeLimits::default());
    reader.read_file_header().unwrap();
    assert!(reader.dictionaries().is_empty());
}
