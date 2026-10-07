//! Shared imports for every benchmark module (external crates and crate-internal helpers).

pub(crate) use std::collections::BTreeMap;
pub(crate) use std::fs;
pub(crate) use std::hint::black_box;
pub(crate) use std::io::{Cursor, Read, Write};
pub(crate) use std::path::PathBuf;
pub(crate) use std::time::{Instant, SystemTime, UNIX_EPOCH};
pub(crate) use ace_analysis::{analyze_numeric, numeric_prefilter, AnalysisLevel, BlockAnalyzer, DefaultBlockAnalyzer};
pub(crate) use ace_cost::{CandidateEstimator, DefaultCandidateEstimator};
pub(crate) use ace_core::{
    AccessHint, AceConfig, BlockSizePolicy, CandidateTier, CodecId, CompressionProfile,
    DecodingPlan, EntropyCodecId, LzMode, PhysicalCompressionPlan, TransformId,
};
pub(crate) use ace_codecs::{estimate_numeric, numeric_encode};
pub(crate) use ace_engine::{AceEngine, AceIndexedDecoder};
pub(crate) use ace_entropy::{huffman_encode, rans4x_encode, rans_encode};
pub(crate) use ace_planner::{
    classify_planner_route, encode_plan_payload, evaluate_candidates_v4_with_route,
    CandidateEligibility, CompressionPlanner, DefaultCompressionPlanner, PlanningContext,
    PlannerRoute, PolicyOracle, PolicyOracleCandidate, RoutePolicy,
};
pub(crate) use ace_stream::{compress_reader_known_size, StreamLimits};
pub(crate) use flate2::{read::GzDecoder, write::GzEncoder, Compression};
pub(crate) use serde::Serialize;
pub(crate) use serde_json::{Map, Value};

pub(crate) use crate::corpus::*;
pub(crate) use crate::json::*;
pub(crate) use crate::plan_util::*;
pub(crate) use crate::timing::*;
