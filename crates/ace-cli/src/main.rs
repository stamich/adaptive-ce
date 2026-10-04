use ace_core::{AceConfig, CompressionProfile};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::AceReader;
use ace_stream::{compress_reader_known_size, StreamLimits};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{BufReader, BufWriter, Cursor};

/// Command-line interface for Adaptive Compression Engine milestone 0.3.
#[derive(Debug, Parser)]
#[command(name = "ace", version, about = "Adaptive Compression Engine 0.3")]
struct Cli {
    /// ACE operation to execute.
    #[command(subcommand)]
    command: Command,
}

/// Supported ACE command-line operations.
#[derive(Debug, Subcommand)]
enum Command {
    /// Compresses one file into ACE format 1.2 using the in-memory engine.
    Compress {
        input: String,
        output: String,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
    },
    /// Compresses a file through the bounded-memory ACE 0.3 streaming path.
    CompressStream {
        input: String,
        output: String,
        #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
        profile: ProfileArg,
    },
    /// Decompresses one ACE 1.0/1.1/1.2 file.
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
enum ProfileArg {
    Fast,
    Balanced,
    Dense,
}

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

/// Parses command-line arguments, executes the requested ACE operation and reports failures through `anyhow`.
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Compress {
            input,
            output,
            threads,
            profile,
        } => compress_command(&input, &output, threads, profile.into()),
        Command::CompressStream {
            input,
            output,
            profile,
        } => compress_stream_command(&input, &output, profile.into()),
        Command::Decompress { input, output } => decompress_command(&input, &output),
        Command::Inspect { input, blocks } => inspect_command(&input, blocks),
        Command::Explain { input, profile } => explain_command(&input, profile.into()),
        Command::Verify { input } => verify_command(&input),
        Command::DecodeBlock {
            input,
            block_id,
            output,
        } => decode_block_command(&input, block_id, &output),
        Command::ReadRange {
            input,
            offset,
            length,
            output,
        } => read_range_command(&input, offset, length, &output),
    }
}

/// Compresses a source file and prints the most important telemetry.
fn compress_command(
    input: &str,
    output: &str,
    threads: usize,
    profile: CompressionProfile,
) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("reading {input}"))?;
    let mut config = AceConfig::default();
    config.threads = threads;
    config.profile = profile;
    let engine = AceEngine::new(config)?;
    let (encoded, stats) = engine.compress_with_stats(&data)?;
    fs::write(output, encoded).with_context(|| format!("writing {output}"))?;
    println!("ACE 0.3 compressed {} -> {} bytes ratio={:.3} blocks={} rANS={} rANS4x={} Huffman={} fast-path={} sampled={}", stats.input_bytes, stats.output_bytes, stats.compression_ratio(), stats.block_count, stats.rans_blocks, stats.rans4x_blocks, stats.huffman_blocks, stats.planner_fast_path_blocks, stats.planner_sampled_candidates);
    Ok(())
}

/// Compresses a file with bounded source-block memory and a deterministic format-1.2 index.
fn compress_stream_command(input: &str, output: &str, profile: CompressionProfile) -> Result<()> {
    let source = fs::File::open(input).with_context(|| format!("opening {input}"))?;
    let size = source
        .metadata()
        .with_context(|| format!("stat {input}"))?
        .len();
    let sink = fs::File::create(output).with_context(|| format!("creating {output}"))?;
    let mut config = AceConfig::default();
    config.profile = profile;
    let stats = compress_reader_known_size(
        BufReader::new(source),
        BufWriter::new(sink),
        size,
        config,
        StreamLimits::default(),
    )?;
    println!(
        "ACE 0.3 streamed {} -> {} bytes blocks={} peak_source_buffer={}",
        stats.input_bytes, stats.output_bytes, stats.blocks, stats.peak_source_buffer_bytes
    );
    Ok(())
}

/// Decompresses an ACE file and writes reconstructed bytes.
fn decompress_command(input: &str, output: &str) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("reading {input}"))?;
    let decoded = AceEngine::default_engine().decompress(&data)?;
    fs::write(output, decoded).with_context(|| format!("writing {output}"))?;
    Ok(())
}

