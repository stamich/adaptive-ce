//! Bounded parallel-runtime configuration and reusable per-worker buffers.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod config;
mod scratch;

pub use config::*;
pub use scratch::*;
