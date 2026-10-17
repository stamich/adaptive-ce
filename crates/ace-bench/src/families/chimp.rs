//! `chimp-evaluation` (ACE 0.6.1): is Chimp / Chimp128 worth a TS1 mode?
//!
//! Every block of every binary64 workload is encoded by the best existing codec-level choice
//! (RAW, RLE, LZ fast, NUM1 and every TS1 layout including ALP, no planner, no entropy stage)
//! and, since an entropy stage can beat any bare codec, by the whole engine (BALANCED and DENSE,
//! one block per file, container overhead not subtracted: this favours Chimp by about 100 bytes
//! per block) and by Chimp and Chimp128 ([`crate::chimp`], header cost charged like a TS1 payload). A block
//! "shrinks" when the better Chimp variant is strictly smaller than the best existing choice.
//! Rule: Chimp is added only if at least [`MIN_SHRINKING_BLOCKS`] of all blocks shrink.

use ace_codecs::TIME_SERIES_HEADER_SIZE;
use ace_corpus::Workload;

use super::float::candidate_sizes;
use crate::chimp::{chimp128_decode, chimp128_encode, chimp_decode, chimp_encode};
use crate::prelude::*;

/// Share of blocks that must shrink for Chimp to be kept (the 0.6.1 decision rule).
pub(crate) const MIN_SHRINKING_BLOCKS: f64 = 0.05;
/// Bytes per workload (16 blocks of 256 KiB).
const BYTES: usize = 4 * 1024 * 1024;
/// Block size, the engine default.
const BLOCK: usize = 256 * 1024;

/// The binary64 workloads of the evaluation.
fn workloads() -> [Workload; 9] {
    [
        Workload::F64Constant,
        Workload::F64Step,
        Workload::F64Smooth,
        Workload::F64SensorTemperature,
        Workload::F64FinancialPrice,
        Workload::F64Noisy,
        Workload::F64Random,
        Workload::F64Special,
        Workload::F64DecimalPrice,
    ]
}

/// Little-endian binary64 bit patterns of a block (a trailing partial lane is ignored).
fn lanes(block: &[u8]) -> Vec<u64> {
    block
        .chunks_exact(8)
        .map(|lane| u64::from_le_bytes(lane.try_into().unwrap_or([0; 8])))
        .collect()
}

/// Payload size of a Chimp stream of `bits` bits, with a TS1-sized header.
fn payload_bytes(bits: u64) -> usize {
    TIME_SERIES_HEADER_SIZE + bits.div_ceil(8) as usize
}

/// `chimp-evaluation`: one row per workload plus a decision row.
pub(crate) fn chimp_evaluation_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let (mut blocks, mut shrinking, mut engine_wins) = (0u64, 0u64, 0u64);
    let engines = [CompressionProfile::Balanced, CompressionProfile::Dense]
        .map(|profile| {
            AceEngine::new(AceConfig {
                threads: 1,
                profile,
                ..AceConfig::default()
            })
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    for workload in workloads() {
        let data = generate(workload, BYTES);
        let (mut workload_blocks, mut workload_shrinking) = (0u64, 0u64);
        let (mut best_total, mut chimp_total) = (0usize, 0usize);
        for block in data.chunks(BLOCK) {
            let values = lanes(block);
            let (stream, bits) = chimp_encode(&values);
            let (stream128, bits128) = chimp128_encode(&values);
            assert_eq!(chimp_decode(&stream, bits, values.len())?, values);
            assert_eq!(chimp128_decode(&stream128, bits128, values.len())?, values);
            let chimp = payload_bytes(bits).min(payload_bytes(bits128));
            let codec_best = candidate_sizes(block)
                .into_iter()
                .filter_map(|(_, size)| size)
                .min()
                .ok_or("no candidate")?;
            let engine_best = engines
                .iter()
                .map(|engine| engine.compress(block).map(|packed| packed.len()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .min()
                .ok_or("no engine")?;
            let best = codec_best.min(engine_best);
            engine_wins += u64::from(engine_best < codec_best);
            workload_blocks += 1;
            workload_shrinking += u64::from(chimp < best);
            best_total += best;
            chimp_total += chimp.min(best);
        }
        blocks += workload_blocks;
        shrinking += workload_shrinking;
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload.name())
            .field("path", "chimp_evaluation")
            .field("blocks", workload_blocks)
            .field("blocks_where_chimp_is_smaller", workload_shrinking)
            .field("best_existing_bytes", best_total)
            .field("best_with_chimp_bytes", chimp_total);
        rows.push(row.build());
    }
    let share = shrinking as f64 / blocks.max(1) as f64;
    let mut decision = JsonObjectBuilder::new();
    decision
        .field("workload_id", "all")
        .field("path", "chimp_decision")
        .field("blocks", blocks)
        .field("blocks_where_chimp_is_smaller", shrinking)
        .field(
            "blocks_where_the_engine_beats_every_bare_codec",
            engine_wins,
        )
        .field("share", share)
        .field("rule_min_share", MIN_SHRINKING_BLOCKS)
        .field("add_chimp", share >= MIN_SHRINKING_BLOCKS);
    rows.push(decision.build());
    Ok(rows)
}
