//! Status codes returned by every fallible function (`docs/NATIVE-INTEROP.md`, rule 1).

use ace::ErrorKind;

/// Result of a fallible call: [`ACE_OK`] or one of the `ACE_ERR_*` values.
pub type AceStatus = i32;

/// The call succeeded.
pub const ACE_OK: AceStatus = 0;
/// An argument is invalid: a null pointer, a bad struct size, a zero block size, a reversed range.
pub const ACE_ERR_INVALID_ARGUMENT: AceStatus = 1;
/// The input does not start with the ACE signature.
pub const ACE_ERR_NOT_ACE_FILE: AceStatus = 2;
/// The input uses a feature this build cannot read (a newer format, an unknown codec).
pub const ACE_ERR_UNSUPPORTED: AceStatus = 3;
/// The data is damaged or hostile.
pub const ACE_ERR_CORRUPT: AceStatus = 4;
/// A checksum does not match.
pub const ACE_ERR_CHECKSUM: AceStatus = 5;
/// A decode limit was exceeded; nothing was allocated for the offending size.
pub const ACE_ERR_LIMIT_EXCEEDED: AceStatus = 6;
/// A block number is outside the file.
pub const ACE_ERR_BLOCK_NOT_FOUND: AceStatus = 7;
/// Reading or writing failed.
pub const ACE_ERR_IO: AceStatus = 8;
/// A Rust panic was caught at the boundary.
pub const ACE_ERR_PANIC: AceStatus = 9;
/// A state the library believes impossible; a bug.
pub const ACE_ERR_INTERNAL: AceStatus = 10;
/// A caller-provided output buffer is too small; the required size is reported.
pub const ACE_ERR_BUFFER_TOO_SMALL: AceStatus = 11;

/// Maps a facade [`ErrorKind`] to its status code.
///
/// The enum is `#[non_exhaustive]`; a kind added by a later release that this crate does not know
/// maps to [`ACE_ERR_INTERNAL`].
#[must_use]
pub fn status_of(kind: ErrorKind) -> AceStatus {
    match kind {
        ErrorKind::InvalidConfig => ACE_ERR_INVALID_ARGUMENT,
        ErrorKind::NotAceFile => ACE_ERR_NOT_ACE_FILE,
        ErrorKind::Unsupported => ACE_ERR_UNSUPPORTED,
        ErrorKind::Corrupt => ACE_ERR_CORRUPT,
        ErrorKind::ChecksumMismatch => ACE_ERR_CHECKSUM,
        ErrorKind::LimitExceeded => ACE_ERR_LIMIT_EXCEEDED,
        ErrorKind::BlockNotFound => ACE_ERR_BLOCK_NOT_FOUND,
        ErrorKind::Io => ACE_ERR_IO,
        _ => ACE_ERR_INTERNAL,
    }
}
