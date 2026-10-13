//! Reversible byte-level preprocessing transforms.
//!
//! [`apply_transform`] / [`invert_transform`] dispatch on [`ace_core::TransformId`].
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod delta;
mod dispatch;

pub use delta::*;
pub use dispatch::*;
