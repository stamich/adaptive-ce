//! The C ABI writes exactly the bytes of the committed 0.8.0 storage golden file: columnar blocks
//! from a schema, segments, and embedded and external dictionaries trained through
//! `ace_dictionary_train`, for every profile.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::path::PathBuf;
use std::ptr;

use ace_corpus::RecordWorkload;
use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress, ace_config_default,
    ace_decompress, ace_dictionary_data, ace_dictionary_free, ace_dictionary_id,
    ace_dictionary_len, ace_dictionary_train, ace_engine_free, ace_engine_new_ex, ace_schema_free,
    ace_schema_new, AceBuffer, AceColumn, AceConfig, AceDictionary, AceEngine, AceSchema,
    ACE_COLUMN_BYTES, ACE_COLUMN_FLOAT, ACE_COLUMN_SIGNED, ACE_COLUMN_UNSIGNED,
    ACE_DICTIONARY_EXTERNAL, ACE_OK,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The members of a case that a binding needs (the file has more).
#[derive(Deserialize)]
struct Case {
    case: String,
    generator: String,
    block_size: u64,
    schema: String,
    segmentation_min: u64,
    dictionary: String,
    dictionary_id: String,
    dictionary_sha256: String,
    input_sha256: String,
    fast_sha256: String,
    fast_bytes: usize,
    balanced_sha256: String,
    balanced_bytes: usize,
    dense_sha256: String,
    dense_bytes: usize,
}

#[derive(Deserialize)]
struct Golden {
    cases: Vec<Case>,
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn take(buffer: *mut AceBuffer) -> Vec<u8> {
    assert!(!buffer.is_null());
    // SAFETY: `buffer` is a live handle; it is freed once, after the copy.
    unsafe {
        let bytes =
            std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer)).to_vec();
        ace_buffer_free(buffer);
        bytes
    }
}

fn json_record(i: usize) -> Vec<u8> {
    format!(
        "{{\"sensor\":\"temperature_reading\",\"unit\":\"celsius\",\"site\":\"plant-{}\",\"value\":{}}}\n",
        i % 7,
        1000 + i * 13 % 97
    )
    .into_bytes()
}

fn input(generator: &str) -> Vec<u8> {
    if let Some(rest) = generator.strip_prefix("records-") {
        let (name, size) = rest.split_once(':').unwrap();
        return RecordWorkload::from_name(&format!("records-{name}"))
            .unwrap()
            .generate(size.parse().unwrap());
    }
    if let Some(count) = generator.strip_prefix("json-records:") {
        return (0..count.parse().unwrap()).flat_map(json_record).collect();
    }
    assert_eq!(generator, "mixed-regimes");
    let mut out: Vec<u8> = (0..400_000u32).flat_map(|i| (i / 2).to_le_bytes()).collect();
    out.extend((0..1_500_000u64).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8));
    out
}

fn columns(layout: &str) -> Vec<AceColumn> {
    layout
        .split(',')
        .map(|column| AceColumn {
            kind: match &column[..1] {
                "u" => ACE_COLUMN_UNSIGNED,
                "s" => ACE_COLUMN_SIGNED,
                "f" => ACE_COLUMN_FLOAT,
                "b" => ACE_COLUMN_BYTES,
                other => panic!("column kind {other}"),
            },
            width: column[1..].parse().unwrap(),
        })
        .collect()
}

fn train() -> *mut AceDictionary {
    let samples: Vec<Vec<u8>> = (0..300).map(json_record).collect();
    let pointers: Vec<*const u8> = samples.iter().map(|s| s.as_ptr()).collect();
    let lengths: Vec<usize> = samples.iter().map(Vec::len).collect();
    let mut handle = ptr::null_mut();
    // SAFETY: parallel arrays of readable samples; `handle` writable.
    let status = unsafe {
        ace_dictionary_train(pointers.as_ptr(), lengths.as_ptr(), samples.len(), 2048, &mut handle)
    };
    assert_eq!(status, ACE_OK);
    handle
}

#[test]
fn the_c_abi_reproduces_the_0_8_0_storage_golden_file() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/0.8.0/GOLDEN.json");
    let golden: Golden = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(golden.cases.len(), 6);
    for case in &golden.cases {
        let data = input(&case.generator);
        assert_eq!(sha256(&data), case.input_sha256, "{}: generator", case.case);

        let dictionary = if case.dictionary == "none" { ptr::null_mut() } else { train() };
        if !dictionary.is_null() {
            // SAFETY: live handle; the bytes are valid until the free.
            unsafe {
                assert_eq!(format!("{:016x}", ace_dictionary_id(dictionary)), case.dictionary_id);
                let bytes = std::slice::from_raw_parts(
                    ace_dictionary_data(dictionary),
                    ace_dictionary_len(dictionary),
                );
                assert_eq!(sha256(bytes), case.dictionary_sha256, "{}: trained dictionary", case.case);
            }
        }
        let mut schema: *mut AceSchema = ptr::null_mut();
        if !case.schema.is_empty() {
            let columns = columns(&case.schema);
            // SAFETY: readable column array; `schema` writable.
            assert_eq!(unsafe { ace_schema_new(columns.as_ptr(), columns.len(), &mut schema) }, ACE_OK);
        }

        for (profile, (sha, size)) in [
            ("fast", (&case.fast_sha256, case.fast_bytes)),
            ("balanced", (&case.balanced_sha256, case.balanced_bytes)),
            ("dense", (&case.dense_sha256, case.dense_bytes)),
        ] {
            let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
            // SAFETY: the function writes the whole struct.
            let mut config = unsafe {
                ace_config_default(config.as_mut_ptr());
                config.assume_init()
            };
            config.profile = match profile {
                "fast" => 0,
                "balanced" => 1,
                _ => 2,
            };
            config.block_size = case.block_size;
            config.schema = schema;
            config.dictionary = dictionary;
            config.dictionary_scope = u32::from(case.dictionary == "external") * ACE_DICTIONARY_EXTERNAL;
            if case.segmentation_min > 0 {
                config.segmentation = 1;
                config.segment_min = case.segmentation_min;
            }
            let mut engine: *mut AceEngine = ptr::null_mut();
            let known: [*const AceDictionary; 1] = [dictionary];
            let count = usize::from(!dictionary.is_null());
            // SAFETY: live config and handles; `engine` writable.
            let status =
                unsafe { ace_engine_new_ex(&config, ptr::null(), known.as_ptr(), count, &mut engine) };
            assert_eq!(status, ACE_OK, "{} {profile}", case.case);

            let mut out = ptr::null_mut();
            // SAFETY: live engine and input; the output is writable.
            let status = unsafe {
                ace_compress(engine, data.as_ptr(), data.len(), &mut out, ptr::null_mut())
            };
            assert_eq!(status, ACE_OK);
            let file = take(out);
            assert_eq!(file.len(), size, "{} {profile}: size", case.case);
            assert_eq!(&sha256(&file), sha, "{} {profile}: bytes", case.case);

            let mut back = ptr::null_mut();
            // SAFETY: live engine and file; the output is writable.
            let status = unsafe { ace_decompress(engine, file.as_ptr(), file.len(), &mut back) };
            assert_eq!(status, ACE_OK, "{} {profile}", case.case);
            assert_eq!(take(back), data, "{} {profile}: round trip", case.case);
            // SAFETY: freed once.
            unsafe { ace_engine_free(engine) };
        }
        // SAFETY: freed once; engines keep their own copies.
        unsafe {
            ace_schema_free(schema);
            ace_dictionary_free(dictionary);
        }
    }
}
