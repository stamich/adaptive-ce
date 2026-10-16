//! The public API of the Adaptive Compression Engine (ACE).
//!
//! ACE is a lossless, schema-free, adaptive block compressor. It looks at every block of the
//! input (text, structured records, integer counters and timestamps, floating-point series),
//! chooses the cheapest reversible pipeline for it with a deterministic cost model, and stores
//! the result in a self-describing container with a block index for random access. The same
//! input and configuration always produce the same bytes, on every machine, thread count and
//! SIMD backend, and every byte read from a file is validated before it is trusted.
//!
//! This crate is the **only supported entry point**. The other crates of the workspace
//! (`ace-core`, `ace-engine`, `ace-format`, …) are internal building blocks: they are not
//! published and may change in any release.
//!
//! # Items
//!
//! | Item | Purpose |
//! |---|---|
//! | [`Config`], [`ConfigBuilder`], [`Profile`], [`BlockSizePolicy`], [`AccessHint`] | compression settings, validated by `build()` |
//! | [`Engine`] | compress, decompress, stream |
//! | [`RangeReader`], [`RangeStats`] | read a byte range without decompressing the rest |
//! | `BlockReport` (feature `explain`) | why the planner chose a pipeline, per block |
//! | [`DecodeLimits`], [`DecodeLimitsBuilder`] | upper bounds enforced while decoding untrusted files |
//! | [`Stats`] | what a compression run did: sizes, ratio, blocks per codec |
//! | [`Error`], [`ErrorKind`], [`Result`] | every failure, classified for the caller |
//!
//! # Stability
//!
//! The public items of this crate follow the compatibility rules of `docs/COMPATIBILITY.md`:
//! within a `0.x` series they change only additively, and enums and structs that may grow are
//! `#[non_exhaustive]`. Output bytes follow the format rules: a patch release never changes
//! them.

mod config;
mod engine;
mod error;
#[cfg(feature = "explain")]
mod explain;
mod limits;
mod range;
mod stats;

pub use config::{AccessHint, BlockSizePolicy, Config, ConfigBuilder, Profile};
pub use engine::Engine;
pub use error::{Error, ErrorKind, Result};
#[cfg(feature = "explain")]
pub use explain::BlockReport;
pub use limits::{DecodeLimits, DecodeLimitsBuilder};
pub use range::{RangeReader, RangeStats};
pub use stats::Stats;
