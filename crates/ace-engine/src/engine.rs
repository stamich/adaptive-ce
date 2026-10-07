//! The [`AceEngine`] façade: configuration, compression, decompression and explanation.

use std::io::{Cursor, Read, Write};

use ace_analysis::{analyze_numeric, recommend_block_size, AnalysisLevel, DefaultBlockAnalyzer};
use ace_core::{AceConfig, AceError, AceResult, BlockSizePolicy, CompressionStats, DecodeLimits};
use ace_format::AceReader;
use ace_planner::{
    evaluate_all_candidates, evaluate_candidates_v4_with_route, DefaultCompressionPlanner,
    PlanningContext,
};
use ace_runtime::RuntimeConfig;
use rayon::prelude::*;

use crate::block_encoder::encode_block;
use crate::block_pipeline::decode_encoded_block;
use crate::container::assemble_container;
use crate::{BlockExplanation, FixedBlockChunker};

/// Largest input prefix sampled by the `Auto` block-size policy.
const AUTO_POLICY_SAMPLE_BYTES: usize = 4 * 1024 * 1024;

/// Public façade of the ACE compression engine.
#[derive(Debug, Clone)]
pub struct AceEngine {
    /// Encoder configuration.
    config: AceConfig,
    /// Decoder resource limits.
    limits: DecodeLimits,
}

/// Inherent methods of [`AceEngine`].
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

    /// Creates an engine using default settings.
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

    /// Compresses an in-memory byte slice into a complete ACE Format 1.3 file.
    pub fn compress(&self, input: &[u8]) -> AceResult<Vec<u8>> {
        self.compress_with_stats(input).map(|(bytes, _)| bytes)
    }

    /// Compresses bytes and returns both the file image and detailed telemetry.
    ///
    /// Blocks are encoded in parallel in batches of at most `max_in_flight_blocks`, then
    /// serialized in block-id order, so the output never depends on the thread count.
    pub fn compress_with_stats(&self, input: &[u8]) -> AceResult<(Vec<u8>, CompressionStats)> {
        let block_size = self.resolved_block_size(input);
        let chunker =
            FixedBlockChunker::new(block_size).ok_or(AceError::InvalidConfig("zero block size"))?;
        let chunks: Vec<(u64, &[u8])> = chunker
            .chunks(input)
            .enumerate()
            .map(|(id, bytes)| (id as u64, bytes))
            .collect();
        let mut effective_config = self.config.clone();
        effective_config.block_size = block_size;
        let runtime = RuntimeConfig::from_ace(&effective_config)?;
        let pool = runtime.build_pool()?;

        let mut blocks = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(runtime.max_in_flight_blocks) {
            let encoded: Vec<_> = pool.install(|| {
                batch
                    .par_iter()
                    .map(|&(id, bytes)| encode_block(&self.config, id, bytes))
                    .collect()
            });
            for block in encoded {
                blocks.push(block?);
            }
        }

        let mut stats = CompressionStats {
            input_bytes: input.len() as u64,
            block_count: blocks.len() as u64,
            ..Default::default()
        };
        let output = assemble_container(
            blocks,
            input.len(),
            block_size,
            self.config.write_index,
            &mut stats,
        )?;
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

    /// Decompresses an in-memory ACE 1.0, 1.1, 1.2 or 1.3 file into reconstructed bytes.
    pub fn decompress(&self, input: &[u8]) -> AceResult<Vec<u8>> {
        let mut out = Vec::new();
        self.decompress_from(Cursor::new(input), &mut out)?;
        Ok(out)
    }

    /// Reads, validates and sequentially decompresses ACE 1.0–1.3 blocks to a writer.
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

    /// Produces deterministic planner explanations (every candidate fully evaluated) without
    /// writing an ACE stream.
    pub fn explain(&self, input: &[u8]) -> AceResult<Vec<BlockExplanation>> {
        let chunker = FixedBlockChunker::new(self.resolved_block_size(input))
            .ok_or(AceError::InvalidConfig("zero block size"))?;
        chunker
            .chunks(input)
            .enumerate()
            .map(|(id, block)| self.explain_block(id as u64, block))
            .collect()
    }

    /// Explains one block: routing, analysis, all candidate costs and the selected plan.
    fn explain_block(&self, block_id: u64, block: &[u8]) -> AceResult<BlockExplanation> {
        let context = PlanningContext::classify(block, &self.config);
        let profile = DefaultBlockAnalyzer
            .analyze_with_level(block, AnalysisLevel::for_profile(self.config.profile));
        let candidates = DefaultCompressionPlanner.candidates_for_route(
            &profile,
            &self.config,
            context.route.route,
        );
        let evaluated = evaluate_all_candidates(block, &candidates, &self.config)?;
        let decision = evaluate_candidates_v4_with_route(
            block,
            &profile,
            &candidates,
            &self.config,
            &context.route,
        )?;
        Ok(BlockExplanation {
            block_id,
            profile,
            numeric_profile: analyze_numeric(block),
            candidates: evaluated,
            selected: decision.plan,
            telemetry: decision.telemetry,
        })
    }

    /// Resolves the file-level block size according to the configured block-size policy.
    fn resolved_block_size(&self, input: &[u8]) -> usize {
        if !matches!(self.config.block_size_policy, BlockSizePolicy::Auto) {
            return self.config.block_size;
        }
        let numeric = analyze_numeric(&input[..input.len().min(AUTO_POLICY_SAMPLE_BYTES)]);
        recommend_block_size(self.config.access_hint, self.config.profile, &numeric).block_size
    }
}
