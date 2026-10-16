//! The error type of the public API.

use std::fmt;

use ace_core::AceError;

/// What went wrong, in terms a caller can act on.
///
/// New kinds may be added in a minor release, so match with a wildcard arm. The kinds are the
/// stable contract; the message of an [`Error`] is for people and may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A `Config` or an argument is invalid (for example a zero block size).
    InvalidConfig,
    /// The input does not start with the ACE signature: it is not an ACE file.
    NotAceFile,
    /// A valid-looking file uses something this build cannot read: a newer format version
    /// (the message names the ACE release that reads it), an unknown feature flag, codec,
    /// entropy coder or transform, or a missing dictionary.
    Unsupported,
    /// The data is damaged or hostile: truncated, inconsistent lengths, an invalid compressed
    /// stream, or input that ends early.
    Corrupt,
    /// A checksum does not match (a block, or the block index). [`Error::block_id`] names the
    /// block when it is known.
    ChecksumMismatch,
    /// A `DecodeLimits` limit or a streaming limit was exceeded; nothing
    /// was allocated for the offending size.
    LimitExceeded,
    /// A block number is outside the file. [`Error::block_id`] holds it.
    BlockNotFound,
    /// Reading or writing failed; [`std::error::Error::source`] holds the I/O error.
    Io,
}

/// Failure of an ACE operation.
///
/// The [kind](Self::kind) says what to do about it, [`Display`](fmt::Display) says what happened.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    block_id: Option<u64>,
    source: Option<std::io::Error>,
}

/// `Result` with the ACE [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// What kind of failure this is.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The block the failure concerns, for [`ErrorKind::ChecksumMismatch`] and
    /// [`ErrorKind::BlockNotFound`] when it is known.
    #[must_use]
    pub fn block_id(&self) -> Option<u64> {
        self.block_id
    }

    /// An error of `kind` with `message`.
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            block_id: None,
            source: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: error.to_string(),
            block_id: None,
            source: Some(error),
        }
    }
}

impl Error {
    /// Maps the internal error of the engine crates onto the public one.
    pub(crate) fn from_core(error: AceError) -> Self {
        let message = error.to_string();
        let (kind, block_id) = match &error {
            AceError::InvalidMagic => (ErrorKind::NotAceFile, None),
            AceError::UnsupportedVersion { .. }
            | AceError::UnsupportedFeature(_)
            | AceError::UnsupportedCodec(_)
            | AceError::UnsupportedEntropyCodec(_)
            | AceError::UnsupportedTransform(_)
            | AceError::MissingDictionary(_) => (ErrorKind::Unsupported, None),
            AceError::ChecksumMismatch(block) => (ErrorKind::ChecksumMismatch, Some(*block)),
            AceError::IndexChecksumMismatch => (ErrorKind::ChecksumMismatch, None),
            AceError::ResourceLimitExceeded(_) => (ErrorKind::LimitExceeded, None),
            AceError::InvalidConfig(_) => (ErrorKind::InvalidConfig, None),
            AceError::BlockNotFound(block) => (ErrorKind::BlockNotFound, Some(*block)),
            AceError::Io(_) => {
                return match error {
                    // The input ended inside the file: it is truncated, not an I/O failure.
                    AceError::Io(io) if io.kind() == std::io::ErrorKind::UnexpectedEof => Self {
                        kind: ErrorKind::Corrupt,
                        message: format!("file is truncated: {io}"),
                        block_id: None,
                        source: Some(io),
                    },
                    AceError::Io(io) => Self::from(io),
                    _ => Self::new(ErrorKind::Io, message),
                };
            }
            // Malformed, InvalidLzReference, InvalidHuffman, InvalidRans, InvalidTimeSeries and
            // every error added later: the data cannot be decoded.
            _ => (ErrorKind::Corrupt, None),
        };
        Self {
            kind,
            message,
            block_id,
            source: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test
mod tests {
    use super::*;

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
            let error = Error::from_core(internal);
            assert_eq!(error.kind(), kind, "{message}");
            assert_eq!(error.block_id(), block, "{message}");
            assert_eq!(error.to_string(), message);
            assert!(std::error::Error::source(&error).is_none());
        }
    }

    #[test]
    fn io_errors_keep_their_source() {
        let io = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe closed");
        let error = Error::from_core(AceError::Io(io));
        assert_eq!(error.kind(), ErrorKind::Io);
        assert!(error.to_string().contains("pipe closed"));
        let source = std::error::Error::source(&error).expect("io source");
        assert!(source.to_string().contains("pipe closed"));
        let direct = Error::from(std::io::Error::other("boom"));
        assert_eq!(direct.kind(), ErrorKind::Io);
    }

    #[test]
    fn newer_format_message_names_the_release() {
        let error = Error::from_core(AceError::UnsupportedVersion { major: 1, minor: 5 });
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert!(error.to_string().contains("ACE 0.6.1 or newer"), "{error}");
    }
}
