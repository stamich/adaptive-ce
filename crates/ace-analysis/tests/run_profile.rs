//! Tests of `ace_analysis` `run_profile` (formerly the inline unit tests of `src`).

use ace_analysis::*;

/// A u32 series changing every 200 values is admitted at every width that sees it.
#[test]
fn sparse_changes_are_admitted() {
    let data: Vec<u8> = (0..65_536u32)
        .flat_map(|i| (1_000 + i / 200 * 7).to_le_bytes())
        .collect();
    let prefilter = run_prefilter(&data);
    let widths: Vec<u8> = prefilter.admitted().map(|p| p.lane_bytes).collect();
    assert_eq!(widths, vec![8, 4]);
    assert!(prefilter.lanes[1].expect("u32").equal_ratio > 0.98);
}

/// A counter changes every value and is rejected.
#[test]
fn counters_are_rejected() {
    let data: Vec<u8> = (0..65_536u32).flat_map(u32::to_le_bytes).collect();
    assert!(!run_prefilter(&data).any());
}

/// Byte runs and zero padding are left to RLE.
#[test]
fn byte_runs_are_rejected() {
    assert!(!run_prefilter(&[0u8; 65_536]).any());
    let mut runs = vec![0x11u8; 32_768];
    runs.extend(vec![0x22u8; 32_768]);
    let profile = run_profile(&runs, 8).expect("profile");
    assert_eq!(profile.equal_ratio, 1.0);
    assert_eq!(profile.splat_ratio, 1.0);
    assert!(!profile.admitted());
}

/// Short inputs and unsupported widths produce no profile.
#[test]
fn short_or_unsupported() {
    assert_eq!(run_profile(&[0u8; 100], 8), None);
    assert_eq!(run_profile(&[0u8; 4096], 3), None);
    assert!(run_profile(&[0u8; 4096], 2).is_some());
}
