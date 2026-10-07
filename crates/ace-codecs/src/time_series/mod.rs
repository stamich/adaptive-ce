//! Self-describing time-series codec ("TS1") introduced by ACE Format 1.4.
//!
//! A TS1 payload interprets a block as little-endian lanes of one width plus up to
//! `lane_bytes - 1` verbatim *tail bytes* and stores the lanes with one of three modes
//! ([`TimeSeriesMode`]):
//!
//! | mode         | lanes          | stored stream                                    | good for                    |
//! |--------------|----------------|--------------------------------------------------|-----------------------------|
//! | `GorillaF64` | IEEE-754 f64   | XOR with the previous value, leading/meaningful-bit windows | slowly changing float series |
//! | `GorillaF32` | IEEE-754 f32   | same with 32-bit parameters                      | f32 sensor data             |
//! | `RunDelta`   | u16 / u32 / u64 | Elias-gamma run lengths of zero deltas + ZigZag deltas | integers that rarely change |
//!
//! Integer Delta / Delta-of-Delta / FOR stay in NUM1; TS1 only adds what NUM1 cannot express.
//!
//! Module map: `mode` (mode ids and lane widths), `header` (28-byte wire header,
//! parse/serialize/validate), `sink` (bit sink shared by encoders and exact size counters),
//! `gamma` (Elias-gamma integers), `gorilla`, `run_delta`, `codec` (public encode / decode /
//! estimate entry points).
//!
//! Safety properties: every length and bit field is validated before allocation or shifting,
//! decoding is bounds-checked through [`ace_bitpack::BitReader`], and the decoder never panics
//! on arbitrary input. Lossless means bit-identical, including NaN payloads and `-0.0`.

mod codec;
mod gamma;
mod gorilla;
mod header;
mod mode;
mod run_delta;
mod sink;

pub use codec::*;
pub use header::*;
pub use mode::*;

#[cfg(test)]
mod tests;
