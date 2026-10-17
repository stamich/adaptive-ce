//! `version`: product version, format support, active backends and build provenance.

use std::io::Write;

use ace_format::{FORMAT_MAJOR, FORMAT_MINOR, FORMAT_MINOR_BASE};
use anyhow::Result;

/// Prints everything a bug report needs to identify the binary and its capabilities.
pub fn version() -> Result<()> {
    let mut out = std::io::stdout().lock();
    let readers: Vec<String> = (0..=FORMAT_MINOR)
        .map(|minor| format!("{FORMAT_MAJOR}.{minor}"))
        .collect();
    writeln!(out, "ace {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(out, "git revision:   {}", env!("ACE_CLI_GIT_REVISION"))?;
    writeln!(
        out,
        "format writer:  {FORMAT_MAJOR}.{FORMAT_MINOR_BASE} | {FORMAT_MAJOR}.4 | \
         {FORMAT_MAJOR}.{FORMAT_MINOR} (minimal version: 1.4 when a block uses TS1, \
         1.5 when a payload uses a Format 1.5 mode)"
    )?;
    writeln!(out, "format readers: {}", readers.join(" "))?;
    writeln!(
        out,
        "codecs:         RAW, LZ, RLE, NUM1 (FOR / Delta / DoD), TS1 (Gorilla f64 / f32, RunDelta)"
    )?;
    writeln!(out, "entropy coders: Huffman, rANS, rANS4x")?;
    writeln!(out, "planner:        V5")?;
    writeln!(
        out,
        "simd backend:   {:?} (ACE_SIMD=scalar forces the portable kernels)",
        ace_simd::selected_backend()
    )?;
    writeln!(out, "crc32c backend: {}", ace_simd::crc32c_backend_name())?;
    writeln!(
        out,
        "build:          {}, {}, {}",
        env!("ACE_CLI_RUSTC"),
        env!("ACE_CLI_TARGET"),
        env!("ACE_CLI_PROFILE")
    )?;
    Ok(())
}
