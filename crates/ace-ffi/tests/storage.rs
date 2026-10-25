//! ABI 1.1: dictionaries, schemas and segmentation through the `extern "C"` API.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::ptr;

use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress, ace_config_default,
    ace_decompress, ace_dictionary_data, ace_dictionary_free, ace_dictionary_id,
    ace_dictionary_len, ace_dictionary_new, ace_dictionary_train, ace_engine_free, ace_engine_new,
    ace_engine_new_ex, ace_range_free, ace_range_open, ace_range_read, ace_schema_free,
    ace_schema_new, ace_schema_record_size, AceBuffer, AceColumn, AceConfig, AceDictionary,
    AceEngine, AceRangeReader, AceSchema, ACE_COLUMN_BYTES, ACE_COLUMN_FLOAT, ACE_COLUMN_UNSIGNED,
    ACE_DICTIONARY_EXTERNAL, ACE_ERR_INVALID_ARGUMENT, ACE_ERR_UNSUPPORTED, ACE_OK,
};

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

/// Small JSON-like records that share a vocabulary.
fn records(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| {
            format!(
                "{{\"sensor\":\"temperature_reading\",\"unit\":\"celsius\",\"site\":\"plant-{}\",\"value\":{}}}\n",
                i % 7,
                1000 + i * 13 % 97
            )
            .into_bytes()
        })
        .collect()
}

fn new_dictionary(bytes: &[u8], id: u64) -> *mut AceDictionary {
    let mut handle = ptr::null_mut();
    // SAFETY: `bytes` is readable and `handle` writable.
    let status = unsafe { ace_dictionary_new(bytes.as_ptr(), bytes.len(), id, &mut handle) };
    assert_eq!(status, ACE_OK);
    assert!(!handle.is_null());
    handle
}

fn engine_with(config: &AceConfig, dictionaries: &[*const AceDictionary]) -> *mut AceEngine {
    let mut handle = ptr::null_mut();
    // SAFETY: the config and the handles are live; `handle` is writable.
    let status = unsafe {
        ace_engine_new_ex(
            config,
            ptr::null(),
            dictionaries.as_ptr(),
            dictionaries.len(),
            &mut handle,
        )
    };
    assert_eq!(status, ACE_OK);
    handle
}

fn compress(engine: *const AceEngine, input: &[u8]) -> Vec<u8> {
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; the output is writable.
    let status = unsafe {
        ace_compress(
            engine,
            input.as_ptr(),
            input.len(),
            &mut out,
            ptr::null_mut(),
        )
    };
    assert_eq!(status, ACE_OK);
    take(out)
}

fn decompress(engine: *const AceEngine, file: &[u8]) -> Result<Vec<u8>, i32> {
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; the output is writable.
    let status = unsafe { ace_decompress(engine, file.as_ptr(), file.len(), &mut out) };
    if status == ACE_OK {
        Ok(take(out))
    } else {
        assert!(out.is_null());
        Err(status)
    }
}

#[test]
fn dictionary_handle_reports_its_content_and_derived_id() {
    let handle = new_dictionary(b"temperature_reading celsius", 0);
    // SAFETY: live handle; the data pointer is valid for `len` bytes until the free.
    unsafe {
        assert_eq!(ace_dictionary_len(handle), 27);
        let bytes = std::slice::from_raw_parts(ace_dictionary_data(handle), 27);
        assert_eq!(bytes, b"temperature_reading celsius");
        assert_eq!(
            ace_dictionary_id(handle),
            ace::Dictionary::new(b"temperature_reading celsius".to_vec())
                .unwrap()
                .id()
        );
        ace_dictionary_free(handle);
    }
    let named = new_dictionary(b"abc", 42);
    // SAFETY: live handle, freed once.
    unsafe {
        assert_eq!(ace_dictionary_id(named), 42);
        ace_dictionary_free(named);
    }
}

