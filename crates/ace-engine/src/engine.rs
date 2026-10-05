use crate::{BlockExplanation, FixedBlockChunker};
use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_codecs::decode_codec;
use ace_core::{
    AceConfig, AceError, AceResult, CodecId, CompressionStats, DecodeLimits, EntropyCodecId,
    PhysicalCompressionPlan,
};
use ace_entropy::decode_entropy;
use ace_format::{
    checksum, encode_block_header, encode_file_header, encode_index, encode_trailer, AceReader,
    BlockHeader, BlockIndex, BlockIndexEntry, FileHeader, FileTrailer, BLOCK_HEADER_SIZE,
    FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE,
};
use ace_planner::{
    encode_plan_payload, evaluate_all_candidates, evaluate_candidates_v3, CompressionPlanner,
    DefaultCompressionPlanner, PlannerTelemetry,
};
use ace_runtime::RuntimeConfig;
use ace_transforms::invert_transform;
use rayon::prelude::*;
use std::io::{Cursor, Read, Write};
use std::time::{Duration, Instant};

/// Public façade of the ACE 0.3 compression engine.
#[derive(Debug, Clone)]
pub struct AceEngine {
    config: AceConfig,
    limits: DecodeLimits,
}

/// Fully encoded independent block produced by a parallel worker before ordered file assembly.
#[derive(Debug)]
struct EncodedBlock {
    id: u64,
    original_size: usize,
    metadata: Vec<u8>,
    payload: Vec<u8>,
    plan: PhysicalCompressionPlan,
    crc32c: u32,
    analysis_time: Duration,
    planning_time: Duration,
    encoding_time: Duration,
    planner_telemetry: PlannerTelemetry,
}

impl AceEngine {
    /// Creates an engine with explicit encoder configuration and default decoder limits.
    pub fn new(config: AceConfig) -> AceResult<Self> {
        if config.block_size == 0 || config.block_size > u32::MAX as usize {
            return Err(AceError::InvalidConfig(
                "block size must fit u32 and be non-zero",
            ));
        }
        RuntimeConfig::from_ace(&config)?;
        Ok(Self {
            config,
            limits: DecodeLimits::default(),
        })
    }

    /// Creates an engine using ACE 0.2.1 default settings.
    pub fn default_engine() -> Self {
        Self::new(AceConfig::default()).expect("default ACE configuration is valid")
    }

