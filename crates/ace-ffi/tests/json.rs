//! The JSON shapes of `docs/FFI.md`: Stats and the planner report.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::ptr;

use ace::Stats;
use ace_ffi::json::{explain_json, stats_json};
use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_engine_free, ace_engine_new,
    ace_explain_json, ACE_OK,
};

#[test]
fn stats_json_is_stable_and_escapes_labels() {
    let mut stats = Stats::default();
    stats.input_bytes = 1000;
    stats.output_bytes = 250;
    stats.format_minor = 5;
    stats.blocks = 2;
    stats.lz_blocks = 1;
    stats.numeric_blocks = 1;
    stats.plan_distribution.insert("lz+huffman".to_owned(), 1);
    stats.plan_distribution.insert("na\"me\\".to_owned(), 1);
    assert_eq!(
        stats_json(&stats),
        "{\"input_bytes\":1000,\"output_bytes\":250,\"format_minor\":5,\"blocks\":2,\
         \"raw_blocks\":0,\"rle_blocks\":0,\"lz_blocks\":1,\"numeric_blocks\":1,\
         \"time_series_blocks\":0,\"ratio\":4,\"plan_distribution\":\
         {\"lz+huffman\":1,\"na\\\"me\\\\\":1},\
         \"segmented_blocks\":0,\"columnar_blocks\":0,\"dictionary_blocks\":0}"
    );
}

#[test]
fn empty_stats_have_a_unit_ratio() {
    assert!(stats_json(&Stats::default()).contains("\"ratio\":1,"));
}

#[test]
fn explain_json_lists_every_block() {
    let data: Vec<u8> = (0_u32..5000).flat_map(|i| (i * 3).to_le_bytes()).collect();
    let mut handle = ptr::null_mut();
    // SAFETY: null configs are the defaults; the output is writable.
    let status = unsafe { ace_engine_new(ptr::null(), ptr::null(), &mut handle) };
    assert_eq!(status, ACE_OK);
    let mut out = ptr::null_mut();
    // SAFETY: live engine and input; writable output.
    let status = unsafe { ace_explain_json(handle, data.as_ptr(), data.len(), &mut out) };
    assert_eq!(status, ACE_OK);
    // SAFETY: `out` is a live buffer freed once after the copy.
    let text = unsafe {
        let bytes = std::slice::from_raw_parts(ace_buffer_data(out), ace_buffer_len(out)).to_vec();
        ace_buffer_free(out);
        ace_engine_free(handle);
        String::from_utf8(bytes).unwrap()
    };
    let reports = ace::Engine::default().explain(&data).unwrap();
    assert_eq!(text, explain_json(&reports));
    assert!(text.starts_with("[{\"block_id\":0,\"size\":20000,\"route\":\""));
    assert!(text.contains("\"selected\":\""));
    assert!(text.contains("\"candidates\":["));
    assert!(text.ends_with("]}]"));
}

#[test]
fn explain_of_no_blocks_is_an_empty_array() {
    assert_eq!(explain_json(&[]), "[]");
}
