//! Reversible byte-level preprocessing transforms.
//!
//! [`apply_transform`] / [`invert_transform`] dispatch on [`ace_core::TransformId`].
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod delta;
mod dispatch;

pub use delta::*;
pub use dispatch::*;
