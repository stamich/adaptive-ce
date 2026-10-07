//! Float prefilter on the shared corpus: admission of Corpus V4 floats, zero false positives
//! on Corpus V3 and on the false-positive corpus (a hard gate of ACE 0.5.0).

use ace_analysis::{float_prefilter, FloatWidth};
use ace_corpus::Workload;

/// Engine block size: the prefilter sees one block at a time.
const BLOCK: usize = 256 * 1024;
/// Bytes generated per workload.
const BYTES: usize = 1024 * 1024;

/// Admitted width of every block of `workload` (`None` = rejected).
fn admitted_widths(workload: Workload) -> Vec<Option<FloatWidth>> {
    workload
        .generate(BYTES)
        .chunks(BLOCK)
        .map(|block| float_prefilter(block).admitted.map(|p| p.width))
        .collect()
}

/// Corpus V4 float workloads are admitted with the right width in every block.
#[test]
fn corpus_v4_floats_are_admitted() {
    let expected = [
        (Workload::F64Constant, FloatWidth::F64),
        (Workload::F64Step, FloatWidth::F64),
        (Workload::F64Smooth, FloatWidth::F64),
        (Workload::F64SensorTemperature, FloatWidth::F64),
        (Workload::F64FinancialPrice, FloatWidth::F64),
        (Workload::F64Noisy, FloatWidth::F64),
        (Workload::F32Smooth, FloatWidth::F32),
        (Workload::F32Sensor, FloatWidth::F32),
    ];
    for (workload, width) in expected {
        for (block, admitted) in admitted_widths(workload).into_iter().enumerate() {
            assert_eq!(admitted, Some(width), "{} block {block}", workload.name());
        }
    }
}

/// No block of Corpus V3 (the 0.4.x corpus) or of the non-float V4 workloads is admitted.
#[test]
fn integer_and_byte_workloads_are_rejected() {
    let rejected = Workload::CORPUS_V3.into_iter().chain([
        Workload::F64Random,
        Workload::F64Special,
        Workload::IntSparseChange,
        Workload::IntCounterReset,
    ]);
    for workload in rejected {
        assert!(
            admitted_widths(workload).iter().all(Option::is_none),
            "false positive: {}",
            workload.name()
        );
    }
}

/// The false-positive corpus: NaN-heavy, Inf-heavy, random and text-like buffers.
#[test]
fn false_positive_corpus_is_rejected() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();
    let smooth = |i: u64| 20.0 + i as f64 * 0.001;
    cases.push((
        "nan-heavy",
        (0..32_768u64)
            .flat_map(|i| if i % 8 == 0 { f64::NAN } else { smooth(i) }.to_le_bytes())
            .collect(),
    ));
    cases.push((
        "inf-heavy",
        (0..32_768u64)
            .flat_map(|i| if i % 8 == 0 { f64::INFINITY } else { smooth(i) }.to_le_bytes())
            .collect(),
    ));
    cases.push((
        "random-u64",
        (0..32_768).flat_map(|_| next().to_le_bytes()).collect(),
    ));
    cases.push((
        "random-f64-finite",
        (0..32_768)
            .flat_map(|_| {
                let bits = next() & !(0x7ffu64 << 52) | ((next() % 0x7fe) << 52);
                bits.to_le_bytes()
            })
            .collect(),
    ));
    cases.push((
        "text",
        b"timestamp=1700000000123 sensor=temp-01 value=21.5 unit=C status=ok\n"
            .iter()
            .copied()
            .cycle()
            .take(BLOCK)
            .collect(),
    ));
    cases.push((
        "u64-small-ints",
        (0..32_768u64)
            .flat_map(|i| (i % 1000).to_le_bytes())
            .collect(),
    ));
    for (name, data) in cases {
        let prefilter = float_prefilter(&data);
        assert_eq!(prefilter.admitted, None, "false positive: {name}");
        assert!(prefilter.rejection.is_some(), "{name}");
    }
}

/// Prints the first-block profile of every workload for threshold calibration.
///
/// `cargo test -p ace-analysis --test float_prefilter_corpus -- --ignored --nocapture`
#[test]
#[ignore = "calibration report"]
fn report_profiles() {
    for workload in Workload::ALL {
        let data = workload.generate(BYTES);
        let block = &data[..BLOCK];
        for width in FloatWidth::ALL {
            if let Some(p) = ace_analysis::float_profile(block, width) {
                println!(
                    "{:24} {} nf={:.2} sub={:.2} zero={:.2} impl={:.2} xz={:.2} mb={:5.1} exp={:4} sign={:.2} -> {:?}",
                    workload.name(),
                    width.label(),
                    p.non_finite_ratio,
                    p.subnormal_ratio,
                    p.zero_ratio,
                    p.implausible_magnitude_ratio,
                    p.xor_zero_ratio,
                    p.mean_xor_meaningful_bits,
                    p.exponent_distinct,
                    p.sign_change_ratio,
                    ace_analysis::admit_float(&p).err().map(|r| r.label()),
                );
            }
        }
    }
}
