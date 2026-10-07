//! `ace` command-line tool.
//!
//! * `args` — clap argument model.
//! * `commands` — one module per command family (compress, decompress/verify/random access,
//!   inspect, explain).
//! * `files` — file I/O helpers with contextual error messages.

mod args;
mod commands;
mod files;

use ace_core::AceConfig;
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
            disable_float,
        } => commands::compress(
            &input,
            &output,
            AceConfig {
                threads,
                profile: profile.into(),
                block_size_policy: block_policy.into(),
                access_hint: access_hint.into(),
                enable_float_specialization: !disable_float,
                ..AceConfig::default()
            },
        ),
        Command::CompressStream {
            input,
            output,
            profile,
            disable_float,
        } => commands::compress_stream(
            &input,
            &output,
            AceConfig {
                profile: profile.into(),
                enable_float_specialization: !disable_float,
                ..AceConfig::default()
            },
        ),
        Command::Decompress { input, output } => commands::decompress(&input, &output),
        Command::Inspect { input, blocks } => commands::inspect(&input, blocks),
        Command::Explain {
            input,
            profile,
            disable_float,
        } => commands::explain(
            &input,
            AceConfig {
                profile: profile.into(),
                enable_float_specialization: !disable_float,
                ..AceConfig::default()
            },
        ),
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
