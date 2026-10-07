//! High-level adaptive compression, decompression and random-access APIs.
//!
//! * `engine` — the [`AceEngine`] façade (compress / decompress / explain).
//! * `block_encoder` — plans and encodes one independent block (planner → payload).
//! * `block_pipeline` — the reversible per-block pipeline framing (entropy metadata prefix,
//!   decode + checksum verification).
//! * `container` — assembles encoded blocks into a container and aggregates statistics.
//! * `indexed` — random-access decoder over the block index.
//! * `chunker`, `explain` — fixed-size block splitting and planner explanations.

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
