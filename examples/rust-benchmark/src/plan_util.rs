//! Plan identity, estimator-error and oracle helpers shared by planner families.

use crate::prelude::*;

/// Aggregates analytical estimator error for one deterministic benchmark bucket.
#[derive(Debug, Default, Clone)]
pub(crate) struct EstimatorErrorStats {
    /// Number of candidate observations in the bucket.
    pub(crate) count: u64,
    /// Sum of absolute prediction errors in bytes.
    pub(crate) absolute_error_sum: u128,
    /// Sum of signed prediction errors in bytes (`predicted - actual`).
    pub(crate) signed_error_sum: i128,
    /// Sum of absolute percentage errors.
    pub(crate) absolute_percentage_error_sum: f64,
    /// Individual absolute errors used to report deterministic p95.
    pub(crate) absolute_errors: Vec<u64>,
}

/// Inherent methods of [`EstimatorErrorStats`].
impl EstimatorErrorStats {
    /// Records one prediction and its diagnostic full-encode size.
    pub(crate) fn observe(&mut self, predicted: u64, actual: u64) {
        let signed = predicted as i128 - actual as i128;
        let absolute = if signed < 0 { (-signed) as u128 } else { signed as u128 };
        let absolute = absolute.min(u64::MAX as u128) as u64;
        self.count = self.count.saturating_add(1);
        self.absolute_error_sum = self.absolute_error_sum.saturating_add(absolute as u128);
        self.signed_error_sum = self.signed_error_sum.saturating_add(signed);
        if actual != 0 {
            self.absolute_percentage_error_sum += absolute as f64 / actual as f64;
        }
        self.absolute_errors.push(absolute);
    }

    /// Serializes MAE, MAPE, signed bias and p95 absolute error for JSON output.
    pub(crate) fn json(&self) -> Value {
        if self.count == 0 {
            let mut row = JsonObjectBuilder::new();
            row.field("count", 0u64)
                .field("mae_bytes", 0.0f64)
                .field("mape", 0.0f64)
                .field("bias_bytes", 0.0f64)
                .field("p95_absolute_error_bytes", 0u64);
            return row.build();
        }
        let mut errors = self.absolute_errors.clone();
        errors.sort_unstable();
        let p95_index = ((errors.len() - 1) * 95) / 100;
        let mut row = JsonObjectBuilder::new();
        row.field("count", self.count)
            .field("mae_bytes", self.absolute_error_sum as f64 / self.count as f64)
            .field("mape", self.absolute_percentage_error_sum / self.count as f64)
            .field("bias_bytes", self.signed_error_sum as f64 / self.count as f64)
            .field("p95_absolute_error_bytes", errors[p95_index]);
        row.build()
    }
}

/// Returns serialized bytes produced by one internal physical plan.
pub(crate) fn encoded_plan_size(input:&[u8], plan:&PhysicalCompressionPlan)->Result<usize,Box<dyn std::error::Error>>{
    let (metadata,payload,_)=encode_plan_payload(input,plan)?;
    let prefix=if matches!(plan.decoding.entropy,EntropyCodecId::None){0}else{4};
    Ok(metadata.len()+payload.len()+prefix)
}

/// Returns true when two candidates have identical decoder semantics and LZ search policy.
pub(crate) fn same_plan(a:&PhysicalCompressionPlan,b:&PhysicalCompressionPlan)->bool{a.decoding==b.decoding&&a.lz_mode==b.lz_mode}

/// Returns a stable estimator-calibration bucket for one transform/codec family.
pub(crate) fn estimator_family_id(plan: &PhysicalCompressionPlan) -> String {
    let delta = plan.decoding.transforms.iter().any(|t| matches!(t, TransformId::DeltaByte));
    let base = match (plan.decoding.codec, plan.lz_mode) {
        (CodecId::Raw, _) => "raw",
        (CodecId::Rle, _) => "rle",
        (CodecId::Lz, Some(LzMode::Fast)) => "lz_fast",
        (CodecId::Lz, Some(LzMode::Balanced)) => "lz_balanced",
        (CodecId::Lz, None) => "lz",
        (CodecId::Numeric, _) => "numeric",
    };
    if delta { format!("delta_{base}") } else { base.to_string() }
}

/// Returns a stable textual physical-plan identifier for JSON diagnostics.
pub(crate) fn plan_id(p:&PhysicalCompressionPlan)->String{
    let mut parts=p.decoding.transforms.iter().map(|t|match t{TransformId::None=>"none",TransformId::DeltaByte=>"delta"}).collect::<Vec<_>>();
    parts.push(match(p.decoding.codec,p.lz_mode){(CodecId::Raw,_)=>"raw",(CodecId::Rle,_)=>"rle",(CodecId::Lz,Some(LzMode::Balanced))=>"lz_balanced",(CodecId::Lz,_)=>"lz_fast",(CodecId::Numeric,_)=>"numeric"});
    parts.push(match p.decoding.entropy{EntropyCodecId::None=>"none",EntropyCodecId::Huffman=>"huffman",EntropyCodecId::Rans=>"rans",EntropyCodecId::Rans4x=>"rans4x"});
    parts.join("+")
}

/// Returns the JSON label for one candidate tier.
pub(crate) fn tier_name(t:CandidateTier)->&'static str{match t{CandidateTier::Mandatory=>"mandatory",CandidateTier::Likely=>"likely",CandidateTier::Exploratory=>"exploratory"}}

/// Builds the offline oracle search space; it is intentionally broader than runtime candidate pruning.
pub(crate) fn oracle_plans()->Vec<PhysicalCompressionPlan>{
    let mut plans=Vec::new();
    let mut add=|transforms:Vec<TransformId>,codec:CodecId,entropy:EntropyCodecId,lz:Option<LzMode>|plans.push(PhysicalCompressionPlan{decoding:DecodingPlan{transforms,codec,dictionary:None,entropy},lz_mode:lz,tier:CandidateTier::Exploratory,cost:Default::default(),score:0,reason:"oracle"});
    add(vec![],CodecId::Raw,EntropyCodecId::None,None);
    add(vec![],CodecId::Rle,EntropyCodecId::None,None);
    add(vec![],CodecId::Numeric,EntropyCodecId::None,None);
    for e in [EntropyCodecId::Huffman,EntropyCodecId::Rans,EntropyCodecId::Rans4x]{
        add(vec![],CodecId::Raw,e,None); add(vec![],CodecId::Rle,e,None); add(vec![TransformId::DeltaByte],CodecId::Raw,e,None);
        for lz in [LzMode::Fast,LzMode::Balanced]{add(vec![],CodecId::Lz,e,Some(lz));add(vec![TransformId::DeltaByte],CodecId::Lz,e,Some(lz));}
    }
    plans
}
