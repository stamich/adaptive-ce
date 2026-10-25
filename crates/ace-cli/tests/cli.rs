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
