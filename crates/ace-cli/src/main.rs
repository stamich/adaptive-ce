//! `ace` command-line tool.
//!
//! * `args` — clap argument model.
//! * `commands` — one module per command family (compress, decompress/verify/random access,
//!   inspect, explain).
//! * `files` — file I/O helpers with contextual error messages.

mod args;
mod commands;
mod files;

use anyhow::Result;
use clap::Parser;

use crate::args::{Cli, Command};

/// Parses command-line arguments and dispatches to the requested command.
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Compress {
            input,
            output,
            threads,
            profile,
            block_policy,
            access_hint,
        } => commands::compress(
            &input,
            &output,
            threads,
            profile.into(),
            block_policy.into(),
            access_hint.into(),
        ),
        Command::CompressStream {
            input,
            output,
            profile,
        } => commands::compress_stream(&input, &output, profile.into()),
        Command::Decompress { input, output } => commands::decompress(&input, &output),
        Command::Inspect { input, blocks } => commands::inspect(&input, blocks),
        Command::Explain { input, profile } => commands::explain(&input, profile.into()),
        Command::Verify { input } => commands::verify(&input),
        Command::DecodeBlock {
            input,
            block_id,
            output,
        } => commands::decode_block(&input, block_id, &output),
        Command::ReadRange {
            input,
            offset,
            length,
            output,
        } => commands::read_range(&input, offset, length, &output),
    }
}
