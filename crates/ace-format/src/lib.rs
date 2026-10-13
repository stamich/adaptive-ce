//! ACE container format 1.0–1.4: headers, block index, trailer, checksums and block I/O.
//!
//! * `header` — file and block headers (encode/decode).
//! * `index` — AIDX block index and ACET trailer.
//! * `checksum` — CRC32C used by every integrity check.
//! * `block_io` — the single implementation of "read one serialized block" with limits.
//! * `reader` / `writer` — sequential container reader and writer (the writer also builds
//!   the block index and trailer).
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

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
