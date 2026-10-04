//! ACE format 1.0/1.1/1.2 framing, block-index serialization and corruption checks.

mod header;
mod index;
mod reader;
mod writer;

pub use header::*;
pub use index::*;
pub use reader::*;
pub use writer::*;

/// Computes CRC32C for ACE headers, indexes and reconstructed block payloads.
pub fn checksum(bytes: &[u8]) -> u32 { crc32c::crc32c(bytes) }
