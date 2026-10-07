//! Primary structural byte codecs used by ACE (RAW, RLE, LZ, the NUM1 numeric codec and the
//! TS1 time-series codec).
//!
//! [`encode_codec`] / [`decode_codec`] dispatch on [`ace_core::CodecId`]; every codec also
//! exposes its own `*_encode` / `*_decode` functions.

mod dispatch;
mod lz;
mod numeric;
mod raw;
mod rle;
mod time_series;

pub use dispatch::*;
pub use lz::*;
pub use numeric::*;
pub use raw::*;
pub use rle::*;
pub use time_series::*;
