//! JSON text for `Stats` (`docs/FFI.md`, "Stats JSON"). Key order is fixed.

use std::fmt::Write as _;

use ace::{BlockReport, Stats};

/// Appends `text` as a JSON string literal.
fn push_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Renders `stats` as one JSON object with the members of the documented schema.
#[must_use]
pub fn stats_json(stats: &Stats) -> String {
    let mut out = String::with_capacity(256);
    let _ = write!(
        out,
        "{{\"input_bytes\":{},\"output_bytes\":{},\"format_minor\":{},\"blocks\":{},\
         \"raw_blocks\":{},\"rle_blocks\":{},\"lz_blocks\":{},\"numeric_blocks\":{},\
         \"time_series_blocks\":{},\"ratio\":{},\"plan_distribution\":{{",
        stats.input_bytes,
        stats.output_bytes,
        stats.format_minor,
        stats.blocks,
        stats.raw_blocks,
        stats.rle_blocks,
        stats.lz_blocks,
        stats.numeric_blocks,
        stats.time_series_blocks,
        json_number(stats.ratio()),
    );
    for (index, (label, count)) in stats.plan_distribution.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        push_string(&mut out, label);
        let _ = write!(out, ":{count}");
    }
    out.push_str("}}");
    out
}

/// A finite `f64` in JSON syntax; non-finite values (never produced) become `0`.
fn json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "0".to_owned()
    }
}

/// Renders the planner report as a JSON array with one object per block.
#[must_use]
pub fn explain_json(reports: &[BlockReport]) -> String {
    let mut out = String::with_capacity(128 * reports.len() + 2);
    out.push('[');
    for (index, report) in reports.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"block_id\":{},\"size\":{},\"route\":",
            report.block_id, report.size
        );
        push_string(&mut out, &report.route);
        out.push_str(",\"selected\":");
        push_string(&mut out, &report.selected);
        out.push_str(",\"reason\":");
        push_string(&mut out, &report.reason);
        out.push_str(",\"candidates\":[");
        for (position, candidate) in report.candidates.iter().enumerate() {
            if position > 0 {
                out.push(',');
            }
            push_string(&mut out, candidate);
        }
        out.push_str("]}");
    }
    out.push(']');
    out
}
