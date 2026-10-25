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

use ace::Config;
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
            storage,
            dictionary,
            external_dictionary,
        } => commands::compress(
            &input,
            &output,
            commands::storage_options(
                Config::builder()
                    .threads(threads)
                    .profile(profile.into())
                    .block_size_policy(block_policy.into())
                    .access_hint(access_hint.into())
                    .float_specialization(!disable_float),
                &storage,
                dictionary.as_deref(),
                external_dictionary,
            )?
            .build()?,
        ),
        Command::CompressStream {
            input,
            output,
            profile,
            disable_float,
            storage,
            dictionary,
        } => commands::compress_stream(
            &input,
            &output,
            commands::storage_options(
                Config::builder()
                    .profile(profile.into())
                    .float_specialization(!disable_float),
                &storage,
                dictionary.as_deref(),
                true,
            )?
            .build()?,
        ),
        Command::Train {
            output,
            samples,
            size,
            chunk_size,
        } => commands::train(&output, &samples, size, chunk_size),
        Command::Decompress {
            input,
            output,
            dictionary,
        } => commands::decompress(&input, &output, &dictionary),
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
        Command::Verify { input, dictionary } => commands::verify(&input, &dictionary),
        Command::DecodeBlock {
            input,
            block_id,
            output,
            dictionary,
        } => commands::decode_block(&input, block_id, &output, &dictionary),
        Command::ReadRange {
            input,
            offset,
            length,
            output,
            dictionary,
        } => commands::read_range(&input, offset, length, &output, &dictionary),
        Command::Version => commands::version(),
    }
}
