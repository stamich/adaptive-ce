/// Result of sampled repeated-sequence analysis used as a cheap LZ predictor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepetitionStats {
    /// Fraction of sampled four-byte fingerprints seen previously.
    pub collision_ratio: f32,
    /// Mean exact extension length for repeated sampled fingerprints.
    pub mean_match_length: f32,
    /// Approximate p95 exact extension length among repeated fingerprints.
    pub p95_match_length: f32,
    /// Fraction of source coverage represented by sampled matches after accounting for sample stride.
    pub match_coverage: f32,
    /// Fraction of repeated fingerprints whose exact match is at least 32 bytes long.
    pub long_match_ratio: f32,
}

/// Estimates repeated-sequence density, coverage and match length without running the full LZ matcher.
///
/// ACE 0.3-buildfix7 deliberately mirrors the production codec more closely than the original
/// analyzer: match extension is allowed across the current position (the production decoder
/// supports overlapping references) and the maximum inspected match length equals the codec's
/// 130-byte token limit. The analysis remains sampled every 16 bytes and therefore avoids a
/// full LZ trial encode.
pub fn analyze_repetition(input: &[u8]) -> RepetitionStats {
    const TABLE: usize = 8192;
    const STEP: usize = 16;
    const MAX_MATCH: usize = 130;
    if input.len() < 4 {
        return RepetitionStats {
            collision_ratio: 0.0,
            mean_match_length: 0.0,
            p95_match_length: 0.0,
            match_coverage: 0.0,
            long_match_ratio: 0.0,
        };
    }

    let mut last = vec![usize::MAX; TABLE];
    let mut samples = 0usize;
    let mut hits = 0usize;
    let mut long_hits = 0usize;
    let mut match_bytes = 0usize;
    let mut lengths = Vec::new();
    let mut i = 0usize;

    while i + 4 <= input.len() {
        let h = hash4(&input[i..i + 4]) & (TABLE - 1);
        let previous = last[h];
        if previous != usize::MAX
            && previous < i
            && &input[previous..previous + 4] == &input[i..i + 4]
        {
            let max = (input.len() - i).min(MAX_MATCH);
            let mut len = 4usize;
            while len < max
                && previous + len < input.len()
                && input[previous + len] == input[i + len]
            {
                len += 1;
            }
            hits += 1;
            long_hits += if len >= 32 { 1 } else { 0 };
            match_bytes = match_bytes.saturating_add(len);
            lengths.push(len);
        }
        last[h] = i;
        samples += 1;
        i = i.saturating_add(STEP);
    }

    lengths.sort_unstable();
    let p95_match_length = if lengths.is_empty() {
        0.0
    } else {
        let idx = ((lengths.len() - 1) * 95) / 100;
        lengths[idx] as f32
    };
    let sampled_source_bytes = samples.saturating_mul(STEP).max(1);

    RepetitionStats {
        collision_ratio: if samples == 0 {
            0.0
        } else {
            hits as f32 / samples as f32
        },
        mean_match_length: if hits == 0 {
            0.0
        } else {
            match_bytes as f32 / hits as f32
        },
        p95_match_length,
        match_coverage: (match_bytes as f32 / sampled_source_bytes as f32).clamp(0.0, 1.0),
        long_match_ratio: if hits == 0 {
            0.0
        } else {
            long_hits as f32 / hits as f32
        },
    }
}

/// Computes a stable integer hash of exactly four bytes for analysis sampling.
fn hash4(bytes: &[u8]) -> usize {
    let v = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    (v.wrapping_mul(2_654_435_761) >> 19) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirms long periodic data produces strong coverage and long-match evidence.
    #[test]
    fn periodic_data_has_long_match_signal() {
        let data = b"0123456789abcdef".repeat(4096);
        let stats = analyze_repetition(&data);
        assert!(stats.collision_ratio > 0.5);
        assert!(stats.mean_match_length >= 32.0);
        assert!(stats.p95_match_length >= stats.mean_match_length.min(32.0));
        assert!(stats.match_coverage > 0.5);
        assert!(stats.long_match_ratio > 0.5);
    }

    /// Confirms tiny inputs stay on the zero-evidence path.
    #[test]
    fn tiny_input_has_zero_repetition_stats() {
        let stats = analyze_repetition(b"abc");
        assert_eq!(stats.collision_ratio, 0.0);
        assert_eq!(stats.match_coverage, 0.0);
    }
}
