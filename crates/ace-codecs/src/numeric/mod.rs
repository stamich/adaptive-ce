//! Self-describing integer codec ("NUM1") introduced by ACE Format 1.3.
//!
//! A NUM1 payload interprets a block as little-endian unsigned integers of one lane width
//! (16, 32 or 64 bits) plus up to `width - 1` verbatim *tail bytes*. The integers are stored with
//! one of three sub-codecs ([`NumericMode`]):
//!
//! | mode               | stored stream                          | good for                  |
//! |--------------------|----------------------------------------|---------------------------|
//! | `FrameOfReference` | `v[i] - min(v)` bit-packed             | bounded-range values      |
//! | `Delta`            | `zigzag(v[i] - v[i-1])` bit-packed     | counters, drifting values |
//! | `DeltaOfDelta`     | `zigzag(d[i] - d[i-1])` bit-packed     | regular timestamps        |
//!
//! Module map: `mode` (sub-codec ids), `header` (40-byte wire header, parse/serialize),
//! `estimate` (exact, allocation-free size estimation), `encode`, `decode`, `lane_dispatch`
//! (run-time width → compile-time lane).
//!
//! Safety properties: every length is validated with checked arithmetic before allocation, the
//! decoder never panics on arbitrary input, and all arithmetic wraps modulo the lane size.

mod decode;
mod encode;
mod estimate;
mod header;
mod lane_dispatch;
mod mode;

pub use decode::*;
pub use encode::*;
pub use estimate::*;
pub use header::*;
pub use mode::*;

#[cfg(test)]
mod tests;
