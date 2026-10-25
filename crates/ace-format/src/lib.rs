//! ACE container format 1.0–1.5: headers, block index, trailer, checksums and block I/O.
//!
//! * `header` — file and block headers (encode/decode).
//! * `index` — AIDX block index and ACET trailer.
//! * `checksum` — CRC32C used by every integrity check.
//! * `block_io` — the single implementation of "read one serialized block" with limits.
//! * `reader` / `writer` — sequential container reader and writer (the writer also builds
//!   the block index and trailer).
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod block_io;
mod checksum;
mod dictionary_section;
mod header;
mod index;
mod reader;
mod segment;
mod writer;

pub use block_io::*;
pub use checksum::*;
pub use dictionary_section::*;
pub use header::*;
pub use index::*;
pub use reader::*;
pub use segment::*;
pub use writer::*;
