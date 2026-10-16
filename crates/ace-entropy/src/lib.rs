//! Entropy coders used as the final stage of ACE physical compression plans.
//!
//! [`encode_entropy`] / [`decode_entropy`] dispatch on [`ace_core::EntropyCodecId`].
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod dispatch;
mod huffman;
mod rans;
mod rans4x;

pub use dispatch::*;
pub use huffman::*;
pub use rans::*;
pub use rans4x::*;
