//! `ace` command-line tool.
//!
//! * `args` — clap argument model.
//! * `commands` — one module per command family (compress, decompress/verify/random access,
//!   inspect, explain).
//! * `files` — file I/O helpers with contextual error messages.
//!
//! Exit status: 0 on success, 1 on any error (printed to stderr as `Error: …`). A closed
//! standard output (`ace explain … | head`) is not an error: the command stops quietly with
//! status 0 instead of panicking on `EPIPE`.

mod args;
mod commands;
mod files;

use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use ace_core::{AceConfig, AceError};
use anyhow::Result;
use clap::Parser;

use crate::args::{Cli, Command};

/// Runs the command; maps a closed standard output to a quiet success.
fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            // Best effort: if stderr is closed as well there is nobody left to tell.
            let _ = writeln!(std::io::stderr(), "Error: {error:?}");
            ExitCode::FAILURE
        }
    }
}

/// True when `error` was caused by writing to a closed pipe (the reader went away).
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .map(|io| io.kind() == ErrorKind::BrokenPipe)
            .or_else(|| match cause.downcast_ref::<AceError>() {
                Some(AceError::Io(io)) => Some(io.kind() == ErrorKind::BrokenPipe),
                _ => None,
            })
            .unwrap_or(false)
    })
}

/// Dispatches to the requested command.
fn run(command: Command) -> Result<()> {
    match command {
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
