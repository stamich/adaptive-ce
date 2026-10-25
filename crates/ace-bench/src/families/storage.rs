//! `storage` (ACE 0.8.0): columnar blocks and adaptive segmentation against their targets.
//!
//! * `columnar` rows — for every [`RecordWorkload`] and profile, the size of the row-wise block
//!   stream (no schema) next to the columnar one (schema from the record layout), the gain, and
//!   the encode / decode timing of the columnar configuration.
//! * `segmented` rows — a 3 MiB block made of JSON, counters and random bytes, compressed with
//!   and without segmentation (one block, so the container choice is isolated).
//! * `summary` — how many record workloads missed the 10 % target.
//!
//! Every row verifies its round trip. The gates live in `tools/ace-check_regressions0.8.0.py`
//! (section `storage`).

use ace_core::{ColumnKind, ColumnSpec, Schema, SegmentationConfig};
use ace_corpus::{FieldKind, RecordWorkload, Workload};

use super::float::profile_name;
use crate::prelude::*;

/// Input size of the record rows.
const BYTES: usize = 4 * 1024 * 1024;
/// Bytes of each part of the segmentation fixture.
const PART: usize = 1024 * 1024;

/// Profiles measured.
const PROFILES: [CompressionProfile; 3] = [
    CompressionProfile::Fast,
    CompressionProfile::Balanced,
    CompressionProfile::Dense,
];

/// The [`Schema`] that describes the layout of a record workload.
fn schema_of(workload: RecordWorkload) -> Result<Schema, Box<dyn std::error::Error>> {
    let columns = workload
        .layout()
        .iter()
        .map(|&(width, kind)| ColumnSpec {
            width,
            kind: match kind {
                FieldKind::Bytes => ColumnKind::Bytes,
                FieldKind::Unsigned => ColumnKind::Unsigned,
                FieldKind::Signed => ColumnKind::Signed,
                FieldKind::Float => ColumnKind::Float,
            },
        })
        .collect();
    Ok(Schema::new(columns)?)
}

/// Single-threaded configuration for one profile.
fn config(profile: CompressionProfile) -> AceConfig {
    AceConfig {
        threads: 1,
        profile,
        ..AceConfig::default()
    }
}

/// One `columnar` row: row-wise versus columnar for `workload` under `profile`.
fn columnar_row(
    workload: RecordWorkload,
    profile: CompressionProfile,
) -> Result<(Value, f64), Box<dyn std::error::Error>> {
    let data = fingerprinted_records(workload);
    let rowwise = AceEngine::new(config(profile))?;
    let columnar = AceEngine::new(AceConfig {
        schema: Some(schema_of(workload)?),
        ..config(profile)
    })?;
    let row_bytes = rowwise.compress(&data)?;
    let (encode, (col_bytes, stats)) = measure(|| Ok(columnar.compress_with_stats(&data)?))?;
    let (decode, restored) = measure(|| Ok(columnar.decompress(&col_bytes)?))?;
    assert_eq!(restored, data, "columnar roundtrip of {}", workload.name());
    assert_eq!(
        rowwise.decompress(&row_bytes)?,
        data,
        "row-wise roundtrip of {}",
        workload.name()
    );
    let gain = row_bytes.len() as f64 / col_bytes.len().max(1) as f64;
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", workload.name())
        .field("path", "columnar")
        .field("profile", profile_name(profile))
        .field("input_bytes", data.len())
        .field("rowwise_bytes", row_bytes.len())
        .field("columnar_bytes", col_bytes.len())
        .field("gain_vs_rowwise", gain)
        .field("format_minor", col_bytes[5])
        .field("columnar_blocks", stats.columnar_blocks)
        .field("column_streams", stats.column_streams)
        .value("encode", timing_json(&encode, data.len()))
        .value("decode", timing_json(&decode, data.len()));
    Ok((row.build(), gain))
}

/// The generated (and fingerprinted) bytes of a record workload.
fn fingerprinted_records(workload: RecordWorkload) -> Vec<u8> {
    crate::corpus::fingerprinted(workload.name(), workload.generate(BYTES))
}

/// The `segmented` row: JSON, counters and random bytes in one block.
fn segmented_row() -> Result<Value, Box<dyn std::error::Error>> {
    let mut data = Vec::with_capacity(3 * PART);
    data.extend_from_slice(&generate(Workload::StructuredJson, PART));
    data.extend_from_slice(&generate(Workload::U32Counter, PART));
    data.extend_from_slice(&generate(Workload::Random, PART));
    let base = AceConfig {
        threads: 1,
        block_size: data.len(),
        ..AceConfig::default()
    };
    let plain = AceEngine::new(base.clone())?.compress(&data)?;
    let segmented = AceEngine::new(AceConfig {
        segmentation: Some(SegmentationConfig::default()),
        ..base
    })?;
    let (encode, (bytes, stats)) = measure(|| Ok(segmented.compress_with_stats(&data)?))?;
    let (decode, restored) = measure(|| Ok(segmented.decompress(&bytes)?))?;
    assert_eq!(restored, data, "segmented roundtrip");
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", "segmented-mixed-3m")
        .field("path", "segmented")
        .field("input_bytes", data.len())
        .field("plain_bytes", plain.len())
        .field("segmented_bytes", bytes.len())
        .field(
            "gain_vs_plain",
            plain.len() as f64 / bytes.len().max(1) as f64,
        )
        .field("format_minor", bytes[5])
        .field("segmented_blocks", stats.segmented_blocks)
        .field("segments", stats.segments)
        .value("encode", timing_json(&encode, data.len()))
        .value("decode", timing_json(&decode, data.len()));
    Ok(row.build())
}

/// `storage`: columnar gains, the segmentation fixture and the 10 % target summary.
pub(crate) fn storage_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let (mut missed, mut checked) = (Vec::new(), 0u64);
    for workload in RecordWorkload::ALL {
        for profile in PROFILES {
            let (row, gain) = columnar_row(workload, profile)?;
            rows.push(row);
            checked += 1;
            if gain < 1.10 {
                missed.push(format!("{}/{}", workload.name(), profile_name(profile)));
            }
        }
    }
    rows.push(segmented_row()?);
    let mut summary = JsonObjectBuilder::new();
    summary
        .field("workload_id", "summary")
        .field("path", "storage_summary")
        .field("cases_checked", checked)
        .field("below_target", missed.len())
        .field("below_target_cases", missed);
    rows.push(summary.build());
    Ok(rows)
}
