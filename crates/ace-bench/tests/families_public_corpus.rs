//! Tests of `ace_bench` `families_public_corpus` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ace_bench::families::{corpus_files, measure_directory, megabytes_per_second};
use ace_bench::timing::MeasurementPlan;
use std::fs;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ace-public-corpus-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn files_are_listed_sorted_and_directories_skipped() {
    let dir = scratch("list");
    fs::write(dir.join("b"), b"2").unwrap();
    fs::write(dir.join("a"), b"1").unwrap();
    fs::create_dir(dir.join("sub")).unwrap();
    let names: Vec<String> = corpus_files(&dir)
        .unwrap()
        .into_iter()
        .map(|f| f.0)
        .collect();
    assert_eq!(names, ["a", "b"]);
    assert!(corpus_files(&dir.join("missing")).is_err());
}

#[test]
fn speeds_are_decimal_megabytes_per_second() {
    assert!((megabytes_per_second(1_000_000, 1e9) - 1.0).abs() < 1e-9);
    assert_eq!(megabytes_per_second(10, 0.0), 0.0);
}

#[test]
fn a_directory_yields_a_row_per_file_and_compressor() {
    let dir = scratch("rows");
    let text = b"the quick brown fox jumps over the lazy dog\n".repeat(2000);
    fs::write(dir.join("fox.txt"), &text).unwrap();
    fs::write(dir.join("empty"), b"").unwrap();
    let rows = measure_directory(&dir, 60_000, MeasurementPlan::QUICK).unwrap();
    let paths: Vec<&str> = rows.iter().map(|r| r["path"].as_str().unwrap()).collect();
    assert_eq!(
        paths,
        [
            "ace-fast",
            "ace-balanced",
            "ace-dense",
            "lz4",
            "zstd-3",
            "gzip-6"
        ]
    );
    for r in &rows {
        assert_eq!(r["workload_id"], "fox.txt");
        assert_eq!(r["input_bytes"], 60_000);
        let l = &r["lzbench"];
        assert!(l["ratio_percent"].as_f64().unwrap() < 50.0, "{r}");
        assert!(l["compress_mb_s"].as_f64().unwrap() > 0.0);
        assert!(l["decompress_mb_s"].as_f64().unwrap() > 0.0);
    }
}
