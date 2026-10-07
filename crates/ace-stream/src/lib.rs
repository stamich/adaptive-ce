//! Bounded-memory streaming adapters.
//!
//! Format 1.3 starts with a fixed header containing the original size and block count, so the
//! non-seekable encoder needs the source size up front. The payload is then processed one
//! independent block at a time without holding the complete input in memory.

mod decoder;
mod encoder;
mod limits;

pub use decoder::*;
pub use encoder::*;
pub use limits::*;
