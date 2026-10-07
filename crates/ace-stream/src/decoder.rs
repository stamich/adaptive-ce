//! Streaming decompression.

use std::io::{Read, Write};

use ace_core::{AceResult, DecodeLimits};
use ace_engine::AceEngine;

/// Sequentially decompresses an ACE 1.0–1.3 stream into a writer with explicit limits.
pub fn decompress_stream<R: Read, W: Write>(
    reader: R,
    writer: W,
    limits: DecodeLimits,
) -> AceResult<()> {
    AceEngine::default_engine()
        .with_decode_limits(limits)
        .decompress_from(reader, writer)
}
