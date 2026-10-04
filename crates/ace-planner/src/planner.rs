use crate::EntropySelectionPolicy;
use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CodecId, CompressionProfile, DecodingPlan,
    EntropyCodecId, LzMode, PhysicalCompressionPlan, TransformId,
};

/// Generates a deterministic candidate set from one analyzed block.
pub trait CompressionPlanner {
    /// Returns candidates in stable order. RAW is always first.
    fn candidates(&self, profile: &BlockProfile, config: &AceConfig) -> Vec<PhysicalCompressionPlan>;
}

/// ACE 0.3 candidate generator with profile-specific search breadth.
///
/// The buildfix deliberately separates the FAST search space from BALANCED/DENSE.
/// FAST stays narrow to protect throughput, while BALANCED and DENSE retain enough
/// oracle coverage to satisfy the planner-recall release gate.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultCompressionPlanner;

impl CompressionPlanner for DefaultCompressionPlanner {
    /// Generates candidates without consulting wall-clock time or thread scheduling.
    fn candidates(&self, p: &BlockProfile, c: &AceConfig) -> Vec<PhysicalCompressionPlan> {
        if should_early_raw(p, c) {
            return vec![PhysicalCompressionPlan::raw()];
        }

        match c.profile {
            CompressionProfile::Fast => fast_candidates(p),
            CompressionProfile::Balanced => balanced_candidates(p),
            CompressionProfile::Dense => dense_candidates(p),
        }
    }
}

/// Returns true when the analyzer has very high confidence that compression work is wasteful.
fn should_early_raw(p: &BlockProfile, c: &AceConfig) -> bool {
    c.enable_early_raw
        && p.entropy_h0 > 7.985
        && p.run_score < 0.002
        && p.delta_score < 0.002
        && p.repetition_score < 0.002
        && p.sampled_match_length < 4.5
}

/// Builds the deliberately small FAST candidate set.
///
/// FAST never evaluates `LzMode::Balanced` and never admits rANS as a mandatory
/// baseline. rANS is considered only when the analyzer predicts a strong structural
/// opportunity large enough to justify the scalar encoder cost measured in ACE 0.2.
fn fast_candidates(p: &BlockProfile) -> Vec<PhysicalCompressionPlan> {
    let policy = EntropySelectionPolicy::for_profile(CompressionProfile::Fast);
    let allow_rans = policy.input_allows_rans(p.size);
    let mut out = vec![PhysicalCompressionPlan::raw()];

    out.push(plan(
        Vec::new(),
        CodecId::Raw,
        EntropyCodecId::Huffman,
        None,
        CandidateTier::Mandatory,
        "FAST mandatory Huffman baseline",
    ));

    if p.run_score >= 0.12 || p.zero_ratio >= 0.20 {
        out.push(plan(
            Vec::new(),
            CodecId::Rle,
            EntropyCodecId::None,
            None,
            CandidateTier::Likely,
            "FAST strong run/zero evidence",
        ));
        out.push(plan(
            Vec::new(),
            CodecId::Rle,
            EntropyCodecId::Huffman,
            None,
            CandidateTier::Likely,
            "FAST strong run/zero evidence plus Huffman",
        ));
    }

    if p.delta_score >= 0.10 {
        out.push(plan(
            vec![TransformId::DeltaByte],
            CodecId::Raw,
            EntropyCodecId::Huffman,
            None,
            CandidateTier::Likely,
            "FAST strong delta evidence",
        ));
        if allow_rans && p.delta_score >= 0.30 {
            out.push(plan(
                vec![TransformId::DeltaByte],
                CodecId::Raw,
                EntropyCodecId::Rans,
                None,
                CandidateTier::Likely,
                "FAST exceptional delta gain justifies rANS trial",
            ));
        }
    }

    let lz_signal = lz_signal(p);
    if lz_signal >= 0.06 || p.sampled_match_length >= 7.0 {
        out.push(plan(
            Vec::new(),
            CodecId::Lz,
            EntropyCodecId::Huffman,
            Some(LzMode::Fast),
            CandidateTier::Likely,
            "FAST repeated-sequence evidence",
        ));
        if allow_rans && lz_signal >= 0.25 {
            out.push(plan(
                Vec::new(),
                CodecId::Lz,
                EntropyCodecId::Rans,
                Some(LzMode::Fast),
                CandidateTier::Likely,
                "FAST exceptional LZ gain justifies rANS trial",
            ));
        }
    }

    deduplicate(out)
}

/// Builds the BALANCED search space.
///
/// BALANCED intentionally covers the complete ACE 0.2.1 oracle family while using
/// tiers to preserve explainability. This fixes the 0.84375 recall observed in the
/// first 0.2.1 release without broadening FAST.
fn balanced_candidates(p: &BlockProfile) -> Vec<PhysicalCompressionPlan> {
    full_oracle_family(p, CompressionProfile::Balanced, false)
}

/// Builds the DENSE search space.
///
/// DENSE evaluates the same decoder-semantic families as the offline oracle and marks
/// deeper LZ+rANS combinations as likely when structural evidence supports them.
fn dense_candidates(p: &BlockProfile) -> Vec<PhysicalCompressionPlan> {
    full_oracle_family(p, CompressionProfile::Dense, true)
}

