//! Entropy coders used as the final stage of ACE physical compression plans.
//!
//! [`encode_entropy`] / [`decode_entropy`] dispatch on [`ace_core::EntropyCodecId`].
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod dispatch;
mod huffman;
mod rans;
mod rans4x;

pub use dispatch::*;
pub use huffman::*;
pub use rans::*;
pub use rans4x::*;
