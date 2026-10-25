//! Format 1.6: the container codecs `Segmented` (5) and `Columnar` (6) exist only in 1.6 files and
//! only as plain outer blocks; the dictionary file flag needs 1.6.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{
    AceError, CodecId, DictionaryId, DictionaryRef, DictionaryScope, EntropyCodecId, TransformId,
};
use ace_format::{
    decode_block_header, decode_file_header, encode_block_header, encode_file_header, BlockHeader,
    FileHeader, BLOCK_HEADER_SIZE, FILE_FLAG_HAS_DICTIONARIES, FILE_FLAG_HAS_INDEX, FORMAT_MINOR,
};

/// A plain outer block of `codec`.
fn plain(codec: CodecId) -> BlockHeader {
    BlockHeader {
        block_id: 1,
        original_size: 262_144,
        encoded_size: 4096,
        metadata_size: 0,
        codec,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: 7,
    }
}

/// Parses `header` as a block of a file that declares `minor`.
fn parse(header: &BlockHeader, minor: u8) -> Result<BlockHeader, AceError> {
    let bytes = encode_block_header(header);
    let (fixed, descriptors) = bytes.split_at(BLOCK_HEADER_SIZE);
    decode_block_header(fixed, descriptors, minor)
}

/// The container codecs are known, round-trip through their serialized id and are flagged as
/// containers; the older codecs are not.
#[test]
fn container_codecs_have_stable_ids() {
    assert_eq!(CodecId::Segmented as u8, 5);
    assert_eq!(CodecId::Columnar as u8, 6);
    assert_eq!(CodecId::try_from(5).unwrap(), CodecId::Segmented);
    assert_eq!(CodecId::try_from(6).unwrap(), CodecId::Columnar);
    assert!(matches!(
        CodecId::try_from(7),
        Err(AceError::UnsupportedCodec(7))
    ));
    for codec in [CodecId::Raw, CodecId::Rle, CodecId::Lz, CodecId::Numeric, CodecId::TimeSeries] {
        assert!(!codec.is_container());
    }
    assert!(CodecId::Segmented.is_container() && CodecId::Columnar.is_container());
}

/// A SEG1 / COL1 block parses in a 1.6 file and is rejected by every older minor version.
#[test]
fn container_codecs_require_format_1_6() {
    for codec in [CodecId::Segmented, CodecId::Columnar] {
        assert_eq!(parse(&plain(codec), 6).unwrap().codec, codec);
        for minor in 0..6u8 {
            assert!(
                matches!(
                    parse(&plain(codec), minor),
                    Err(AceError::UnsupportedVersion { minor: m, .. }) if m == minor
                ),
                "{codec:?} minor {minor}"
            );
        }
    }
}

/// An outer SEG1 / COL1 block with a transform, an entropy stage or a dictionary is malformed.
#[test]
fn container_blocks_must_be_plain() {
    for codec in [CodecId::Segmented, CodecId::Columnar] {
        let mut with_transform = plain(codec);
        with_transform.transforms = vec![TransformId::DeltaByte];
        let mut with_entropy = plain(codec);
        with_entropy.entropy = EntropyCodecId::Rans;
        let mut with_dictionary = plain(codec);
        with_dictionary.dictionary = Some(DictionaryRef {
            id: DictionaryId(9),
            scope: DictionaryScope::File,
        });
        for header in [with_transform, with_entropy, with_dictionary] {
            assert!(
                matches!(parse(&header, 6), Err(AceError::Malformed(_))),
                "{codec:?}"
            );
        }
    }
}

/// The dictionary file flag announces the `ADIC` section: valid in 1.6, rejected below it.
#[test]
fn dictionary_flag_requires_format_1_6() {
    let header = |minor: u8, flags: u16| {
        encode_file_header(&FileHeader {
            minor_version: minor,
            flags,
            default_block_size: 4096,
            original_size: 0,
            block_count: 0,
        })
    };
    assert_eq!(FORMAT_MINOR, 6);
    let flags = FILE_FLAG_HAS_INDEX | FILE_FLAG_HAS_DICTIONARIES;
    assert_eq!(
        decode_file_header(&header(6, flags)).unwrap().flags,
        flags,
        "1.6 carries the flag"
    );
    for minor in 1..6u8 {
        assert!(
            matches!(
                decode_file_header(&header(minor, flags)),
                Err(AceError::UnsupportedFeature(FILE_FLAG_HAS_DICTIONARIES))
            ),
            "minor {minor}"
        );
        assert!(decode_file_header(&header(minor, FILE_FLAG_HAS_INDEX)).is_ok());
    }
}
