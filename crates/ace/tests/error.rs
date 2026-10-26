//! Tests of `ace` `error` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace::internal::error_from_core;
use ace::*;
use ace_core::AceError;

#[test]
fn kinds_follow_the_internal_error() {
    let cases = [
        (AceError::InvalidMagic, ErrorKind::NotAceFile, None),
        (
            AceError::UnsupportedVersion { major: 1, minor: 5 },
            ErrorKind::Unsupported,
            None,
        ),
        (
            AceError::UnsupportedFeature(8),
            ErrorKind::Unsupported,
            None,
        ),
        (AceError::UnsupportedCodec(99), ErrorKind::Unsupported, None),
        (AceError::MissingDictionary(7), ErrorKind::Unsupported, None),
        (AceError::Malformed("x"), ErrorKind::Corrupt, None),
        (AceError::InvalidLzReference, ErrorKind::Corrupt, None),
        (AceError::InvalidHuffman("x"), ErrorKind::Corrupt, None),
        (AceError::InvalidRans("x"), ErrorKind::Corrupt, None),
        (AceError::InvalidTimeSeries("x"), ErrorKind::Corrupt, None),
        (
            AceError::ChecksumMismatch(3),
            ErrorKind::ChecksumMismatch,
            Some(3),
        ),
        (
            AceError::IndexChecksumMismatch,
            ErrorKind::ChecksumMismatch,
            None,
        ),
        (
            AceError::ResourceLimitExceeded("x"),
            ErrorKind::LimitExceeded,
            None,
        ),
        (AceError::InvalidConfig("x"), ErrorKind::InvalidConfig, None),
        (
            AceError::BlockNotFound(9),
            ErrorKind::BlockNotFound,
            Some(9),
        ),
    ];
    for (internal, kind, block) in cases {
        let message = internal.to_string();
        let error = error_from_core(internal);
        assert_eq!(error.kind(), kind, "{message}");
        assert_eq!(error.block_id(), block, "{message}");
        assert_eq!(error.to_string(), message);
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn io_errors_keep_their_source() {
    let io = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe closed");
    let error = error_from_core(AceError::Io(io));
    assert_eq!(error.kind(), ErrorKind::Io);
    assert!(error.to_string().contains("pipe closed"));
    let source = std::error::Error::source(&error).expect("io source");
    assert!(source.to_string().contains("pipe closed"));
    let direct = Error::from(std::io::Error::other("boom"));
    assert_eq!(direct.kind(), ErrorKind::Io);
}

#[test]
fn newer_format_message_mentions_a_newer_release() {
    let error = error_from_core(AceError::UnsupportedVersion { major: 1, minor: 7 });
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert!(error.to_string().contains("newer ACE release"), "{error}");
}
