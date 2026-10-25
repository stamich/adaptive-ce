//! Compression and decompression with preset dictionaries (Format 1.6): embedded and external
//! dictionaries in every reader, the rules that keep files without a dictionary unchanged, and
//! the behaviour on damaged or unresolvable references.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;
use std::sync::Arc;

use ace_core::{
    AceConfig, AceError, CodecId, DecodeLimits, DictionaryId, DictionaryRef, DictionaryScope,
    EncoderDictionary, EntropyCodecId, SegmentationConfig,
};
use ace_dictionary::{
    dictionary_id_of, train_dictionary, Dictionary, DictionaryRegistry, TrainerConfig,
};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{
    checksum, AceWriter, BlockHeader, EmbeddedDictionary, FileHeader, FILE_FLAG_HAS_DICTIONARIES,
    FILE_FLAG_HAS_INDEX,
};

/// One small JSON-like record; the keys are shared, the values differ.
fn record(i: usize) -> String {
    format!(
        "{{\"event\":\"temperature_reading\",\"device_id\":{},\"unit\":\"celsius\",\
         \"value\":{},\"firmware\":\"release-2.7.1\",\"site\":\"warehouse-{}\"}}\n",
        i,
        (i * 7919) % 400,
        i % 13
    )
}

/// `count` records as one input.
fn records(count: usize) -> Vec<u8> {
    (0..count).map(record).collect::<String>().into_bytes()
}

/// A dictionary trained on separate records, with its scope.
fn dictionary(scope: DictionaryScope) -> EncoderDictionary {
    let samples: Vec<Vec<u8>> = (1000..1200).map(|i| record(i).into_bytes()).collect();
    let borrowed: Vec<&[u8]> = samples.iter().map(Vec::as_slice).collect();
    // A small dictionary: an embedded one has to pay for its own bytes.
    let config = TrainerConfig {
        dictionary_size: 1024,
        segment_len: 32,
        ..TrainerConfig::default()
    };
    let bytes = train_dictionary(&borrowed, &config).unwrap();
    EncoderDictionary {
        id: dictionary_id_of(&bytes),
        scope,
        bytes: Arc::from(bytes),
    }
}

/// 4 KiB blocks: small blocks are where a dictionary matters.
fn config(dictionary: Option<EncoderDictionary>) -> AceConfig {
    AceConfig {
        block_size: 4096,
        threads: 1,
        dictionary,
        ..AceConfig::default()
    }
}

/// A registry holding `dictionary`.
fn registry(dictionary: &EncoderDictionary) -> DictionaryRegistry {
    let mut registry = DictionaryRegistry::new();
    registry.insert(Dictionary::new(
        dictionary.id,
        dictionary.scope,
        dictionary.bytes.to_vec(),
    ));
    registry
}

/// File flags of a container.
fn flags(file: &[u8]) -> u16 {
    u16::from_le_bytes([file[6], file[7]])
}

#[test]
fn an_embedded_dictionary_makes_a_smaller_self_contained_file() {
    let input = records(4000);
    let plain = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
    let engine = AceEngine::new(config(Some(dictionary(DictionaryScope::File)))).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert!(file.len() < plain.len(), "{} !< {}", file.len(), plain.len());
    assert_eq!(stats.format_minor, 6);
    assert_eq!(file[5], 6);
    assert_ne!(flags(&file) & FILE_FLAG_HAS_DICTIONARIES, 0);
    assert!(stats.dictionary_blocks > 0);
    // Self-contained: a default engine without any registry reads it, in every reader.
    assert_eq!(AceEngine::default_engine().decompress(&file).unwrap(), input);
    let mut streamed = Vec::new();
    AceEngine::default_engine()
        .decompress_from(Cursor::new(&file), &mut streamed)
        .unwrap();
    assert_eq!(streamed, input);
    let mut indexed = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default()).unwrap();
    assert_eq!(indexed.read_range(5000..9000).unwrap(), &input[5000..9000]);
    assert_eq!(indexed.decode_block(1).unwrap(), &input[4096..8192]);
}

