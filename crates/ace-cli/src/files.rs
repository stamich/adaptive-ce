//! File helpers that attach the path to every I/O error.

use std::fs;

use anyhow::{bail, Context, Result};

/// Reads a whole file.
pub fn read(path: &str) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("reading {path}"))
}

/// Writes a whole file.
pub fn write(path: &str, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).with_context(|| format!("writing {path}"))
}

/// Opens a file for reading.
pub fn open(path: &str) -> Result<fs::File> {
    fs::File::open(path).with_context(|| format!("opening {path}"))
}

/// Creates (or truncates) a file for writing.
pub fn create(path: &str) -> Result<fs::File> {
    fs::File::create(path).with_context(|| format!("creating {path}"))
}

/// Refuses an `output` that names the same file as `input` (after resolving `.` / `..` and
/// symbolic links): writing it would destroy the input before it is fully processed.
pub fn ensure_distinct(input: &str, output: &str) -> Result<()> {
    let (Ok(input_path), Ok(output_path)) = (fs::canonicalize(input), fs::canonicalize(output))
    else {
        // `output` does not exist yet (or `input` does not, which the read reports).
        return Ok(());
    };
    if input_path == output_path {
        bail!("refusing to overwrite the input file {input} (output names the same file)");
    }
    Ok(())
}
