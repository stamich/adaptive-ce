//! Deterministic synthetic corpora used by every family.


/// Generates the deterministic heterogeneous corpus used across benchmark families.
pub(crate) fn mixed_data(mebibytes: usize) -> Vec<u8> {
    let target = mebibytes * 1024 * 1024;
    let quarter = target / 4;
    let mut data = Vec::with_capacity(target);
    data.extend(std::iter::repeat(0u8).take(quarter));
    while data.len() < quarter * 2 {
        let v = (data.len() as u32 / 16).to_le_bytes();
        data.extend_from_slice(&v);
    }
    while data.len() < quarter * 3 {
        data.extend_from_slice(b"{\"status\":\"ACTIVE\",\"service\":\"graphnet\",\"region\":\"eu\"}\n");
    }
    let mut x = 0x9e37_79b9u32;
    while data.len() < target {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5; data.push((x & 0xff) as u8);
    }
    data.truncate(target);
    data
}

/// Returns a stable coarse data-class label for one quarter of the synthetic mixed corpus.
pub(crate) fn data_class(block_index: usize, block_count: usize) -> &'static str {
    let quarter = (block_count / 4).max(1);
    match block_index / quarter { 0 => "zeros", 1 => "numeric", 2 => "structured", _ => "random" }
}

/// Returns one deterministic corpus by stable identifier.
///
/// Corpus V2 intentionally covers the broad structures that drive ACE planner decisions without
/// relying on proprietary datasets or network access.
pub(crate) fn corpus_bytes(kind: &str, bytes: usize) -> Vec<u8> {
    match kind {
        "zeros" => vec![0u8; bytes],
        "low-cardinality" => (0..bytes).map(|i| [0u8, 1, 2, 3][i % 4]).collect(),
        "runs" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 0u8;
            while out.len() < bytes {
                let remaining = bytes - out.len();
                let run = remaining.min(4096);
                out.extend(std::iter::repeat(value).take(run));
                value = value.wrapping_add(17);
            }
            out
        }
        "numeric-u32" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 10_000u32;
            while out.len() < bytes {
                value = value.wrapping_add(3);
                out.extend_from_slice(&value.to_le_bytes());
            }
            out.truncate(bytes);
            out
        }
        "delta-series" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 1_000_000u64;
            while out.len() < bytes {
                value = value.wrapping_add((out.len() as u64 % 7) + 1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            out.truncate(bytes);
            out
        }
        "structured-json" => {
            let record = br#"{"service":"ace","status":"ACTIVE","region":"eu","value":123456}
"#;
            let mut out = Vec::with_capacity(bytes);
            while out.len() < bytes {
                out.extend_from_slice(record);
            }
            out.truncate(bytes);
            out
        }
        "random" => {
            let mut out = Vec::with_capacity(bytes);
            let mut x = 0x9e37_79b9u32;
            while out.len() < bytes {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                out.push((x & 0xff) as u8);
            }
            out
        }
        "mixed" => mixed_data(bytes.div_ceil(1024 * 1024))[..bytes].to_vec(),
        other => panic!("unknown hardening corpus: {other}"),
    }
}

/// Generates one deterministic numeric/time-series workload for ACE 0.4 benchmarks.
pub(crate) fn numeric_workload(kind: &str, bytes: usize) -> Vec<u8> {
    match kind {
        "u32-counter" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 10_000u32;
            while out.len() < bytes { value = value.wrapping_add(3); out.extend_from_slice(&value.to_le_bytes()); }
            out.truncate(bytes); out
        }
        "u64-timestamps" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 1_780_000_000_000u64;
            let mut i = 0u64;
            while out.len() < bytes { value = value.wrapping_add(1000 + (i % 3)); out.extend_from_slice(&value.to_le_bytes()); i += 1; }
            out.truncate(bytes); out
        }
        "u64-fixed-step" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 1_780_000_000_000u64;
            while out.len() < bytes { value = value.wrapping_add(1000); out.extend_from_slice(&value.to_le_bytes()); }
            out.truncate(bytes); out
        }
        "gauge-sawtooth" => {
            let mut out = Vec::with_capacity(bytes);
            let mut i = 0u32;
            while out.len() < bytes { let value = 100_000u32 + (i % 4096); out.extend_from_slice(&value.to_le_bytes()); i = i.wrapping_add(1); }
            out.truncate(bytes); out
        }
        "monotonic-outliers" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 1_000_000u32;
            let mut i = 0u32;
            while out.len() < bytes {
                value = value.wrapping_add(if i % 1024 == 0 { 100_000 } else { 1 });
                out.extend_from_slice(&value.to_le_bytes()); i = i.wrapping_add(1);
            }
            out.truncate(bytes); out
        }
        "delta-variable" => {
            let mut out = Vec::with_capacity(bytes);
            let mut value = 100_000u32;
            let mut i = 0u32;
            while out.len() < bytes { value = value.wrapping_add((i % 17) + 1); out.extend_from_slice(&value.to_le_bytes()); i = i.wrapping_add(1); }
            out.truncate(bytes); out
        }
        other => panic!("unknown numeric workload: {other}"),
    }
}
