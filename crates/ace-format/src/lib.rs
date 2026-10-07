//! ACE container format 1.0–1.3: headers, block index, trailer, checksums and block I/O.
//!
//! * `header` — file and block headers (encode/decode).
//! * `index` — AIDX block index and ACET trailer.
//! * `checksum` — CRC32C used by every integrity check.
//! * `block_io` — the single implementation of "read one serialized block" with limits.
//! * `reader` / `writer` — sequential container reader and writer (the writer also builds
//!   the block index and trailer).

mod block_io;
mod checksum;
mod header;
mod index;
mod reader;
mod writer;

pub use block_io::*;
pub use checksum::*;
pub use header::*;
pub use index::*;
pub use reader::*;
pub use writer::*;
