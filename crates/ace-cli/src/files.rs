//! File helpers that attach the path to every I/O error.

use std::fs;

use anyhow::{Context, Result};

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
