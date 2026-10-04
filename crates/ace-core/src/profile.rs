/// Statistical description of one input block used by the adaptive planner.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockProfile {
    /// Number of source bytes in the block.
    pub size: usize,
    /// Zero-order Shannon entropy in bits per byte, nominally in `0..=8`.
    pub entropy_h0: f32,
    /// Sampled first-order conditional entropy estimate in bits per byte.
    pub entropy_h1: f32,
    /// Fraction of source bytes equal to zero.
    pub zero_ratio: f32,
    /// Fraction of bytes participating in repeated-byte runs of length at least four.
    pub run_score: f32,
    /// Normalized entropy improvement predicted after byte-delta transformation.
    pub delta_score: f32,
    /// Sampled fingerprint collision ratio used as a proxy for LZ matchability.
    pub repetition_score: f32,
    /// Approximate mean sampled LZ match length in bytes.
    pub sampled_match_length: f32,
    /// Approximate p95 sampled LZ match length in bytes.
    pub sampled_match_p95: f32,
    /// Fraction of sampled source coverage that can plausibly be represented by LZ matches.
    pub sampled_match_coverage: f32,
    /// Fraction of sampled LZ hits whose match length is at least 32 bytes.
    pub long_match_ratio: f32,
    /// Number of byte values present in the block.
    pub unique_byte_count: u16,
    /// Combined score estimating that further compression is unlikely to be useful.
    pub incompressibility_score: f32,
}