#[test]
fn dictionary_functions_accept_null_and_reject_bad_input() {
    // SAFETY: null is allowed by every query and by free.
    unsafe {
        assert_eq!(ace_dictionary_id(ptr::null()), 0);
        assert_eq!(ace_dictionary_len(ptr::null()), 0);
        assert!(ace_dictionary_data(ptr::null()).is_null());
        ace_dictionary_free(ptr::null_mut());
    }
    let mut handle = ptr::null_mut();
    // SAFETY: an empty dictionary is rejected; the output is writable.
    let status = unsafe { ace_dictionary_new(ptr::null(), 0, 0, &mut handle) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    assert!(handle.is_null());
    let big = vec![1u8; 70_000];
    // SAFETY: readable input; the output is writable.
    let status = unsafe { ace_dictionary_new(big.as_ptr(), big.len(), 0, &mut handle) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    // SAFETY: a null output pointer is reported, not written.
    let status = unsafe { ace_dictionary_new(big.as_ptr(), 10, 0, ptr::null_mut()) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
}

#[test]
fn trained_dictionary_equals_the_facade_dictionary() {
    let samples = records(300);
    let pointers: Vec<*const u8> = samples.iter().map(|s| s.as_ptr()).collect();
    let lengths: Vec<usize> = samples.iter().map(Vec::len).collect();
    let mut handle = ptr::null_mut();
    // SAFETY: arrays of equal length, each element readable; `handle` writable.
    let status = unsafe {
        ace_dictionary_train(
            pointers.as_ptr(),
            lengths.as_ptr(),
            samples.len(),
            2048,
            &mut handle,
        )
    };
    assert_eq!(status, ACE_OK);
    let expected = ace::Dictionary::train(&samples, 2048).unwrap();
    // SAFETY: live handle; bytes valid until the free.
    unsafe {
        assert_eq!(ace_dictionary_id(handle), expected.id());
        let bytes =
            std::slice::from_raw_parts(ace_dictionary_data(handle), ace_dictionary_len(handle));
        assert_eq!(bytes, expected.as_bytes());
        ace_dictionary_free(handle);
    }
}

#[test]
fn training_needs_samples_and_valid_arrays() {
    let mut handle = ptr::null_mut();
    // SAFETY: zero samples are rejected before any read; `handle` writable.
    let status = unsafe { ace_dictionary_train(ptr::null(), ptr::null(), 0, 1024, &mut handle) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    assert!(handle.is_null());
    // SAFETY: a null array with a non-zero count is reported.
    let status = unsafe { ace_dictionary_train(ptr::null(), ptr::null(), 5, 1024, &mut handle) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
}

/// A dictionary of the configuration is embedded; the engine without it reads the file alone.
#[test]
fn embedded_dictionary_round_trips_through_a_plain_engine() {
    let samples = records(300);
    // Enough blocks for the savings to exceed the cost of embedding the dictionary.
    let input: Vec<u8> = records(600).concat();
    let dictionary = ace::Dictionary::train(&samples, 2048).unwrap();
    let handle = new_dictionary(dictionary.as_bytes(), dictionary.id());

    let mut config = AceConfig {
        reserved: [0; 5],
        ..zeroed_config()
    };
    config.block_size = 4096;
    config.dictionary = handle;
    let writer = engine_with(&config, &[]);
    let packed = compress(writer, &input);

    let facade = ace::Engine::new(
        ace::Config::builder()
            .block_size(4096)
            .dictionary(dictionary.clone())
            .build()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        packed,
        facade.compress(&input).unwrap(),
        "same bytes as the Rust facade"
    );
    let without =
        ace::Engine::new(ace::Config::builder().block_size(4096).build().unwrap()).unwrap();
    assert!(
        packed.len() < without.compress(&input).unwrap().len(),
        "the dictionary pays"
    );

    let mut plain = ptr::null_mut();
    // SAFETY: null configs are the defaults; `plain` writable.
    let status = unsafe { ace_engine_new(ptr::null(), ptr::null(), &mut plain) };
    assert_eq!(status, ACE_OK);
    assert_eq!(decompress(plain, &packed).unwrap(), input);
    // SAFETY: each handle freed once; the engines own copies of the dictionary.
    unsafe {
        ace_engine_free(plain);
        ace_engine_free(writer);
        ace_dictionary_free(handle);
    }
}

#[test]
fn external_dictionary_needs_the_reader_to_supply_it() {
    let dictionary = ace::Dictionary::train(&records(300), 2048).unwrap();
    let input: Vec<u8> = records(60).concat();
    let handle = new_dictionary(dictionary.as_bytes(), dictionary.id());

    let mut config = zeroed_config();
    config.block_size = 4096;
    config.dictionary = handle;
    config.dictionary_scope = ACE_DICTIONARY_EXTERNAL;
    let writer = engine_with(&config, &[]);
    let packed = compress(writer, &input);

    let mut plain = ptr::null_mut();
    // SAFETY: null configs are the defaults; `plain` writable.
    let status = unsafe { ace_engine_new(ptr::null(), ptr::null(), &mut plain) };
    assert_eq!(status, ACE_OK);
    assert_eq!(decompress(plain, &packed), Err(ACE_ERR_UNSUPPORTED));

    let reader = engine_with(&zeroed_config(), &[handle]);
    assert_eq!(decompress(reader, &packed).unwrap(), input);

    // Range reads resolve the same dictionaries.
    let mut range: *mut AceRangeReader = ptr::null_mut();
    // SAFETY: live engine, readable file, writable output.
    let status = unsafe { ace_range_open(reader, packed.as_ptr(), packed.len(), &mut range) };
    assert_eq!(status, ACE_OK);
    let mut out = ptr::null_mut();
    // SAFETY: live reader; the output is writable.
    assert_eq!(unsafe { ace_range_read(range, 5, 25, &mut out) }, ACE_OK);
    assert_eq!(take(out), input[5..25]);
    // SAFETY: each handle freed once.
    unsafe {
        ace_range_free(range);
        ace_engine_free(reader);
        ace_engine_free(plain);
        ace_engine_free(writer);
        ace_dictionary_free(handle);
    }
}

#[test]
fn engine_new_ex_rejects_a_null_dictionary_handle() {
    let config = zeroed_config();
    let handles = [ptr::null::<AceDictionary>()];
    let mut out = ptr::null_mut();
    // SAFETY: the array is readable; a null element is reported.
    let status = unsafe { ace_engine_new_ex(&config, ptr::null(), handles.as_ptr(), 1, &mut out) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    assert!(out.is_null());
}

fn schema(columns: &[AceColumn]) -> Result<*mut AceSchema, i32> {
    let mut handle = ptr::null_mut();
    // SAFETY: readable column array; `handle` writable.
    let status = unsafe { ace_schema_new(columns.as_ptr(), columns.len(), &mut handle) };
    if status == ACE_OK {
        Ok(handle)
    } else {
        assert!(handle.is_null());
        Err(status)
    }
}

#[test]
fn schema_reports_its_record_size_and_validates_columns() {
    let columns = [
        AceColumn {
            kind: ACE_COLUMN_UNSIGNED,
            width: 8,
        },
        AceColumn {
            kind: ACE_COLUMN_FLOAT,
            width: 8,
        },
        AceColumn {
            kind: ACE_COLUMN_BYTES,
            width: 8,
        },
    ];
    let handle = schema(&columns).unwrap();
    // SAFETY: live handle, freed once.
    unsafe {
        assert_eq!(ace_schema_record_size(handle), 24);
        ace_schema_free(handle);
        assert_eq!(ace_schema_record_size(ptr::null()), 0);
        ace_schema_free(ptr::null_mut());
    }
    assert_eq!(schema(&[]), Err(ACE_ERR_INVALID_ARGUMENT));
    assert_eq!(
        schema(&[AceColumn { kind: 9, width: 4 }]),
        Err(ACE_ERR_INVALID_ARGUMENT)
    );
    assert_eq!(
        schema(&[AceColumn {
            kind: ACE_COLUMN_UNSIGNED,
            width: 3
        }]),
        Err(ACE_ERR_INVALID_ARGUMENT)
    );
    assert_eq!(
        schema(&[AceColumn {
            kind: ACE_COLUMN_FLOAT,
            width: 70_000
        }]),
        Err(ACE_ERR_INVALID_ARGUMENT)
    );
}

/// Fixed-size records: a counter, a price and a flag word.
fn table(count: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..count {
        out.extend_from_slice(&(1_700_000_000 + i * 5).to_le_bytes());
        out.extend_from_slice(&(100.0 + (i % 50) as f64 * 0.25).to_le_bytes());
        out.extend_from_slice(&(i % 3).to_le_bytes());
    }
    out
}

#[test]
fn schema_in_the_config_gives_the_facade_bytes() {
    let handle = schema(&[
        AceColumn {
            kind: ACE_COLUMN_UNSIGNED,
            width: 8,
        },
        AceColumn {
            kind: ACE_COLUMN_FLOAT,
            width: 8,
        },
        AceColumn {
            kind: ACE_COLUMN_UNSIGNED,
            width: 8,
        },
    ])
    .unwrap();
    let input = table(20_000);
    let mut config = zeroed_config();
    config.schema = handle;
    let writer = engine_with(&config, &[]);
    let packed = compress(writer, &input);

    let facade_schema = ace::Schema::new([
        ace::Column::unsigned(8),
        ace::Column::float(8),
        ace::Column::unsigned(8),
    ])
    .unwrap();
    let facade = ace::Engine::new(
        ace::Config::builder()
            .schema(facade_schema)
            .build()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(packed, facade.compress(&input).unwrap());
    assert!(packed.len() < ace::Engine::default().compress(&input).unwrap().len());
    assert_eq!(decompress(writer, &packed).unwrap(), input);
    // SAFETY: freed once; the engine owns a copy of the schema.
    unsafe {
        ace_engine_free(writer);
        ace_schema_free(handle);
    }
}

#[test]
fn segmentation_in_the_config_gives_the_facade_bytes() {
    let mut input = Vec::new();
    input.extend((0..400_000u32).flat_map(|i| (i / 2).to_le_bytes()));
    input.extend((0..1_500_000u64).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8));
    let mut config = zeroed_config();
    config.block_size = 4 << 20;
    config.segmentation = 1;
    config.segment_min = 128 * 1024;
    let writer = engine_with(&config, &[]);
    let packed = compress(writer, &input);

    let segmentation = ace::Segmentation::default().min_segment_size(128 * 1024);
    let facade = ace::Engine::new(
        ace::Config::builder()
            .block_size(4 << 20)
            .segmentation(segmentation)
            .build()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(packed, facade.compress(&input).unwrap());
    assert_eq!(decompress(writer, &packed).unwrap(), input);
    // SAFETY: freed once.
    unsafe { ace_engine_free(writer) };
}

/// A caller built against ABI 1.0 declares 56 bytes; the engine reads only those and keeps the
/// 1.1 defaults (no schema, no dictionary, no segmentation).
#[test]
fn a_one_point_zero_config_keeps_the_old_behaviour() {
    let mut config = zeroed_config();
    config.struct_size = 56;
    // Garbage behind the declared size must be ignored.
    config.dictionary_scope = 77;
    config.segmentation = 1;
    config.segment_min = 1;
    let writer = engine_with(&config, &[]);
    let input = table(5_000);
    assert_eq!(
        compress(writer, &input),
        ace::Engine::default().compress(&input).unwrap()
    );
    // SAFETY: freed once.
    unsafe { ace_engine_free(writer) };
}

#[test]
fn an_unknown_dictionary_scope_is_an_invalid_argument() {
    let handle = new_dictionary(b"temperature_reading", 0);
    let mut config = zeroed_config();
    config.dictionary = handle;
    config.dictionary_scope = 5;
    let mut out = ptr::null_mut();
    // SAFETY: live config and handle; `out` writable.
    let status = unsafe { ace_engine_new_ex(&config, ptr::null(), ptr::null(), 0, &mut out) };
    assert_eq!(status, ACE_ERR_INVALID_ARGUMENT);
    assert!(out.is_null());
    // SAFETY: freed once.
    unsafe { ace_dictionary_free(handle) };
}

/// The defaults as `ace_config_default` writes them (all ABI 1.1 members off).
fn zeroed_config() -> AceConfig {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: the function writes the whole struct.
    unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    }
}
