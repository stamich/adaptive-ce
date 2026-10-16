//! A file of a newer format (ACE 0.6.0 forward-compatibility contract): every reader rejects it
//! at the file header with `UnsupportedVersion`, and the message names the ACE release that can
//! read it (1.5 is written by ACE 0.6.1 and newer).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, AceError, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};

/// A valid indexed file whose header is patched to declare `major.minor` (CRC recomputed).
fn forged(major: u8, minor: u8) -> Vec<u8> {
    let input: Vec<u8> = (0u32..20_000).flat_map(u32::to_le_bytes).collect();
    let engine = AceEngine::new(AceConfig::default()).unwrap();
    let mut bytes = engine.compress(&input).unwrap();
    bytes[4] = major;
    bytes[5] = minor;
    let crc = ace_format::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// The outcome of every reader on `bytes`.
fn outcomes(bytes: &[u8]) -> Vec<(&'static str, AceError)> {
    let engine = AceEngine::default_engine();
    let mut out = vec![
        ("decompress", engine.decompress(bytes).unwrap_err()),
        (
            "decompress_into",
            engine.decompress_into(bytes, &mut Vec::new()).unwrap_err(),
        ),
        (
            "decompress_from",
            engine
                .decompress_from(Cursor::new(bytes), Vec::new())
                .unwrap_err(),
        ),
    ];
    match AceIndexedDecoder::open(Cursor::new(bytes), DecodeLimits::default()) {
        Err(error) => out.push(("indexed open", error)),
        Ok(_) => panic!("the indexed reader accepted a newer format"),
    }
    out
}

#[test]
fn format_1_5_names_the_release_that_reads_it() {
    for (reader, error) in outcomes(&forged(1, 5)) {
        assert!(
            matches!(error, AceError::UnsupportedVersion { major: 1, minor: 5 }),
            "{reader}: {error:?}"
        );
        let message = error.to_string();
        assert!(message.contains("1.5"), "{reader}: {message}");
        assert!(
            message.contains("ACE 0.6.1 or newer"),
            "{reader}: {message}"
        );
        assert!(
            message.contains("supports up to 1.4"),
            "{reader}: {message}"
        );
    }
}

#[test]
fn unknown_newer_versions_say_so_without_inventing_a_release() {
    for (major, minor) in [(1, 6), (1, 255), (2, 0)] {
        for (reader, error) in outcomes(&forged(major, minor)) {
            let message = error.to_string();
            assert!(
                matches!(error, AceError::UnsupportedVersion { .. }),
                "{reader} {major}.{minor}: {message}"
            );
            assert!(
                message.contains("written by a newer ACE release"),
                "{reader} {major}.{minor}: {message}"
            );
            assert!(
                !message.contains("0.6.1"),
                "{reader} {major}.{minor}: {message}"
            );
        }
    }
}

#[test]
fn supported_versions_still_decode() {
    for minor in [3u8, 4] {
        let bytes = forged(1, minor);
        assert!(
            AceEngine::default_engine().decompress(&bytes).is_ok(),
            "1.{minor}"
        );
    }
}

#[test]
fn feature_errors_inside_a_supported_version_carry_no_hint() {
    // A 1.0 header cannot contain a Numeric block: the block-level check reports
    // `UnsupportedVersion` for a version that is not newer, so no "newer ACE" text is added.
    let error = AceError::UnsupportedVersion { major: 1, minor: 2 };
    assert_eq!(error.to_string(), "unsupported ACE format version 1.2");
}