/// Builds a recall-oriented candidate family for BALANCED/DENSE profiles.
fn full_oracle_family(
    p: &BlockProfile,
    profile: CompressionProfile,
    dense: bool,
) -> Vec<PhysicalCompressionPlan> {
    let allow_rans = EntropySelectionPolicy::for_profile(profile).input_allows_rans(p.size);
    let mut out = vec![PhysicalCompressionPlan::raw()];
    let lz_strength = lz_signal(p);

    out.push(plan(
        Vec::new(),
        CodecId::Raw,
        EntropyCodecId::Huffman,
        None,
        CandidateTier::Mandatory,
        "mandatory Huffman baseline",
    ));
    if allow_rans {
        out.push(plan(
            Vec::new(),
            CodecId::Raw,
            EntropyCodecId::Rans,
            None,
            CandidateTier::Mandatory,
            "mandatory rANS baseline",
        ));
        if p.size >= 16 * 1024 {
            out.push(plan(
                Vec::new(),
                CodecId::Raw,
                EntropyCodecId::Rans4x,
                None,
                CandidateTier::Exploratory,
                "ACE 0.3 rANS4x baseline",
            ));
        }
    }

    let run_tier = if p.run_score >= 0.08 || p.zero_ratio >= 0.15 {
        CandidateTier::Likely
    } else {
        CandidateTier::Exploratory
    };
    out.push(plan(Vec::new(), CodecId::Rle, EntropyCodecId::None, None, run_tier, "RLE oracle-coverage baseline"));
    out.push(plan(Vec::new(), CodecId::Rle, EntropyCodecId::Huffman, None, run_tier, "RLE plus Huffman oracle coverage"));
    if allow_rans {
        out.push(plan(Vec::new(), CodecId::Rle, EntropyCodecId::Rans, None, run_tier, "RLE plus rANS oracle coverage"));
        if p.size >= 16 * 1024 { out.push(plan(Vec::new(), CodecId::Rle, EntropyCodecId::Rans4x, None, run_tier, "RLE plus rANS4x ACE 0.3 candidate")); }
    }

    let delta_tier = if p.delta_score >= 0.05 {
        CandidateTier::Likely
    } else {
        CandidateTier::Exploratory
    };
    out.push(plan(
        vec![TransformId::DeltaByte],
        CodecId::Raw,
        EntropyCodecId::Huffman,
        None,
        delta_tier,
        "delta plus Huffman oracle coverage",
    ));
    if allow_rans {
        out.push(plan(
            vec![TransformId::DeltaByte],
            CodecId::Raw,
            EntropyCodecId::Rans,
            None,
            delta_tier,
            "delta plus rANS oracle coverage",
        ));
        if p.size >= 16 * 1024 { out.push(plan(vec![TransformId::DeltaByte], CodecId::Raw, EntropyCodecId::Rans4x, None, delta_tier, "delta plus rANS4x ACE 0.3 candidate")); }
    }

    let lz_fast_tier = if lz_strength >= 0.04 || p.sampled_match_length >= 6.0 {
        CandidateTier::Likely
    } else {
        CandidateTier::Exploratory
    };
    out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Huffman, Some(LzMode::Fast), lz_fast_tier, "LZ fast plus Huffman oracle coverage"));
    if allow_rans {
        out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Rans, Some(LzMode::Fast), lz_fast_tier, "LZ fast plus rANS oracle coverage"));
        if p.size >= 16 * 1024 { out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Rans4x, Some(LzMode::Fast), lz_fast_tier, "LZ fast plus rANS4x ACE 0.3 candidate")); }
    }

    let lz_balanced_tier = if dense || lz_strength >= 0.10 {
        CandidateTier::Likely
    } else {
        CandidateTier::Exploratory
    };
    out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Huffman, Some(LzMode::Balanced), lz_balanced_tier, "LZ balanced plus Huffman oracle coverage"));
    if allow_rans {
        out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Rans, Some(LzMode::Balanced), lz_balanced_tier, "LZ balanced plus rANS oracle coverage"));
        if p.size >= 16 * 1024 { out.push(plan(Vec::new(), CodecId::Lz, EntropyCodecId::Rans4x, Some(LzMode::Balanced), lz_balanced_tier, "LZ balanced plus rANS4x ACE 0.3 candidate")); }
    }

    let delta_lz_tier = if dense && (p.delta_score >= 0.04 || lz_strength >= 0.05) {
        CandidateTier::Likely
    } else {
        CandidateTier::Exploratory
    };
    for mode in [LzMode::Fast, LzMode::Balanced] {
        out.push(plan(
            vec![TransformId::DeltaByte],
            CodecId::Lz,
            EntropyCodecId::Huffman,
            Some(mode),
            delta_lz_tier,
            "delta plus LZ plus Huffman oracle coverage",
        ));
        if allow_rans {
            out.push(plan(
                vec![TransformId::DeltaByte],
                CodecId::Lz,
                EntropyCodecId::Rans,
                Some(mode),
                delta_lz_tier,
                "delta plus LZ plus rANS oracle coverage",
            ));
        }
    }

    deduplicate(out)
}

/// Computes the combined evidence used to classify LZ candidates.
fn lz_signal(p: &BlockProfile) -> f32 {
    p.repetition_score
        .max((p.sampled_match_length / 32.0).clamp(0.0, 1.0))
}

/// Creates an unevaluated candidate with stable decoder semantics and explanation text.
fn plan(
    transforms: Vec<TransformId>,
    codec: CodecId,
    entropy: EntropyCodecId,
    lz_mode: Option<LzMode>,
    tier: CandidateTier,
    reason: &'static str,
) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms,
            codec,
            dictionary: None,
            entropy,
        },
        lz_mode,
        tier,
        cost: Default::default(),
        score: u128::MAX,
        reason,
    }
}

/// Removes decoder-equivalent duplicate candidates while preserving stable first occurrence order.
fn deduplicate(candidates: Vec<PhysicalCompressionPlan>) -> Vec<PhysicalCompressionPlan> {
    let mut out = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let exists = out.iter().any(|p: &PhysicalCompressionPlan| {
            p.decoding == candidate.decoding && p.lz_mode == candidate.lz_mode
        });
        if !exists {
            out.push(candidate);
        }
    }
    out
}