#[test]
fn an_external_dictionary_needs_the_registry_to_decode() {
    let input = records(4000);
    let dictionary = dictionary(DictionaryScope::External);
    let engine = AceEngine::new(config(Some(dictionary.clone()))).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert_eq!(stats.format_minor, 6);
    assert_eq!(flags(&file) & FILE_FLAG_HAS_DICTIONARIES, 0, "nothing is embedded");
    assert!(stats.dictionary_blocks > 0);

    // Smaller than the embedded variant by the section.
    let embedded = AceEngine::new(config(Some(self::dictionary(DictionaryScope::File))))
        .unwrap()
        .compress(&input)
        .unwrap();
    assert!(file.len() < embedded.len());

    // The engine that wrote it reads it back; so does one with the registry.
    assert_eq!(engine.decompress(&file).unwrap(), input);
    let reader = AceEngine::default_engine().with_dictionaries(registry(&dictionary));
    assert_eq!(reader.decompress(&file).unwrap(), input);

    // Without the dictionary the reader says which one is missing.
    let missing = AceEngine::default_engine().decompress(&file).unwrap_err();
    assert!(matches!(missing, AceError::MissingDictionary(id) if id == dictionary.id.0));
    let mut indexed = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default()).unwrap();
    assert!(matches!(
        indexed.read_range(0..100),
        Err(AceError::MissingDictionary(_))
    ));
    let mut indexed = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default())
        .unwrap()
        .with_dictionaries(registry(&dictionary));
    assert_eq!(indexed.read_range(100..4200).unwrap(), &input[100..4200]);
}

