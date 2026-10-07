//! Command-line argument model (clap) and its mapping onto core configuration enums.

use ace_core::{AccessHint, BlockSizePolicy, CompressionProfile};
use clap::{Parser, Subcommand, ValueEnum};

/// Command-line interface for Adaptive Compression Engine milestone 0.4.
#[derive(Debug, Parser)]
#[command(name = "ace", version, about = "Adaptive Compression Engine")]
pub struct Cli {
    /// ACE operation to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported ACE command-line operations.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compresses one file into ACE Format 1.3 using Planner V4.
    Compress {
        /// Source file.
        input: String,
        /// Destination ACE file.
        output: String,
        /// Worker threads (0 = all logical CPUs).
        #[arg(long, default_value_t = 0)]
        threads: usize,
        /// Cost profile.
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
        /// File-level block-size policy.
        #[arg(long, value_enum, default_value_t = BlockPolicyArg::Fixed)]
        block_policy: BlockPolicyArg,
        /// Expected access pattern (used by `--block-policy auto`).
        #[arg(long, value_enum, default_value_t = AccessHintArg::Balanced)]
        access_hint: AccessHintArg,
    },
    /// Compresses a file through the bounded-memory ACE 0.4 streaming path.
    CompressStream {
        input: String,
        output: String,
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
    },
    /// Decompresses one ACE 1.0/1.1/1.2/1.3 file.
    Decompress { input: String, output: String },
    /// Prints file and per-block physical metadata without decoding payloads.
    Inspect {
        input: String,
        #[arg(long)]
        blocks: bool,
    },
    /// Prints analyzer features and deterministic planner decisions for source data.
    Explain {
        input: String,
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
    },
    /// Fully decodes and checks every block checksum while discarding the reconstructed bytes.
    Verify { input: String },
    /// Decodes one indexed block without reading preceding blocks.
    DecodeBlock {
        input: String,
        block_id: u64,
        output: String,
    },
    /// Decodes only indexed blocks intersecting a logical byte range.
    ReadRange {
        input: String,
        offset: u64,
        length: u64,
        output: String,
    },
}

/// CLI representation of the three deterministic ACE cost profiles.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProfileArg {
    Fast,
    Balanced,
    Dense,
}

/// Maps the CLI spelling onto [`CompressionProfile`].
impl From<ProfileArg> for CompressionProfile {
    /// Maps CLI profile spelling to the core planner profile.
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Fast => Self::Fast,
            ProfileArg::Balanced => Self::Balanced,
            ProfileArg::Dense => Self::Dense,
        }
    }
}

/// CLI representation of the file-level block-size policy.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BlockPolicyArg {
    Fixed,
    Auto,
}

/// Maps the CLI spelling onto [`BlockSizePolicy`].
impl From<BlockPolicyArg> for BlockSizePolicy {
    /// Maps CLI block policy to the core configuration enum.
    fn from(value: BlockPolicyArg) -> Self {
        match value {
            BlockPolicyArg::Fixed => Self::Fixed,
            BlockPolicyArg::Auto => Self::Auto,
        }
    }
}

/// CLI representation of the expected source access pattern.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AccessHintArg {
    Sequential,
    Balanced,
    RandomAccess,
}

/// Maps the CLI spelling onto [`AccessHint`].
impl From<AccessHintArg> for AccessHint {
    /// Maps CLI access hint to the core block-size advisor enum.
    fn from(value: AccessHintArg) -> Self {
        match value {
            AccessHintArg::Sequential => Self::Sequential,
            AccessHintArg::Balanced => Self::Balanced,
            AccessHintArg::RandomAccess => Self::RandomAccess,
        }
    }
}
