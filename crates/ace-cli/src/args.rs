//! Command-line argument model (clap) and its mapping onto core configuration enums.

use ace::{AccessHint, BlockSizePolicy, Profile};
use clap::{Parser, Subcommand, ValueEnum};

/// Command-line interface of the Adaptive Compression Engine.
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
    /// Compresses one file into an ACE file (Format 1.3 to 1.6, the lowest the blocks need).
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
        /// Disables the Float lane and TS1 (0.4.6 decisions, always Format 1.3).
        #[arg(long)]
        disable_float: bool,
        #[command(flatten)]
        storage: StorageArgs,
        /// Preset dictionary made by `ace train`; embedded in the file when it pays.
        #[arg(long, value_name = "FILE")]
        dictionary: Option<String>,
        /// Store only the id of `--dictionary`; readers must pass the same file to
        /// `--dictionary` of `decompress`.
        #[arg(long, requires = "dictionary")]
        external_dictionary: bool,
    },
    /// Compresses a file through the bounded-memory streaming path.
    CompressStream {
        input: String,
        output: String,
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
        /// Disables the Float lane and TS1 (0.4.6 decisions, always Format 1.3).
        #[arg(long)]
        disable_float: bool,
        #[command(flatten)]
        storage: StorageArgs,
        /// External preset dictionary made by `ace train` (streams cannot embed one).
        #[arg(long, value_name = "FILE")]
        dictionary: Option<String>,
    },
    /// Trains a preset dictionary from sample files (one sample per file, or per chunk).
    Train {
        /// Destination dictionary file (the raw dictionary bytes).
        output: String,
        /// Sample files; the more alike the data you will compress, the better.
        #[arg(required = true)]
        samples: Vec<String>,
        /// Dictionary size in bytes (64 to 65535).
        #[arg(long, default_value_t = 4096)]
        size: usize,
        /// Cut every sample file into samples of this many bytes (for a few large files).
        #[arg(long, value_name = "BYTES")]
        chunk_size: Option<usize>,
    },
    /// Decompresses one ACE 1.0 – 1.6 file.
    Decompress {
        input: String,
        output: String,
        /// External dictionary file the input was compressed with (repeatable).
        #[arg(long, value_name = "FILE")]
        dictionary: Vec<String>,
    },
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
        /// Disables the Float lane and TS1 (0.4.6 decisions).
        #[arg(long)]
        disable_float: bool,
    },
    /// Fully decodes and checks every block checksum while discarding the reconstructed bytes.
    Verify {
        input: String,
        /// External dictionary file the input was compressed with (repeatable).
        #[arg(long, value_name = "FILE")]
        dictionary: Vec<String>,
    },
    /// Decodes one indexed block without reading preceding blocks.
    DecodeBlock {
        input: String,
        block_id: u64,
        output: String,
        /// External dictionary file the input was compressed with (repeatable).
        #[arg(long, value_name = "FILE")]
        dictionary: Vec<String>,
    },
    /// Decodes only indexed blocks intersecting a logical byte range.
    ReadRange {
        input: String,
        offset: u64,
        length: u64,
        output: String,
        /// External dictionary file the input was compressed with (repeatable).
        #[arg(long, value_name = "FILE")]
        dictionary: Vec<String>,
    },
    /// Prints the version, git revision, supported formats, SIMD / CRC backends and toolchain.
    Version,
}

/// Block size and the storage-engine options (ACE 0.8.0: schemas, segments) shared by `compress`
/// and `compress-stream`.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct StorageArgs {
    /// Block size in bytes (default 256 KiB; rounded to whole records with `--schema`). Small
    /// blocks suit random access and make a dictionary pay off for small inputs.
    #[arg(long, value_name = "BYTES")]
    pub block_size: Option<usize>,
    /// Record layout of a table of fixed-size records, stored column by column. Comma-separated
    /// fields: `u8 u16 u32 u64` (unsigned), `i8 i16 i32 i64` (signed), `f32 f64` (float) and
    /// `bN` (N opaque bytes). Example: `u64,f64,u32,b4` is a 24-byte record.
    #[arg(long, value_name = "FIELDS")]
    pub schema: Option<String>,
    /// Splits blocks that mix kinds of data into segments with their own codecs.
    #[arg(long)]
    pub segments: bool,
}

/// CLI representation of the three deterministic ACE cost profiles.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProfileArg {
    Fast,
    Balanced,
    Dense,
}

/// Maps the CLI spelling onto [`Profile`].
impl From<ProfileArg> for Profile {
    /// Maps CLI profile spelling to the core planner profile.
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Fast => Self::Fast,
            ProfileArg::Balanced => Self::Balanced,
            ProfileArg::Dense => Self::Dense,
        }
    }
}

/// Maps the CLI spelling onto the internal profile used by the diagnostic `explain` command.
impl From<ProfileArg> for ace_core::CompressionProfile {
    /// Maps CLI profile spelling to the internal planner profile.
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
