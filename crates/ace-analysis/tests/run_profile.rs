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

/// Deterministic xorshift draws for the lane generators below.
fn draws(seed: u32) -> impl Iterator<Item = u32> {
    let mut state = seed;
    std::iter::repeat_with(move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    })
}

/// A value that stays put on half the samples and otherwise moves by a small amount is admitted
/// even though it changes far too often for the 80 % rule (ACE 0.6.1).
#[test]
fn small_changes_are_admitted_below_the_repeat_threshold() {
    let mut value = 5_000u32;
    let data: Vec<u8> = draws(7)
        .take(65_536)
        .flat_map(|draw| {
            if draw % 100 < 40 {
                value = value.wrapping_add(draw >> 20 & 7).wrapping_sub(3);
            }
            value.to_le_bytes()
        })
        .collect();
    let profile = run_profile(&data, 4).expect("profile");
    assert!(profile.equal_ratio < MIN_EQUAL_RATIO, "{profile:?}");
    assert!(
        profile.equal_ratio >= MIN_EQUAL_RATIO_SMALL_DELTA,
        "{profile:?}"
    );
    assert_eq!(profile.small_delta_ratio, 1.0);
    assert!(profile.admitted());
    assert!(run_prefilter(&data).admitted().any(|p| p.lane_bytes == 4));
}

/// The same repeat rate with large jumps is not admitted: the change must be small too.
#[test]
fn large_changes_are_not_admitted_below_the_repeat_threshold() {
    let mut value = 5_000u32;
    let data: Vec<u8> = draws(9)
        .take(65_536)
        .flat_map(|draw| {
            if draw % 100 < 40 {
                value = value.wrapping_add(draw);
            }
            value.to_le_bytes()
        })
        .collect();
    let profile = run_profile(&data, 4).expect("profile");
    assert!(
        profile.equal_ratio >= MIN_EQUAL_RATIO_SMALL_DELTA,
        "{profile:?}"
    );
    assert!(
        profile.small_delta_ratio < MIN_SMALL_DELTA_RATIO,
        "{profile:?}"
    );
    assert!(!profile.admitted());
}

/// Distances are measured in the lane ring: a u16 counter wrapping `65535 -> 0` is a step of one.
#[test]
fn deltas_wrap_around_the_lane() {
    let data: Vec<u8> = (0..65_536u32)
        .flat_map(|i| (((i / 2) as u16).wrapping_add(65_530)).to_le_bytes())
        .collect();
    let profile = run_profile(&data, 2).expect("profile");
    assert_eq!(profile.small_delta_ratio, 1.0);
    assert!(profile.equal_ratio >= MIN_EQUAL_RATIO_SMALL_DELTA);
    assert!(profile.admitted());
}
