//! End-to-end tests of the `ace` binary (ACE 0.5.1 CLI hardening, 0.6.0 forward compatibility): round trips, `ace version`,
//! invalid input of every kind (exit status 1 with a message, never a panic) and a closed
//! standard output (exit status 0, no panic).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The `ace` binary built by Cargo for this test run.
fn ace() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ace"))
}

/// A fresh scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ace-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 300 KiB of compressible text (two blocks).
fn sample(dir: &Path) -> PathBuf {
    let path = dir.join("input.txt");
    let text: Vec<u8> = b"ace cli test line: adaptive compression engine\n"
        .iter()
        .copied()
        .cycle()
        .take(300 * 1024)
        .collect();
    std::fs::write(&path, text).unwrap();
    path
}

/// Runs `ace args…` and returns its output.
fn run(args: &[&str]) -> Output {
    ace().args(args).output().unwrap()
}

/// `ace args…` must fail with exit status 1, an `Error:` message and no panic.
fn assert_clean_failure(args: &[&str], expected: &str) {
    let output = run(args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
    assert!(stderr.contains("Error:"), "{args:?}: {stderr}");
    assert!(
        stderr.contains(expected),
        "{args:?}: expected {expected:?} in {stderr}"
    );
    assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
}

/// Path as `&str`.
fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// compress → verify → inspect → decompress → decode-block → read-range, both writers.
#[test]
fn round_trip_through_every_command() {
    let dir = scratch("roundtrip");
    let input = sample(&dir);
    let original = std::fs::read(&input).unwrap();
    let (ace_file, stream_file, out) = (dir.join("a.ace"), dir.join("s.ace"), dir.join("out"));
    for args in [
        vec!["compress", s(&input), s(&ace_file), "--profile", "dense"],
        vec!["compress-stream", s(&input), s(&stream_file)],
        vec!["verify", s(&ace_file)],
        vec!["inspect", s(&ace_file), "--blocks"],
        vec!["decompress", s(&ace_file), s(&out)],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(std::fs::read(&out).unwrap(), original);
    assert!(run(&["decode-block", s(&ace_file), "1", s(&out)])
        .status
        .success());
    assert_eq!(std::fs::read(&out).unwrap(), original[256 * 1024..]);
    assert!(
        run(&["read-range", s(&stream_file), "1000", "5000", s(&out)])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&out).unwrap(), original[1000..6000]);
}

/// `ace version` reports the version, the minimal-version writer and readers 1.0 – 1.4.
#[test]
fn version_reports_formats_and_backends() {
    let output = run(&["version"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with(&format!("ace {}\n", env!("CARGO_PKG_VERSION"))),
        "{text}"
    );
    for needle in [
        "git revision:",
        "format writer:  1.3 | 1.4 | 1.5 | 1.6",
        "format readers: 1.0 1.1 1.2 1.3 1.4 1.5 1.6",
        "simd backend:",
        "crc32c backend:",
        "build:",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
}

/// Missing files, directories, unreadable or corrupt containers and bad arguments.
#[test]
fn invalid_input_fails_cleanly() {
    let dir = scratch("invalid");
    let input = sample(&dir);
    let missing = dir.join("missing.ace");
    let out = dir.join("out");
    assert_clean_failure(&["decompress", s(&missing), s(&out)], "reading");
    assert_clean_failure(&["compress", s(&missing), s(&out)], "reading");
    assert_clean_failure(&["inspect", s(&dir)], "reading");
    assert_clean_failure(&["compress", s(&dir), s(&out)], "reading");
    // A plain text file is not an ACE container.
    assert_clean_failure(&["decompress", s(&input), s(&out)], "magic");
    assert_clean_failure(&["verify", s(&input)], "magic");
    // A truncated and a bit-flipped container.
    let ace_file = dir.join("a.ace");
    assert!(run(&["compress", s(&input), s(&ace_file)]).status.success());
    let bytes = std::fs::read(&ace_file).unwrap();
    let truncated = dir.join("truncated.ace");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
    assert_clean_failure(&["decompress", s(&truncated), s(&out)], "Error");
    let mut flipped = bytes.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0x40;
    let flipped_file = dir.join("flipped.ace");
    std::fs::write(&flipped_file, &flipped).unwrap();
    assert_clean_failure(&["verify", s(&flipped_file)], "Error");
    // Indexed access out of range.
    assert_clean_failure(&["decode-block", s(&ace_file), "99", s(&out)], "block 99");
    assert_clean_failure(
        &["read-range", s(&ace_file), "0", "999999999", s(&out)],
        "Error",
    );
}

/// The output may never name the input file (directly or through `.` components).
#[test]
fn output_equal_to_input_is_refused() {
    let dir = scratch("same");
    let input = sample(&dir);
    let original = std::fs::read(&input).unwrap();
    let alias = dir.join(".").join("input.txt");
    for command in ["compress", "compress-stream", "decompress"] {
        assert_clean_failure(&[command, s(&input), s(&alias)], "refusing to overwrite");
    }
    assert_eq!(
        std::fs::read(&input).unwrap(),
        original,
        "input was modified"
    );
}

/// Unknown profiles, non-numeric threads and unknown commands are usage errors (status 2).
#[test]
fn invalid_arguments_are_usage_errors() {
    let dir = scratch("args");
    let input = sample(&dir);
    let out = dir.join("out.ace");
    for args in [
        vec!["compress", s(&input), s(&out), "--profile", "turbo"],
        vec!["compress", s(&input), s(&out), "--threads", "many"],
        vec!["compress", s(&input)],
        vec!["frobnicate"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}

/// An output directory without write permission (skipped when running as root, which
/// ignores permissions).
#[cfg(unix)]
#[test]
fn unwritable_output_fails_cleanly() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("perm");
    let input = sample(&dir);
    let locked = dir.join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(locked.join("probe"), b"x").is_ok() {
        return; // root: permissions are not enforced
    }
    assert_clean_failure(
        &["compress", s(&input), s(&locked.join("out.ace"))],
        "writing",
    );
}

/// `ace explain … | head` must not panic on the closed pipe: status 0, empty stderr.
#[test]
fn closed_stdout_is_a_quiet_success() {
    let dir = scratch("pipe");
    let input = sample(&dir);
    let mut child = ace()
        .args(["explain", s(&input)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close the read end before `explain` has produced its first line (it analyses the whole
    // block first), so its first write meets EPIPE regardless of the pipe buffer size.
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status {:?}: {stderr}",
        output.status
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// A file written by a newer ACE (format 1.7) is refused by every command that reads it, and
/// the message says that a newer release wrote it.
#[test]
fn newer_format_is_refused_with_a_clear_message() {
    let dir = scratch("newer");
    let input = sample(&dir);
    let ace_file = dir.join("a.ace");
    assert!(run(&["compress", s(&input), s(&ace_file)]).status.success());
    let mut bytes = std::fs::read(&ace_file).unwrap();
    bytes[5] = 7;
    let crc = ace_format::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    let newer = dir.join("newer.ace");
    std::fs::write(&newer, &bytes).unwrap();
    let out = dir.join("out");
    for args in [
        vec!["decompress", s(&newer), s(&out)],
        vec!["verify", s(&newer)],
        vec!["inspect", s(&newer)],
    ] {
        assert_clean_failure(&args, "written by a newer ACE release");
    }
}

// ---------------------------------------------------------------------------------------------
// Storage engine (ACE 0.8.0): schemas, segments and dictionaries.
// ---------------------------------------------------------------------------------------------

/// A table of 24-byte records (`time u64 | price f64 | id u32 | flags [u8; 4]`).
fn table(dir: &Path) -> PathBuf {
    let path = dir.join("table.bin");
    let mut bytes = Vec::new();
    for i in 0..40_000usize {
        bytes.extend_from_slice(&(1_700_000_000_000u64 + i as u64 * 100).to_le_bytes());
        bytes.extend_from_slice(&(20.0f64 + (i % 50) as f64 * 0.25).to_le_bytes());
        bytes.extend_from_slice(&((i % 40) as u32).to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 0, (i % 2) as u8]);
    }
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Text of one JSON-like record.
fn json_record(i: usize) -> String {
    format!(
        "{{\"event\":\"temperature_reading\",\"device_id\":{i},\"unit\":\"celsius\",\
         \"value\":{},\"firmware\":\"release-2.7.1\"}}\n",
        (i * 7919) % 400
    )
}

/// Writes one sample file per record into `dir/samples` and returns their paths.
fn sample_files(dir: &Path, count: usize) -> Vec<PathBuf> {
    let samples = dir.join("samples");
    std::fs::create_dir_all(&samples).unwrap();
    (1000..1000 + count)
        .map(|i| {
            let path = samples.join(format!("s{i}.json"));
            std::fs::write(&path, json_record(i)).unwrap();
            path
        })
        .collect()
}

#[test]
fn a_schema_stores_the_table_by_columns() {
    let dir = scratch("schema");
    let input = table(&dir);
    let (rowwise, columnar, out) = (dir.join("r.ace"), dir.join("c.ace"), dir.join("out"));
    assert!(run(&["compress", s(&input), s(&rowwise)]).status.success());
    let output = run(&["compress", s(&input), s(&columnar), "--schema", "u64,f64,u32,b4"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("columnar=") && !text.contains("columnar=0 "), "{text}");
    assert!(text.contains("format=1.6"), "{text}");
    assert!(std::fs::metadata(&columnar).unwrap().len() < std::fs::metadata(&rowwise).unwrap().len());
    for args in [
        vec!["verify", s(&columnar)],
        vec!["decompress", s(&columnar), s(&out)],
    ] {
        assert!(run(&args).status.success(), "{args:?}");
    }
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&input).unwrap());
    // The streaming path writes the same bytes.
    let streamed = dir.join("s.ace");
    assert!(run(&["compress-stream", s(&input), s(&streamed), "--schema", "u64,f64,u32,b4", "--profile", "balanced"])
        .status
        .success());
    assert!(run(&["decompress", s(&streamed), s(&out)]).status.success());
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&input).unwrap());
}

#[test]
fn schema_errors_are_clean() {
    let dir = scratch("schema-errors");
    let input = sample(&dir);
    let out = dir.join("out.ace");
    assert_clean_failure(&["compress", s(&input), s(&out), "--schema", "u64,q9"], "unknown schema field");
    assert_clean_failure(&["compress", s(&input), s(&out), "--schema", "b0"], "unknown schema field");
    assert_clean_failure(&["compress", s(&input), s(&out), "--schema", ""], "unknown schema field");
}

#[test]
fn segments_are_accepted() {
    let dir = scratch("segments");
    let input = sample(&dir);
    let (packed, out) = (dir.join("a.ace"), dir.join("out"));
    assert!(run(&["compress", s(&input), s(&packed), "--segments"]).status.success());
    assert!(run(&["decompress", s(&packed), s(&out)]).status.success());
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&input).unwrap());
}

#[test]
fn a_dictionary_is_trained_embedded_and_external() {
    let dir = scratch("dictionary");
    let samples = sample_files(&dir, 150);
    let dictionary = dir.join("dict.bin");
    let mut args = vec!["train", s(&dictionary), "--size", "1024"];
    args.extend(samples.iter().map(|path| s(path)));
    let trained = run(&args);
    assert!(trained.status.success(), "{}", String::from_utf8_lossy(&trained.stderr));
    assert!(String::from_utf8_lossy(&trained.stdout).contains("trained dictionary"));
    let size = std::fs::metadata(&dictionary).unwrap().len();
    assert!(size > 0 && size <= 1024, "{size}");

    // Small blocks: the first bytes of every block are where a dictionary helps.
    let input = dir.join("records.json");
    let text: String = (0..6000).map(json_record).collect();
    std::fs::write(&input, &text).unwrap();
    let (plain, embedded, external, streamed, out) = (
        dir.join("p.ace"),
        dir.join("e.ace"),
        dir.join("x.ace"),
        dir.join("s.ace"),
        dir.join("out"),
    );
    let common = ["--block-size", "4096", "--threads", "1"];
    let compress = |output: &Path, extra: &[&str]| {
        let mut args = vec!["compress", s(&input), s(output)];
        args.extend(common);
        args.extend(extra);
        let result = run(&args);
        assert!(result.status.success(), "{args:?}: {}", String::from_utf8_lossy(&result.stderr));
        String::from_utf8_lossy(&result.stdout).into_owned()
    };
    compress(&plain, &[]);
    let text_out = compress(&embedded, &["--dictionary", s(&dictionary)]);
    assert!(text_out.contains("format=1.6") && !text_out.contains("dictionary=0 "), "{text_out}");
    compress(&external, &["--dictionary", s(&dictionary), "--external-dictionary"]);
    let (plain_len, embedded_len, external_len) = (
        std::fs::metadata(&plain).unwrap().len(),
        std::fs::metadata(&embedded).unwrap().len(),
        std::fs::metadata(&external).unwrap().len(),
    );
    assert!(external_len < embedded_len && embedded_len < plain_len, "{plain_len} {embedded_len} {external_len}");

    // The embedded file is self-contained and shows its dictionary.
    assert!(run(&["decompress", s(&embedded), s(&out)]).status.success());
    assert_eq!(std::fs::read(&out).unwrap(), text.as_bytes());
    let inspect = run(&["inspect", s(&embedded), "--blocks"]);
    let listing = String::from_utf8_lossy(&inspect.stdout);
    assert!(listing.contains("(embedded)") && listing.contains("scope=File"), "{listing}");
    // The external file needs the dictionary, and says so when it is missing.
    assert_clean_failure(&["decompress", s(&external), s(&out)], "dictionary");
    for args in [
        vec!["decompress", s(&external), s(&out), "--dictionary", s(&dictionary)],
        vec!["verify", s(&external), "--dictionary", s(&dictionary)],
        vec!["decode-block", s(&external), "1", s(&out), "--dictionary", s(&dictionary)],
        vec!["read-range", s(&external), "10", "5000", s(&out), "--dictionary", s(&dictionary)],
    ] {
        let result = run(&args);
        assert!(result.status.success(), "{args:?}: {}", String::from_utf8_lossy(&result.stderr));
    }
    assert_eq!(std::fs::read(&out).unwrap(), &text.as_bytes()[10..5010]);
    // `compress-stream` takes an external dictionary and writes the bytes `compress` writes.
    let result = run(&[
        "compress-stream", s(&input), s(&streamed), "--block-size", "4096",
        "--dictionary", s(&dictionary),
    ]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(run(&["decompress", s(&streamed), s(&out), "--dictionary", s(&dictionary)])
        .status
        .success());
    assert_eq!(std::fs::read(&out).unwrap(), text.as_bytes());
}

#[test]
fn train_errors_are_clean() {
    let dir = scratch("train-errors");
    let one = dir.join("one.txt");
    std::fs::write(&one, b"just one short sample").unwrap();
    let out = dir.join("dict.bin");
    assert_clean_failure(&["train", s(&out), s(&one)], "samples");
    assert_clean_failure(&["train", s(&out), s(&one), "--chunk-size", "0"], "chunk-size");
    assert_clean_failure(&["train", s(&out), s(&dir.join("missing.txt"))], "reading");
    assert_clean_failure(&["compress", s(&one), s(&out), "--dictionary", s(&dir.join("none.bin"))], "reading");
}
