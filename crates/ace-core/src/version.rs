//! Format versions this build reads, and the ACE release that introduces each newer one.

use std::fmt;

/// Current major format version.
pub const FORMAT_MAJOR: u8 = 1;
/// Highest minor format version this build reads and writes (1.4 adds the TS1 codec, 1.5 the
/// NUM1 and TS1 mode 4 payloads, 1.6 the segmented and columnar block codecs and the dictionary
/// section).
pub const FORMAT_MINOR: u8 = 6;

/// First minor version that has segmented (`SEG1`) and columnar (`COL1`) blocks and the
/// dictionary section (`ADIC`).
pub const FORMAT_MINOR_STORAGE: u8 = 6;

/// The ACE release that first reads format `major.minor`, for the versions that are known to be
/// planned but not readable by this build (`None` for every other version).
///
/// Format 1.6 was the last planned version and ACE 0.8.0 reads it, so no newer version is
/// announced: an unreadable file is reported as written by "a newer ACE release". An entry is
/// added here when a release plan fixes the number of the next format.
#[must_use]
pub const fn first_release_reading(major: u8, minor: u8) -> Option<&'static str> {
    let _ = (major, minor);
    None
}

/// Explains why format `major.minor` is rejected, for the [`crate::AceError::UnsupportedVersion`]
/// message: it names the ACE release that can read a newer file, and what this build supports.
///
/// Displays nothing when the declared version is not newer than [`FORMAT_MAJOR`].[`FORMAT_MINOR`]
/// (the error is then about a feature the declared version cannot contain).
#[derive(Debug, Clone, Copy)]
pub struct NewerFormatHint {
    /// Major version found in the file.
    pub major: u8,
    /// Minor version found in the file.
    pub minor: u8,
}

impl fmt::Display for NewerFormatHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (self.major, self.minor) <= (FORMAT_MAJOR, FORMAT_MINOR) {
            return Ok(());
        }
        match first_release_reading(self.major, self.minor) {
            Some(release) => write!(f, " (written by ACE {release} or newer")?,
            None => write!(f, " (written by a newer ACE release")?,
        }
        write!(
            f,
            "; this reader supports up to {FORMAT_MAJOR}.{FORMAT_MINOR})"
        )
    }
}
