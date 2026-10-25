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
//! # Quick start
//!
//! ```
//! use std::io::Cursor;
//! use ace::{Config, DecodeLimits, Engine, Profile};
//!
//! # let input = b"2026-10-16,sensor-1,42\n".repeat(20_000);
//! // Compress and decompress in memory.
//! let engine = Engine::new(Config::builder().profile(Profile::Balanced).build()?)?;
//! let (packed, stats) = engine.compress_with_stats(&input)?;
//! assert_eq!(engine.decompress(&packed)?, input);
//! println!("{} -> {} bytes, ratio {:.1}", stats.input_bytes, stats.output_bytes, stats.ratio());
//!
//! // Read a byte range without decompressing the rest.
//! let mut reader = engine.open_range_reader(Cursor::new(&packed))?;
//! assert_eq!(reader.read_range(4_000..4_100)?, input[4_000..4_100]);
//!
//! // Decode untrusted input under explicit limits.
//! let strict = Engine::default()
//!     .with_decode_limits(DecodeLimits::builder().max_output_size(64 << 20).build());
//! let restored = strict.decompress(&packed)?;
//! # assert_eq!(restored, input);
//! # Ok::<(), ace::Error>(())
//! ```
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
mod schema;
mod stats;

pub use config::{AccessHint, BlockSizePolicy, Config, ConfigBuilder, Profile};
pub use engine::Engine;
pub use error::{Error, ErrorKind, Result};
#[cfg(feature = "explain")]
pub use explain::BlockReport;
pub use limits::{DecodeLimits, DecodeLimitsBuilder};
pub use range::{RangeReader, RangeStats};
pub use schema::{Column, ColumnKind, Schema};
pub use stats::Stats;

/// Conversions between the facade types and the internal crates, for the test suite of this
/// crate (feature `internal-testing`; not part of the API).
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod internal {
    use ace_core::{AceConfig, AceError, CompressionStats, DecodeLimits as CoreLimits};

    use crate::{Config, DecodeLimits, Error, Stats};

    /// The facade error for an internal error.
    pub fn error_from_core(error: AceError) -> Error {
        Error::from_core(error)
    }

    /// The internal configuration a [`Config`] resolves to.
    pub fn config_core(config: &Config) -> &AceConfig {
        config.core()
    }

    /// The internal limits a [`DecodeLimits`] resolves to.
    pub fn limits_core(limits: &DecodeLimits) -> &CoreLimits {
        limits.core()
    }

    /// The facade statistics for internal statistics.
    pub fn stats_from_core(stats: &CompressionStats) -> Stats {
        Stats::from_core(stats)
    }
}
