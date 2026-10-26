//! Dictionaries in the facade: training, embedded and external use, readers, streaming and the
//! rules that keep files without a useful dictionary unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace::{Config, DecodeLimits, Dictionary, Engine, ErrorKind, RangeReader};

/// One record; the keys are shared, the values differ.
fn record(i: usize) -> String {
    format!(
        "{{\"event\":\"temperature_reading\",\"device_id\":{i},\"unit\":\"celsius\",\
         \"value\":{},\"firmware\":\"release-2.7.1\",\"site\":\"warehouse-{}\"}}\n",
        (i * 7919) % 400,
        i % 13
    )
}

fn records(range: std::ops::Range<usize>) -> Vec<u8> {
    range.map(record).collect::<String>().into_bytes()
}

fn trained() -> Dictionary {
    let samples: Vec<Vec<u8>> = (1000..1200).map(|i| record(i).into_bytes()).collect();
    Dictionary::train(&samples, 1024).unwrap()
}

fn builder() -> ace::ConfigBuilder {
    Config::builder().block_size(4096).threads(1)
}

#[test]
fn training_is_deterministic_and_the_id_follows_the_content() {
    let a = trained();
    let b = trained();
    assert_eq!(a, b);
    assert_eq!(a.id(), b.id());
    assert!(!a.is_empty() && a.len() <= 1024);
    assert_eq!(Dictionary::new(a.as_bytes().to_vec()).unwrap().id(), a.id());
    assert_ne!(
        Dictionary::new(b"other bytes".to_vec()).unwrap().id(),
        a.id()
    );
    assert_eq!(
        Dictionary::with_id(7, a.as_bytes().to_vec()).unwrap().id(),
        7
    );
}

#[test]
fn invalid_dictionaries_and_training_inputs_are_invalid_configuration() {
    let samples = [b"abc".to_vec()];
    let cases = [
        Dictionary::new(Vec::new()),
        Dictionary::new(vec![1u8; 65_536]),
        Dictionary::with_id(0, vec![1u8, 2, 3]),
        Dictionary::train(&samples, 1024),
        Dictionary::train(&[b"x".repeat(100), b"y".repeat(100)], 1024),
        Dictionary::train(&[b"x".repeat(100), b"x".repeat(100)], 63),
        Dictionary::train(&[b"x".repeat(100), b"x".repeat(100)], 65_536),
    ];
    for case in cases {
        assert_eq!(case.unwrap_err().kind(), ErrorKind::InvalidConfig);
    }
}

#[test]
fn an_embedded_dictionary_gives_a_smaller_self_contained_file() {
    let input = records(0..4000);
    let plain = Engine::new(builder().build().unwrap()).unwrap();
    let with = Engine::new(builder().dictionary(trained()).build().unwrap()).unwrap();
    let plain_file = plain.compress(&input).unwrap();
    let (file, stats) = with.compress_with_stats(&input).unwrap();
    assert!(file.len() < plain_file.len());
    assert_eq!(stats.format_minor, 6);
    assert!(stats.dictionary_blocks > 0);
    assert_eq!(Engine::default().decompress(&file).unwrap(), input);
    let mut reader = RangeReader::open(Cursor::new(&file)).unwrap();
    assert_eq!(
        reader.read_range(10_000..20_000).unwrap(),
        &input[10_000..20_000]
    );
    let config = builder().dictionary(trained()).build().unwrap();
    assert!(config.dictionary_is_embedded());
    assert_eq!(config.dictionary(), Some(trained()));
}

#[test]
fn an_external_dictionary_is_resolved_by_the_reader() {
    let input = records(0..4000);
    let dictionary = trained();
    let writer = Engine::new(
        builder()
            .external_dictionary(dictionary.clone())
            .build()
            .unwrap(),
    )
    .unwrap();
    let file = writer.compress(&input).unwrap();
    assert!(!writer.config().dictionary_is_embedded());

    // The engine that wrote it, and one told about the dictionary, read it.
    assert_eq!(writer.decompress(&file).unwrap(), input);
    let reader = Engine::default().with_dictionaries([dictionary.clone()]);
    assert_eq!(reader.decompress(&file).unwrap(), input);
    let mut range = reader.open_range_reader(Cursor::new(&file)).unwrap();
    assert_eq!(range.read_range(5000..6000).unwrap(), &input[5000..6000]);
    let mut range = RangeReader::open_with_dictionaries(
        Cursor::new(&file),
        &DecodeLimits::default(),
        [dictionary.clone()],
    )
    .unwrap();
    assert_eq!(range.read_block(1).unwrap(), &input[4096..8192]);

    // Without it the error says so.
    let error = Engine::default().decompress(&file).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert!(error.to_string().contains("dictionary"), "{error}");
    let mut range = RangeReader::open(Cursor::new(&file)).unwrap();
    assert_eq!(
        range.read_range(0..100).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    // A different dictionary under the same id never yields wrong bytes.
    let impostor = Dictionary::with_id(dictionary.id(), vec![b'z'; dictionary.len()]).unwrap();
    let wrong = Engine::default().with_dictionaries([impostor]);
    assert!(wrong.decompress(&file).is_err());
}

#[test]
fn a_dictionary_that_does_not_help_leaves_the_bytes_of_the_plain_engine() {
    let mut state = 17u64;
    let noise: Vec<u8> = (0..30_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    let plain = Engine::new(builder().build().unwrap())
        .unwrap()
        .compress(&noise)
        .unwrap();
    for config in [
        builder().dictionary(trained()).build().unwrap(),
        builder().external_dictionary(trained()).build().unwrap(),
    ] {
        assert_eq!(
            Engine::new(config).unwrap().compress(&noise).unwrap(),
            plain
        );
    }
}

#[test]
fn streaming_supports_external_dictionaries_and_matches_compress() {
    let input = records(0..3000);
    let engine = Engine::new(builder().external_dictionary(trained()).build().unwrap()).unwrap();
    let expected = engine.compress(&input).unwrap();
    let mut sink = Cursor::new(Vec::new());
    let stats = engine
        .compress_stream(Cursor::new(&input), &mut sink, input.len() as u64)
        .unwrap();
    assert_eq!(sink.into_inner(), expected);
    assert_eq!(stats.format_minor, 6);

    // An embedded dictionary needs the whole file first.
    let embedded = Engine::new(builder().dictionary(trained()).build().unwrap()).unwrap();
    let mut sink = Cursor::new(Vec::new());
    let error = embedded
        .compress_stream(Cursor::new(&input), &mut sink, input.len() as u64)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidConfig);
}
