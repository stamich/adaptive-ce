//! High-level adaptive compression, decompression and random-access APIs.
//!
//! * `engine` — the [`AceEngine`] façade (compress / decompress / explain).
//! * `block_encoder` — plans and encodes one independent block (planner → payload).
//! * `block_pipeline` — the reversible per-block pipeline framing (entropy metadata prefix,
//!   decode + checksum verification).
//! * `container` — assembles encoded blocks into a container and aggregates statistics.
//! * `indexed` — random-access decoder over the block index.
//! * `chunker`, `explain` — fixed-size block splitting and planner explanations.
//!
//! # Example
//!
//! ```
//! use std::io::Cursor;
//!
//! use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
//! use ace_engine::{AceEngine, AceIndexedDecoder};
//!
//! # fn main() -> Result<(), ace_core::AceError> {
//! let input: Vec<u8> = (0u32..100_000).flat_map(u32::to_le_bytes).collect();
//! let engine = AceEngine::new(AceConfig {
//!     profile: CompressionProfile::Balanced,
//!     ..AceConfig::default()
//! })?;
//!
//! // Whole-file round trip (Format 1.3, deterministic for every thread count).
//! let packed = engine.compress(&input)?;
//! assert_eq!(engine.decompress(&packed)?, input);
//!
//! // Decode into a reusable buffer.
//! let mut buffer = Vec::new();
//! engine.decompress_into(&packed, &mut buffer)?;
//! assert_eq!(buffer, input);
//!
//! // Random access: only the blocks intersecting the range are decoded.
//! let mut reader = AceIndexedDecoder::open(Cursor::new(&packed), DecodeLimits::default())?;
//! assert_eq!(reader.read_range(4_000..4_100)?, &input[4_000..4_100]);
//! # Ok(())
//! # }
//! ```

mod block_encoder;
mod block_pipeline;
mod chunker;
mod container;
mod engine;
mod explain;
mod indexed;

pub use chunker::*;
pub use engine::*;
pub use explain::*;
pub use indexed::*;
