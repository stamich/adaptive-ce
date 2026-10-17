//! Tests of `ace_core` `plan` (formerly the inline unit tests of `src`).

use ace_core::*;

/// Plan labels are stable and `is_plain_numeric` recognizes only bare NUM1 plans.
#[test]
fn plan_label_and_numeric_shape() {
    let mut plan = PhysicalCompressionPlan::raw();
    assert_eq!(plan.label(), "raw+none");
    assert!(!plan.is_plain_numeric());
    plan.decoding.codec = CodecId::Numeric;
    assert!(plan.is_plain_numeric());
    assert_eq!(plan.label(), "numeric+none");
    plan.decoding.codec = CodecId::Lz;
    plan.lz_mode = Some(LzMode::Balanced);
    plan.decoding.transforms = vec![TransformId::DeltaByte];
    plan.decoding.entropy = EntropyCodecId::Rans4x;
    assert_eq!(plan.label(), "delta+lz_balanced+rans4x");
    assert_eq!(EntropyCodecId::None.metadata_prefix_bytes(), 0);
    assert_eq!(
        EntropyCodecId::Huffman.metadata_prefix_bytes(),
        ace_core::PRIMARY_LENGTH_PREFIX_BYTES
    );
}
