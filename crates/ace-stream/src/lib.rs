//! Bounded-memory streaming adapters.
//!
//! The container starts with a fixed header containing the original size and block count, so the
//! non-seekable encoder needs the source size up front. The payload is then processed one
//! independent block at a time without holding the complete input in memory.
//!
//! **Stability:** part of the public ACE API (with `ace-engine`, `ace-stream` and `ace-core`).
//! 0.5.x releases change it only additively; the 0.6 `ace` facade will re-export it (see
//! `docs/API-INVENTORY.md`).

mod decoder;
mod encoder;
mod limits;

pub use decoder::*;
pub use encoder::*;
pub use limits::*;
