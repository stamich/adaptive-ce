//! Format 1.5 payload gating: a stream of a Format 1.5 mode is readable only when the file
//! declares 1.5, and the writer keeps declaring the lowest version the payloads need.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, AceError};
use ace_engine::AceEngine;

/// File header (32) + fixed block header (32): where the payload of a plain first block starts.
const FIRST_PAYLOAD: usize = 64;

/// A counter file whose only block is a plain NUM1 payload, declared as `minor`.
fn numeric_file(minor: u8) -> Vec<u8> {
    let input: Vec<u8> = (0u32..20_000).flat_map(u32::to_le_bytes).collect();
    let mut bytes = AceEngine::new(AceConfig::default())
        .unwrap()
        .compress(&input)
        .unwrap();
    assert_eq!(bytes[5], 3, "plain NUM1 stays Format 1.3");
    assert_eq!(&bytes[FIRST_PAYLOAD..FIRST_PAYLOAD + 4], b"NUM1");
    bytes[5] = minor;
    let crc = ace_format::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// Existing modes keep the minimal version: nothing the 0.6.0 pipeline wrote changes.
#[test]
fn modes_one_to_three_keep_format_1_3() {
    let bytes = numeric_file(3);
    let input: Vec<u8> = (0u32..20_000).flat_map(u32::to_le_bytes).collect();
    assert_eq!(
        AceEngine::default_engine().decompress(&bytes).unwrap(),
        input
    );
}

/// A mode 4 payload in a file that declares less than 1.5 is an unsupported-version error, not a
/// decoder error; in a 1.5 file the version gate passes and the payload is judged on its merits.
#[test]
fn mode_four_needs_a_format_1_5_file() {
    for minor in [3u8, 4] {
        let mut bytes = numeric_file(minor);
        bytes[FIRST_PAYLOAD + 5] = 4;
        let error = AceEngine::default_engine().decompress(&bytes).unwrap_err();
        assert!(
            matches!(error, AceError::UnsupportedVersion { major: 1, minor: m } if m == minor),
            "1.{minor}: {error:?}"
        );
    }
    let mut bytes = numeric_file(5);
    bytes[FIRST_PAYLOAD + 5] = 4;
    let error = AceEngine::default_engine().decompress(&bytes).unwrap_err();
    assert!(
        !matches!(error, AceError::UnsupportedVersion { .. }),
        "{error:?}"
    );
}