/// Parses physical headers and optionally prints every block plan.
fn inspect_command(input: &str, show_blocks: bool) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("reading {input}"))?;
    let limits = ace_core::DecodeLimits::default();
    let mut reader = AceReader::new(Cursor::new(&data), limits);
    let file = reader.read_file_header()?;
    println!(
        "ACE format 1.{} flags=0x{:04x} original={} block_size={} blocks={} indexed={}",
        file.minor_version,
        file.flags,
        file.original_size,
        file.default_block_size,
        file.block_count,
        file.flags & ace_format::FILE_FLAG_HAS_INDEX != 0
    );
    for _ in 0..file.block_count {
        let (header, metadata, payload) = reader.read_block()?;
        if show_blocks {
            println!("block={} original={} codec={:?} entropy={:?} transforms={:?} metadata={} payload={}", header.block_id, header.original_size, header.codec, header.entropy, header.transforms, metadata.len(), payload.len());
        }
    }
    Ok(())
}

/// Runs analyzer and planner only, printing deterministic candidate scores per source block.
fn explain_command(input: &str, profile: CompressionProfile) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("reading {input}"))?;
    let mut config = AceConfig::default();
    config.profile = profile;
    let engine = AceEngine::new(config)?;
    for explanation in engine.explain(&data)? {
        println!(
            "block {} size={} H0={:.3} H1={:.3} run={:.3} delta={:.3} repeat={:.3}",
            explanation.block_id,
            explanation.profile.size,
            explanation.profile.entropy_h0,
            explanation.profile.entropy_h1,
            explanation.profile.run_score,
            explanation.profile.delta_score,
            explanation.profile.repetition_score
        );
        for candidate in &explanation.candidates {
            println!("  candidate tier={:?} {:?}/{:?} transforms={:?} score={} predicted={} metadata={} reason={}", candidate.tier, candidate.decoding.codec, candidate.decoding.entropy, candidate.decoding.transforms, candidate.score, candidate.cost.predicted_size_bytes, candidate.cost.metadata_bytes, candidate.reason);
        }
        println!("  selected {:?}/{:?} transforms={:?} fast_path={} estimated={} sampled={} full_trials={}\n", explanation.selected.decoding.codec, explanation.selected.decoding.entropy, explanation.selected.decoding.transforms, explanation.telemetry.fast_path_hit, explanation.telemetry.estimated_candidates, explanation.telemetry.sampled_candidates, explanation.telemetry.full_trial_encodes);
    }
    Ok(())
}

/// Verifies full logical reconstruction and every block CRC32C without persisting output.
fn verify_command(input: &str) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("reading {input}"))?;
    let decoded = AceEngine::default_engine().decompress(&data)?;
    let mut header_reader = AceReader::new(Cursor::new(&data), ace_core::DecodeLimits::default());
    let header = header_reader.read_file_header()?;
    if header.flags & ace_format::FILE_FLAG_HAS_INDEX != 0 {
        let indexed =
            AceIndexedDecoder::open(Cursor::new(&data), ace_core::DecodeLimits::default())?;
        println!("validated block index entries={}", indexed.block_count());
    }
    println!("verified {} reconstructed bytes", decoded.len());
    Ok(())
}

/// Performs one indexed block read and writes its reconstructed bytes.
fn decode_block_command(input: &str, block_id: u64, output: &str) -> Result<()> {
    let file = fs::File::open(input).with_context(|| format!("opening {input}"))?;
    let mut decoder = AceIndexedDecoder::open(file, ace_core::DecodeLimits::default())?;
    let block = decoder.decode_block(block_id)?;
    fs::write(output, &block).with_context(|| format!("writing {output}"))?;
    println!("decoded block {block_id}: {} bytes", block.len());
    Ok(())
}

/// Performs an indexed logical-range read and writes only the requested reconstructed bytes.
fn read_range_command(input: &str, offset: u64, length: u64, output: &str) -> Result<()> {
    let end = offset.checked_add(length).context("range overflow")?;
    let file = fs::File::open(input).with_context(|| format!("opening {input}"))?;
    let mut decoder = AceIndexedDecoder::open(file, ace_core::DecodeLimits::default())?;
    let metrics = decoder.range_metrics(offset..end)?;
    let bytes = decoder.read_range(offset..end)?;
    fs::write(output, &bytes).with_context(|| format!("writing {output}"))?;
    println!("decoded logical range [{offset}, {end}): {} bytes physical={} blocks={} physical/logical={:.3}", bytes.len(), metrics.physical_bytes_read, metrics.blocks_touched, metrics.overread_ratio());
    Ok(())
}