#[test]
fn a_dictionary_that_is_not_used_leaves_the_bytes_unchanged() {
    // Random bytes do not match a text dictionary: no block uses it, so the file is the file
    // the same input gives without a dictionary (no section, no version bump).
    let mut state = 99u64;
    let input: Vec<u8> = (0..20_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    let without = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
    for scope in [DictionaryScope::File, DictionaryScope::External] {
        let with = AceEngine::new(config(Some(dictionary(scope))))
            .unwrap()
            .compress(&input)
            .unwrap();
        assert_eq!(with, without, "{scope:?}");
    }
    assert_eq!(without[5], 3);
}

#[test]
fn an_embedded_dictionary_that_does_not_pay_for_itself_is_dropped() {
    // A tiny input saves a few bytes at best, far less than the 28 + dictionary bytes of the
    // section, so the file is encoded without the dictionary.
    let input = records(3);
    let without = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
    let with = AceEngine::new(config(Some(dictionary(DictionaryScope::File))))
        .unwrap()
        .compress(&input)
        .unwrap();
    assert_eq!(with, without);
    // The same input with an external dictionary has no section to pay for and uses it.
    let external = AceEngine::new(config(Some(dictionary(DictionaryScope::External))))
        .unwrap()
        .compress(&input)
        .unwrap();
    assert!(external.len() < without.len());
}

#[test]
fn the_output_does_not_depend_on_the_thread_count() {
    let input = records(2000);
    let dictionary = dictionary(DictionaryScope::File);
    let outputs: Vec<Vec<u8>> = [1usize, 2, 8]
        .into_iter()
        .map(|threads| {
            AceEngine::new(AceConfig {
                threads,
                ..config(Some(dictionary.clone()))
            })
            .unwrap()
            .compress(&input)
            .unwrap()
        })
        .collect();
    assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn segments_may_use_the_dictionary() {
    // Records, then counters, then noise: the block is segmented (the counters get NUM1, the
    // noise RAW) and the record segments use the dictionary like blocks do.
    let mut state = 5u64;
    let noise: Vec<u8> = (0..128 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    let mut input = records(2000);
    input.extend((0u32..48 * 1024).flat_map(u32::to_le_bytes));
    input.extend_from_slice(&noise);
    let config = AceConfig {
        block_size: input.len(),
        segmentation: Some(SegmentationConfig {
            window_size: 4096,
            min_segment_size: 16 * 1024,
            max_segment_size: 128 * 1024,
            max_segments: 64,
        }),
        ..config(Some(dictionary(DictionaryScope::External)))
    };
    let engine = AceEngine::new(config.clone()).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert_eq!(stats.segmented_blocks, 1, "the fixture must be segmented");
    assert!(stats.dictionary_blocks > 0, "no segment used the dictionary");
    assert_eq!(engine.decompress(&file).unwrap(), input);
    // Range reads at segment granularity resolve the dictionary too.
    let reader = registry(config.dictionary.as_ref().unwrap());
    let mut indexed = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default())
        .unwrap()
        .with_dictionaries(reader);
    assert_eq!(indexed.read_range(100..2000).unwrap(), &input[100..2000]);
}

#[test]
fn invalid_dictionaries_are_rejected_by_the_engine() {
    let good = dictionary(DictionaryScope::File);
    let bad = [
        EncoderDictionary {
            scope: DictionaryScope::Block,
            ..good.clone()
        },
        EncoderDictionary {
            scope: DictionaryScope::Segment,
            ..good.clone()
        },
        EncoderDictionary {
            bytes: Arc::from(Vec::new()),
            ..good.clone()
        },
        EncoderDictionary {
            bytes: Arc::from(vec![7u8; 65_536]),
            ..good.clone()
        },
    ];
    for dictionary in bad {
        assert!(matches!(
            AceEngine::new(config(Some(dictionary))),
            Err(AceError::InvalidConfig(_))
        ));
    }
    assert!(AceEngine::new(config(Some(good))).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Damaged or unresolvable references. The files are built block by block.
// ---------------------------------------------------------------------------------------------

/// A one-block file whose block is `header` with `payload`, under file minor `minor`.
fn crafted(
    minor: u8,
    embedded: &[EmbeddedDictionary],
    codec: CodecId,
    dictionary: Option<DictionaryRef>,
    payload: &[u8],
    original: &[u8],
) -> Vec<u8> {
    let header = BlockHeader {
        block_id: 0,
        original_size: original.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: 0,
        codec,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary,
        flags: 0,
        payload_crc32c: checksum(original),
    };
    let mut file_flags = FILE_FLAG_HAS_INDEX;
    if !embedded.is_empty() {
        file_flags |= FILE_FLAG_HAS_DICTIONARIES;
    }
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: minor,
            flags: file_flags,
            default_block_size: 4096,
            original_size: original.len() as u64,
            block_count: 1,
        })
        .unwrap();
    if !embedded.is_empty() {
        writer.write_dictionary_section(embedded).unwrap();
    }
    writer.write_block(&header, &[], payload).unwrap();
    writer.finish(true).unwrap().0
}

/// The LZ stream "match of 4 bytes at distance `distance`".
fn match_stream(distance: u16) -> Vec<u8> {
    let mut stream = vec![0x80];
    stream.extend_from_slice(&distance.to_le_bytes());
    stream
}

const ID: DictionaryId = DictionaryId(42);

fn reference(scope: DictionaryScope) -> Option<DictionaryRef> {
    Some(DictionaryRef { id: ID, scope })
}

fn section(bytes: &[u8]) -> Vec<EmbeddedDictionary> {
    vec![EmbeddedDictionary {
        id: ID,
        bytes: bytes.to_vec(),
    }]
}

#[test]
fn a_reference_into_the_dictionary_decodes() {
    // "abc" is the dictionary; "match of 4 at distance 3" produces "abca".
    let file = crafted(
        6,
        &section(b"abc"),
        CodecId::Lz,
        reference(DictionaryScope::File),
        &match_stream(3),
        b"abca",
    );
    assert_eq!(AceEngine::default_engine().decompress(&file).unwrap(), b"abca");
}

#[test]
fn a_reference_before_the_dictionary_is_rejected() {
    let file = crafted(
        6,
        &section(b"abc"),
        CodecId::Lz,
        reference(DictionaryScope::File),
        &match_stream(5),
        b"abca",
    );
    assert!(matches!(
        AceEngine::default_engine().decompress(&file),
        Err(AceError::InvalidLzReference)
    ));
}

#[test]
fn the_wrong_dictionary_is_caught_by_the_checksum() {
    let file = crafted(
        6,
        &[],
        CodecId::Lz,
        reference(DictionaryScope::External),
        &match_stream(3),
        b"abca",
    );
    let mut wrong = DictionaryRegistry::new();
    wrong.insert(Dictionary::new(ID, DictionaryScope::External, b"xyz".to_vec()));
    assert!(matches!(
        AceEngine::default_engine().with_dictionaries(wrong).decompress(&file),
        Err(AceError::ChecksumMismatch(0))
    ));
    let mut right = DictionaryRegistry::new();
    right.insert(Dictionary::new(ID, DictionaryScope::External, b"abc".to_vec()));
    assert_eq!(
        AceEngine::default_engine().with_dictionaries(right).decompress(&file).unwrap(),
        b"abca"
    );
}

#[test]
fn unresolvable_references_are_reported() {
    // A file-scope reference to an id the section does not hold.
    let other = vec![EmbeddedDictionary {
        id: DictionaryId(7),
        bytes: b"abc".to_vec(),
    }];
    let file = crafted(
        6,
        &other,
        CodecId::Lz,
        reference(DictionaryScope::File),
        &match_stream(3),
        b"abca",
    );
    assert!(matches!(
        AceEngine::default_engine().decompress(&file),
        Err(AceError::MissingDictionary(42))
    ));
    // A file-scope reference without any section.
    let file = crafted(
        6,
        &[],
        CodecId::Lz,
        reference(DictionaryScope::File),
        &match_stream(3),
        b"abca",
    );
    assert!(matches!(
        AceEngine::default_engine().decompress(&file),
        Err(AceError::MissingDictionary(42))
    ));
}

#[test]
fn a_descriptor_below_format_1_6_is_missing_its_dictionary() {
    let file = crafted(
        5,
        &[],
        CodecId::Lz,
        reference(DictionaryScope::External),
        &match_stream(3),
        b"abca",
    );
    assert!(matches!(
        AceEngine::default_engine().decompress(&file),
        Err(AceError::MissingDictionary(42))
    ));
}

#[test]
fn reserved_scopes_and_foreign_codecs_are_malformed() {
    for scope in [DictionaryScope::Block, DictionaryScope::Segment] {
        let file = crafted(6, &[], CodecId::Lz, reference(scope), &match_stream(3), b"abca");
        assert!(matches!(
            AceEngine::default_engine().decompress(&file),
            Err(AceError::Malformed("reserved dictionary scope"))
        ));
    }
    // RAW has no use for a dictionary.
    let file = crafted(
        6,
        &section(b"abc"),
        CodecId::Raw,
        reference(DictionaryScope::File),
        b"abca",
        b"abca",
    );
    assert!(matches!(
        AceEngine::default_engine().decompress(&file),
        Err(AceError::Malformed(_))
    ));
}

#[test]
fn an_external_dictionary_above_the_limit_is_refused() {
    let file = crafted(
        6,
        &[],
        CodecId::Lz,
        reference(DictionaryScope::External),
        &match_stream(3),
        b"abca",
    );
    let mut large = DictionaryRegistry::new();
    large.insert(Dictionary::new(ID, DictionaryScope::External, b"abc".to_vec()));
    let limits = DecodeLimits {
        max_dictionary_size: 2,
        ..DecodeLimits::default()
    };
    let engine = AceEngine::default_engine()
        .with_dictionaries(large)
        .with_decode_limits(limits);
    assert!(matches!(
        engine.decompress(&file),
        Err(AceError::ResourceLimitExceeded("dictionary size"))
    ));
}
