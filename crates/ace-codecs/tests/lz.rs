//! Tests of `ace_codecs` `lz` (formerly the inline unit tests of `src`).

use ace_codecs::*;
use ace_core::LzMode;

/// Overlapping and disjoint back-references of every distance and length decode to the input.
#[test]
fn matches_of_every_distance_and_length_roundtrip() {
    for distance in 1..40usize {
        let seed: Vec<u8> = (0..distance as u8)
            .map(|i| i.wrapping_mul(37).wrapping_add(5))
            .collect();
        for len in 1..130usize {
            let data: Vec<u8> = seed
                .iter()
                .copied()
                .cycle()
                .take(distance + len + 8)
                .collect();
            for mode in [LzMode::Fast, LzMode::Balanced] {
                let encoded = lz_encode(&data, mode);
                assert_eq!(
                    lz_decode(&encoded, data.len()).unwrap(),
                    data,
                    "distance {distance} len {len} {mode:?}"
                );
            }
        }
    }
}

/// Verifies overlapping LZ references are decoded correctly.
#[test]
fn repeated_data_roundtrip() {
    let data = b"abcabcabcabcabcabcabcabc";
    for mode in [LzMode::Fast, LzMode::Balanced] {
        let e = lz_encode(data, mode);
        assert_eq!(lz_decode(&e, data.len()).unwrap(), data);
    }
}

/// Inputs of several sizes and shapes (tiny, noise, repetitive, above the threshold).
fn inputs() -> Vec<Vec<u8>> {
    let mut x = 7u32;
    let mut noise = |n: usize| -> Vec<u8> {
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 24) as u8
            })
            .collect()
    };
    vec![
        b"a".to_vec(),
        b"abcd".to_vec(),
        noise(3),
        noise(5000),
        b"status=ACTIVE region=eu\n".repeat(900),
        noise(70_000),
        vec![0u8; 200_000],
        (0..90_000u32).flat_map(|i| (i / 5).to_le_bytes()).collect(),
        noise(10),
    ]
}

/// Calls in any order, with sizes growing and shrinking, give the bytes of the allocating
/// encoder: stale entries of the reused tables are never read.
#[test]
fn reused_tables_give_the_reference_bytes() {
    for mode in [LzMode::Fast, LzMode::Balanced] {
        for round in 0..2 {
            for input in inputs() {
                assert_eq!(
                    lz_encode(&input, mode),
                    lz_encode_allocating(&input, mode),
                    "{mode:?} round {round} len {}",
                    input.len()
                );
            }
        }
    }
}

/// A call that unwound leaves the tables dirty; the next call repairs them.
#[test]
fn dirty_tables_are_repaired() {
    let data = b"abracadabra abracadabra abracadabra".repeat(50);
    let want = lz_encode_allocating(&data, LzMode::Balanced);
    lz_leave_scratch_dirty();
    assert_eq!(lz_encode(&data, LzMode::Balanced), want);
}

/// Inputs around the threshold give the same bytes whichever path encodes them.
#[test]
fn both_paths_agree_around_the_threshold() {
    let base: Vec<u8> = (0..LZ_SCRATCH_MAX_INPUT + 50)
        .map(|i| ((i * i / 7) % 61) as u8)
        .collect();
    for len in [
        LZ_SCRATCH_MAX_INPUT - 1,
        LZ_SCRATCH_MAX_INPUT,
        LZ_SCRATCH_MAX_INPUT + 1,
    ] {
        for mode in [LzMode::Fast, LzMode::Balanced] {
            let data = &base[..len];
            assert_eq!(
                lz_encode(data, mode),
                lz_encode_allocating(data, mode),
                "{len} {mode:?}"
            );
        }
    }
}
