/// Result of sampled repeated-sequence analysis used as a cheap LZ predictor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepetitionStats {
    /// Fraction of sampled four-byte fingerprints seen previously.
    pub collision_ratio: f32,
    /// Mean exact extension length for repeated sampled fingerprints.
    pub mean_match_length: f32,
}

/// Estimates repeated-sequence density and match length without running the full LZ matcher.
pub fn analyze_repetition(input: &[u8]) -> RepetitionStats {
    const TABLE: usize = 4096;
    const STEP: usize = 16;
    if input.len() < 4 { return RepetitionStats { collision_ratio: 0.0, mean_match_length: 0.0 }; }
    let mut last = vec![usize::MAX; TABLE];
    let mut samples = 0usize;
    let mut hits = 0usize;
    let mut match_bytes = 0usize;
    let mut i = 0usize;
    while i + 4 <= input.len() {
        let h = hash4(&input[i..i + 4]) & (TABLE - 1);
        let previous = last[h];
        if previous != usize::MAX && previous < i && &input[previous..previous + 4] == &input[i..i + 4] {
            hits += 1;
            let max = (input.len() - i).min(64);
            let mut len = 4usize;
            while len < max && previous + len < i && input[previous + len] == input[i + len] { len += 1; }
            match_bytes += len;
        }
        last[h] = i;
        samples += 1;
        i = i.saturating_add(STEP);
    }
    RepetitionStats {
        collision_ratio: if samples == 0 { 0.0 } else { hits as f32 / samples as f32 },
        mean_match_length: if hits == 0 { 0.0 } else { match_bytes as f32 / hits as f32 },
    }
}

/// Computes a stable integer hash of exactly four bytes for analysis sampling.
fn hash4(bytes: &[u8]) -> usize {
    let v = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    (v.wrapping_mul(2_654_435_761) >> 20) as usize
}
