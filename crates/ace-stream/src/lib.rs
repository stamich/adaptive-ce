//! Bounded-memory streaming adapters.
//!
//! The container starts with a fixed header containing the original size and block count, so the
//! non-seekable encoder needs the source size up front. The payload is then processed one
//! independent block at a time without holding the complete input in memory.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod decoder;
mod encoder;
mod limits;

pub use decoder::*;
pub use encoder::*;
pub use limits::*;
