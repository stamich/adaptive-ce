//! The C ABI writes exactly the bytes of the committed 0.6.1 golden file, whatever the thread
//! count, and reads them back.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::path::PathBuf;
use std::ptr;

use ace_corpus::Workload;
use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress, ace_config_default,
    ace_decompress, ace_engine_free, ace_engine_new, AceBuffer, AceConfig, AceEngine, ACE_OK,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const WORKLOAD_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
struct Golden {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    workload: String,
    input_sha256: String,
    profiles: Vec<GoldenProfile>,
}

#[derive(Deserialize)]
struct GoldenProfile {
    profile: String,
    ace_sha256: String,
    ace_bytes: usize,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn golden() -> Golden {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/0.6.1/GOLDEN.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn take(buffer: *mut AceBuffer) -> Vec<u8> {
    // SAFETY: `buffer` is a live handle freed once after the copy.
    unsafe {
        let bytes =
            std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer)).to_vec();
        ace_buffer_free(buffer);
        bytes
    }
}

fn engine(profile: u32, threads: u64) -> *mut AceEngine {
    let mut config = std::mem::MaybeUninit::<AceConfig>::uninit();
    // SAFETY: `ace_config_default` initialises the whole struct.
    let mut config = unsafe {
        ace_config_default(config.as_mut_ptr());
        config.assume_init()
    };
    config.profile = profile;
    config.threads = threads;
    let mut handle = ptr::null_mut();
    // SAFETY: valid config, writable output.
    let status = unsafe { ace_engine_new(&config, ptr::null(), &mut handle) };
    assert_eq!(status, ACE_OK);
    handle
}

fn compress(engine: *const AceEngine, input: &[u8]) -> Vec<u8> {
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
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

fn decompress(engine: *const AceEngine, packed: &[u8]) -> Vec<u8> {
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
    let status = unsafe { ace_decompress(engine, packed.as_ptr(), packed.len(), &mut out) };
    assert_eq!(status, ACE_OK);
    take(out)
}

#[test]
fn the_c_abi_reproduces_the_0_6_1_golden_file() {
    let golden = golden();
    assert_eq!(golden.entries.len(), Workload::ALL.len());
    for entry in &golden.entries {
        let workload = Workload::ALL
            .iter()
            .copied()
            .find(|w| w.name() == entry.workload)
            .unwrap();
        let input = workload.generate(WORKLOAD_BYTES);
        assert_eq!(hex(&input), entry.input_sha256, "{}", entry.workload);
        for expected in &entry.profiles {
            let code = match expected.profile.as_str() {
                "fast" => 0,
                "balanced" => 1,
                "dense" => 2,
                other => panic!("unknown profile {other}"),
            };
            let handle = engine(code, 0);
            let packed = compress(handle, &input);
            let label = format!("{} {}", entry.workload, expected.profile);
            assert_eq!(packed.len(), expected.ace_bytes, "{label}");
            assert_eq!(hex(&packed), expected.ace_sha256, "{label}");
            assert_eq!(decompress(handle, &packed), input, "{label}");
            // SAFETY: released once.
            unsafe { ace_engine_free(handle) };
        }
    }
}

#[test]
fn the_bytes_do_not_depend_on_the_thread_count() {
    let golden = golden();
    let entry = golden
        .entries
        .iter()
        .find(|e| e.workload == "low-cardinality")
        .unwrap();
    let workload = Workload::ALL
        .iter()
        .copied()
        .find(|w| w.name() == entry.workload)
        .unwrap();
    let input = workload.generate(WORKLOAD_BYTES);
    let balanced = entry
        .profiles
        .iter()
        .find(|p| p.profile == "balanced")
        .unwrap();
    for threads in [1, 2, 3, 8] {
        let handle = engine(1, threads);
        assert_eq!(
            hex(&compress(handle, &input)),
            balanced.ace_sha256,
            "{threads} threads"
        );
        // SAFETY: released once.
        unsafe { ace_engine_free(handle) };
    }
}