    /// Replaces decoder resource limits.
    pub fn with_decode_limits(mut self, limits: DecodeLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Returns a read-only view of encoder configuration.
    pub fn config(&self) -> &AceConfig {
        &self.config
    }

    /// Compresses an in-memory byte slice into a complete ACE format-1.2 file.
    pub fn compress(&self, input: &[u8]) -> AceResult<Vec<u8>> {
        self.compress_with_stats(input).map(|(bytes, _)| bytes)
    }

    /// Compresses bytes and returns both the file image and detailed telemetry.
    pub fn compress_with_stats(&self, input: &[u8]) -> AceResult<(Vec<u8>, CompressionStats)> {
        let chunker = FixedBlockChunker::new(self.config.block_size)
            .ok_or(AceError::InvalidConfig("zero block size"))?;
        let chunks = chunker
            .chunks(input)
            .enumerate()
            .map(|(id, bytes)| (id as u64, bytes))
            .collect::<Vec<_>>();
        let runtime = RuntimeConfig::from_ace(&self.config)?;
        let pool = runtime.build_pool()?;
        let mut encoded = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(runtime.max_in_flight_blocks) {
            let mut result = pool.install(|| {
                batch
                    .par_iter()
                    .map(|(id, bytes)| self.encode_one_block(*id, bytes))
                    .collect::<Vec<_>>()
            });
            for item in result.drain(..) {
                encoded.push(item?);
            }
        }
        encoded.sort_by_key(|block| block.id);
        let mut flags = 0u16;
        if self.config.write_index {
            flags |= FILE_FLAG_HAS_INDEX;
        }
        let file_header = FileHeader {
            minor_version: 2,
            flags,
            default_block_size: self.config.block_size as u32,
            original_size: input.len() as u64,
            block_count: encoded.len() as u64,
        };
        let mut output = Vec::with_capacity(input.len().saturating_add(FILE_HEADER_SIZE));
        output.extend_from_slice(&encode_file_header(&file_header));
        let mut index = BlockIndex::default();
        let mut original_offset = 0u64;
        let mut stats = CompressionStats {
            input_bytes: input.len() as u64,
            block_count: encoded.len() as u64,
            ..Default::default()
        };
        let serialization_started = Instant::now();
        for block in encoded {
            let file_offset = output.len() as u64;
            let header = BlockHeader {
                block_id: block.id,
                original_size: block.original_size as u32,
                encoded_size: block.payload.len() as u32,
                metadata_size: block.metadata.len() as u32,
                codec: block.plan.decoding.codec,
                entropy: block.plan.decoding.entropy,
                transforms: block.plan.decoding.transforms.clone(),
                dictionary: block.plan.decoding.dictionary,
                flags: 0,
                payload_crc32c: block.crc32c,
            };
            let header_bytes = encode_block_header(&header);
            output.extend_from_slice(&header_bytes);
            output.extend_from_slice(&block.metadata);
            output.extend_from_slice(&block.payload);
            let span = output.len() as u64 - file_offset;
            if span > u32::MAX as u64 {
                return Err(AceError::ResourceLimitExceeded("serialized block span"));
            }
            index.entries.push(BlockIndexEntry {
                block_id: block.id,
                original_offset,
                original_size: block.original_size as u32,
                file_offset,
                encoded_span: span as u32,
                flags: 0,
            });
            original_offset = original_offset
                .checked_add(block.original_size as u64)
                .ok_or(AceError::Malformed("logical output offset overflow"))?;
            stats.analysis_time += block.analysis_time;
            stats.planning_time += block.planning_time;
            stats.encoding_time += block.encoding_time;
            match header.codec {
                CodecId::Raw => stats.raw_blocks += 1,
                CodecId::Rle => stats.rle_blocks += 1,
                CodecId::Lz => stats.lz_blocks += 1,
            }
            if header
                .transforms
                .iter()
                .any(|t| matches!(t, ace_core::TransformId::DeltaByte))
            {
                stats.delta_blocks += 1;
            }
            match header.entropy {
                EntropyCodecId::Huffman => stats.huffman_blocks += 1,
                EntropyCodecId::Rans => stats.rans_blocks += 1,
                EntropyCodecId::Rans4x => stats.rans4x_blocks += 1,
                EntropyCodecId::None => {}
            }
            stats.planner_fast_path_blocks += if block.planner_telemetry.fast_path_hit {
                1
            } else {
                0
            };
            stats.planner_estimated_candidates +=
                block.planner_telemetry.estimated_candidates as u64;
            stats.planner_sampled_candidates += block.planner_telemetry.sampled_candidates as u64;
            stats.planner_full_trial_encodes += block.planner_telemetry.full_trial_encodes as u64;
            stats.record_plan(plan_label(&block.plan));
        }
        stats.serialization_time = serialization_started.elapsed();
        if self.config.write_index {
            let index_started = Instant::now();
            let index_bytes = encode_index(&index);
            let index_offset = output.len() as u64;
            let index_crc32c = checksum(&index_bytes);
            output.extend_from_slice(&index_bytes);
            let trailer = FileTrailer {
                index_offset,
                index_size: index_bytes.len() as u64,
                index_crc32c,
            };
            output.extend_from_slice(&encode_trailer(trailer));
            stats.index_time = index_started.elapsed();
        }
        stats.output_bytes = output.len() as u64;
        Ok((output, stats))
    }

    /// Compresses source bytes and writes the finished ACE file to an arbitrary sink.
    pub fn compress_to<W: Write>(
        &self,
        input: &[u8],
        mut writer: W,
    ) -> AceResult<CompressionStats> {
        let (bytes, stats) = self.compress_with_stats(input)?;
        writer.write_all(&bytes)?;
        Ok(stats)
    }

    /// Decompresses an in-memory ACE 1.0, 1.1 or 1.2 file into reconstructed bytes.
    pub fn decompress(&self, input: &[u8]) -> AceResult<Vec<u8>> {
        let mut out = Vec::new();
        self.decompress_from(Cursor::new(input), &mut out)?;
        Ok(out)
    }

    /// Reads, validates and sequentially decompresses ACE 1.0, 1.1 or 1.2 blocks to an arbitrary writer.
    pub fn decompress_from<R: Read, W: Write>(&self, reader: R, mut writer: W) -> AceResult<()> {
        let mut ace = AceReader::new(reader, self.limits.clone());
        let file = ace.read_file_header()?;
        let mut total = 0u64;
        for expected_id in 0..file.block_count {
            let (header, metadata, payload) = ace.read_block()?;
            if header.block_id != expected_id {
                return Err(AceError::Malformed("unexpected block id"));
            }
            let block = decode_encoded_block(&header, &metadata, &payload, &self.limits)?;
            writer.write_all(&block)?;
            total = total
                .checked_add(block.len() as u64)
                .ok_or(AceError::Malformed("output size overflow"))?;
            if total > file.original_size || total > self.limits.max_output_size {
                return Err(AceError::ResourceLimitExceeded("decoded output size"));
            }
        }
        if total != file.original_size {
            return Err(AceError::Malformed(
                "file original size does not equal reconstructed block sizes",
            ));
        }
        Ok(())
    }

    /// Produces deterministic detailed planner explanations without writing an ACE stream.
    pub fn explain(&self, input: &[u8]) -> AceResult<Vec<BlockExplanation>> {
        let chunker = FixedBlockChunker::new(self.config.block_size)
            .ok_or(AceError::InvalidConfig("zero block size"))?;
        let analyzer = DefaultBlockAnalyzer;
        let planner = DefaultCompressionPlanner;
        let mut out = Vec::new();
        for (id, block) in chunker.chunks(input).enumerate() {
            let profile =
                analyzer.analyze_with_level(block, AnalysisLevel::for_profile(self.config.profile));
            let candidates = planner.candidates(&profile, &self.config);
            let evaluated = evaluate_all_candidates(block, &candidates, &self.config)?;
            let decision = evaluate_candidates_v3(block, &profile, &candidates, &self.config)?;
            out.push(BlockExplanation {
                block_id: id as u64,
                profile,
                candidates: evaluated,
                selected: decision.plan,
                telemetry: decision.telemetry,
            });
        }
        Ok(out)
    }

    /// Encodes one independent source block using deterministic analysis and planning.
    fn encode_one_block(&self, id: u64, input: &[u8]) -> AceResult<EncodedBlock> {
        let analyzer = DefaultBlockAnalyzer;
        let planner = DefaultCompressionPlanner;
        let started = Instant::now();
        let profile =
            analyzer.analyze_with_level(input, AnalysisLevel::for_profile(self.config.profile));
        let analysis_time = started.elapsed();
        let started = Instant::now();
        let candidates = planner.candidates(&profile, &self.config);
        let decision = evaluate_candidates_v3(input, &profile, &candidates, &self.config)?;
        let mut selected = decision.plan;
        let planner_telemetry = decision.telemetry;
        let planning_time = started.elapsed();
        let started = Instant::now();
        let (mut entropy_metadata, mut payload, primary_len) =
            encode_plan_payload(input, &selected)?;
        let mut metadata = wrap_entropy_metadata(
            selected.decoding.entropy,
            primary_len,
            &mut entropy_metadata,
        )?;
        let nonraw_overhead =
            BLOCK_HEADER_SIZE + selected.decoding.transforms.len() + metadata.len();
        if selected.decoding.codec != CodecId::Raw
            || !selected.decoding.transforms.is_empty()
            || !metadata.is_empty()
        {
            if payload
                .len()
                .saturating_add(nonraw_overhead)
                .saturating_add(self.config.min_gain_bytes)
                >= input.len().saturating_add(BLOCK_HEADER_SIZE)
            {
                selected = PhysicalCompressionPlan::raw();
                metadata.clear();
                payload = input.to_vec();
            }
        }
        let encoding_time = started.elapsed();
        Ok(EncodedBlock {
            id,
            original_size: input.len(),
            metadata,
            payload,
            plan: selected,
            crc32c: checksum(input),
            analysis_time,
            planning_time,
            encoding_time,
            planner_telemetry,
        })
    }
}

/// Returns a stable human-readable label for benchmark plan-distribution telemetry.
fn plan_label(plan: &PhysicalCompressionPlan) -> String {
    let mut parts = plan
        .decoding
        .transforms
        .iter()
        .map(|t| match t {
            ace_core::TransformId::None => "none",
            ace_core::TransformId::DeltaByte => "delta",
        })
        .collect::<Vec<_>>();
    parts.push(match (plan.decoding.codec, plan.lz_mode) {
        (CodecId::Raw, _) => "raw",
        (CodecId::Rle, _) => "rle",
        (CodecId::Lz, Some(ace_core::LzMode::Balanced)) => "lz_balanced",
        (CodecId::Lz, _) => "lz_fast",
    });
    parts.push(match plan.decoding.entropy {
        EntropyCodecId::None => "none",
        EntropyCodecId::Huffman => "huffman",
        EntropyCodecId::Rans => "rans",
        EntropyCodecId::Rans4x => "rans4x",
    });
    parts.join("+")
}

/// Adds the uncompressed primary-codec byte length in front of entropy model metadata when required.
fn wrap_entropy_metadata(
    entropy: EntropyCodecId,
    primary_len: usize,
    metadata: &mut Vec<u8>,
) -> AceResult<Vec<u8>> {
    if matches!(entropy, EntropyCodecId::None) {
        return Ok(std::mem::take(metadata));
    }
    if primary_len > u32::MAX as usize {
        return Err(AceError::ResourceLimitExceeded("primary codec stream size"));
    }
    let mut wrapped = Vec::with_capacity(metadata.len() + 4);
    wrapped.extend_from_slice(&(primary_len as u32).to_le_bytes());
    wrapped.append(metadata);
    Ok(wrapped)
}

/// Decodes one already-parsed independent ACE block and verifies its reconstructed checksum.
pub(crate) fn decode_encoded_block(
    header: &BlockHeader,
    metadata: &[u8],
    payload: &[u8],
    limits: &DecodeLimits,
) -> AceResult<Vec<u8>> {
    if header.original_size as usize > limits.max_block_size {
        return Err(AceError::ResourceLimitExceeded("block output size"));
    }
    if header.dictionary.is_some() {
        return Err(AceError::MissingDictionary(
            header.dictionary.expect("checked").id.0,
        ));
    }
    let (entropy_metadata, primary_size) = if matches!(header.entropy, EntropyCodecId::None) {
        (&metadata[..], payload.len())
    } else {
        if metadata.len() < 4 {
            return Err(AceError::Malformed("missing primary-stream length"));
        }
        let primary = u32::from_le_bytes(
            metadata[0..4]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid primary-stream length"))?,
        ) as usize;
        (&metadata[4..], primary)
    };
    let codec_bytes = decode_entropy(header.entropy, entropy_metadata, payload, primary_size)?;
    let mut stage = decode_codec(header.codec, &codec_bytes, header.original_size as usize)?;
    for &transform in header.transforms.iter().rev() {
        stage = invert_transform(transform, &stage)?;
    }
    if stage.len() != header.original_size as usize {
        return Err(AceError::Malformed("decoded block size mismatch"));
    }
    if checksum(&stage) != header.payload_crc32c {
        return Err(AceError::ChecksumMismatch(header.block_id));
    }
    Ok(stage)
}
