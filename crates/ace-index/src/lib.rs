//! Seekable block-index loading and indexed encoded-block access (Format 1.1 and later).
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod index_reader;

pub use index_reader::*;
