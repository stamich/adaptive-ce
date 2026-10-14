//! Scalar integer transforms and bit packing used by ACE numeric compression.
//!
//! The crate is format-agnostic: it operates on integer slices and returns deterministic byte
//! vectors. NUM1 framing lives in `ace-codecs`.
//!
//! | module            | responsibility                                              |
//! |-------------------|-------------------------------------------------------------|
//! | `lane`            | [`Lane`]: the one abstraction over u16 / u32 / u64 lanes     |
//! | `zigzag`          | signed ⇄ unsigned ZigZag mapping                            |
//! | `width`           | bit-width helpers                                           |
//! | `scalar`          | fixed-width little-endian bit packing                       |
//! | `bitstream`       | LSB-first bit I/O: [`BitWriter`] / [`BitReader`] (TS1)       |
//! | `delta`           | first-order wrapping deltas                                 |
//! | `delta_of_delta`  | second-order wrapping deltas                                |
//! | `for_codec`       | frame-of-reference offsets                                  |
//!
//! All arithmetic is wrapping: every transform is a bijection modulo `2^lane_bits`.
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod bitstream;
mod delta;
mod delta_of_delta;
mod for_codec;
mod lane;
mod scalar;
mod width;
mod zigzag;

pub use bitstream::{BitReader, BitWriter};
pub use delta::*;
pub use delta_of_delta::*;
pub use for_codec::*;
pub use lane::*;
pub use scalar::*;
pub use width::*;
pub use zigzag::*;
