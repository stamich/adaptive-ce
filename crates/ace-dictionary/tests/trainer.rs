//! Tests of the deterministic dictionary trainer (ACE 0.8.0).

use ace_dictionary::{
    dictionary_id_of, train_dictionary, TrainerConfig, MAX_TRAINED_DICTIONARY_SIZE,
};

/// A small deterministic generator for the unique parts of the samples.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// `count` JSON-like records that share their keys and differ in their values.
fn json_samples(count: usize) -> Vec<Vec<u8>> {
    let mut state = 0x1234_5678_9abc_def1u64;
    (0..count)
        .map(|i| {
            format!(
                "{{\"event\":\"temperature_reading\",\"device_id\":{},\"unit\":\"celsius\",\
                 \"value\":{},\"firmware\":\"release-2.7.1\",\"nonce\":\"{:016x}\"}}",
                i,
                next(&mut state) % 400,
                next(&mut state)
            )
            .into_bytes()
        })
        .collect()
}

/// Borrows owned samples as the slice-of-slices the trainer takes.
fn borrow(samples: &[Vec<u8>]) -> Vec<&[u8]> {
    samples.iter().map(Vec::as_slice).collect()
}

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn the_same_samples_give_the_same_bytes() {
    let samples = json_samples(200);
    let config = TrainerConfig::default();
    let first = train_dictionary(&borrow(&samples), &config).unwrap();
    let second = train_dictionary(&borrow(&samples), &config).unwrap();
    assert_eq!(first, second);
    assert_eq!(dictionary_id_of(&first), dictionary_id_of(&second));
}

#[test]
fn the_size_cap_is_honoured() {
    let samples = json_samples(300);
    for size in [64usize, 100, 1000, 4096] {
        let config = TrainerConfig {
            dictionary_size: size,
            segment_len: 32,
            ..TrainerConfig::default()
        };
        let dictionary = train_dictionary(&borrow(&samples), &config).unwrap();
        assert!(!dictionary.is_empty() && dictionary.len() <= size, "size {size}");
    }
}

#[test]
fn shared_content_is_selected_and_unique_content_is_not() {
    let samples = json_samples(300);
    let dictionary = train_dictionary(&borrow(&samples), &TrainerConfig::default()).unwrap();
    for shared in [
        &b"temperature_reading"[..],
        b"\"unit\":\"celsius\"",
        b"release-2.7.1",
    ] {
        assert!(
            contains(&dictionary, shared),
            "missing {}",
            String::from_utf8_lossy(shared)
        );
    }
}

#[test]
fn unique_content_is_mostly_left_out() {
    let samples = json_samples(300);
    let config = TrainerConfig {
        dictionary_size: 2048,
        segment_len: 24,
        ..TrainerConfig::default()
    };
    let dictionary = train_dictionary(&borrow(&samples), &config).unwrap();
    // Every sample ends with a 16-digit nonce that no other sample shares. Short segments
    // follow the shared words, so only a small fraction of the nonces can be inside.
    let hits = samples
        .iter()
        .filter(|sample| {
            let start = sample.len() - 18;
            contains(&dictionary, &sample[start..start + 16])
        })
        .count();
    assert!(hits * 10 <= samples.len(), "{hits} nonces in the dictionary");
}

#[test]
fn covered_content_is_not_repeated() {
    // Two kinds of records. The dictionary must hold both, not one of them twice.
    let mut samples = Vec::new();
    for i in 0..100u32 {
        samples.push(format!("ALPHA-HEADER-{i:04} common alpha payload block").into_bytes());
        samples.push(format!("BETA--HEADER-{i:04} common beta  payload block").into_bytes());
    }
    let config = TrainerConfig {
        dictionary_size: 256,
        segment_len: 24,
        ..TrainerConfig::default()
    };
    let dictionary = train_dictionary(&borrow(&samples), &config).unwrap();
    assert!(contains(&dictionary, b"alpha payload"));
    assert!(contains(&dictionary, b"beta  payload"));
}

#[test]
fn the_order_of_the_samples_matters_only_through_the_budget() {
    let samples = json_samples(50);
    let reversed: Vec<Vec<u8>> = samples.iter().rev().cloned().collect();
    let config = TrainerConfig::default();
    // With enough budget the frequencies are order independent; ties are broken by sample index,
    // so the bytes may be arranged differently but the set of shared words is the same.
    let a = train_dictionary(&borrow(&samples), &config).unwrap();
    let b = train_dictionary(&borrow(&reversed), &config).unwrap();
    for word in [&b"temperature_reading"[..], b"celsius", b"release-2.7.1"] {
        assert!(contains(&a, word) && contains(&b, word));
    }
}

#[test]
fn the_input_budget_cuts_the_samples_in_order() {
    let samples = json_samples(100);
    let one = samples[0].len();
    let tight = TrainerConfig {
        max_sample_bytes: one * 3,
        ..TrainerConfig::default()
    };
    let loose = TrainerConfig::default();
    let small = train_dictionary(&borrow(&samples), &tight).unwrap();
    // The first three samples are used and are identical to training on exactly those.
    let head = train_dictionary(&borrow(&samples[..3]), &loose).unwrap();
    assert_eq!(small, head);
}

#[test]
fn unusable_inputs_are_rejected() {
    let config = TrainerConfig::default();
    assert!(train_dictionary(&[], &config).is_err());
    assert!(train_dictionary(&[b"only one sample, long enough"], &config).is_err());
    assert!(train_dictionary(&[b"tiny", b"tiny"], &config).is_err());
    // Two long samples with nothing in common.
    let a: Vec<u8> = (0..200u8).collect();
    let b: Vec<u8> = (0..200u8).rev().map(|x| x ^ 0x5a).collect();
    assert!(train_dictionary(&[&a, &b], &config).is_err());
}

#[test]
fn invalid_configurations_are_rejected() {
    let samples = json_samples(10);
    let bad = [
        TrainerConfig {
            dictionary_size: 0,
            ..TrainerConfig::default()
        },
        TrainerConfig {
            dictionary_size: MAX_TRAINED_DICTIONARY_SIZE + 1,
            ..TrainerConfig::default()
        },
        TrainerConfig {
            segment_len: 7,
            ..TrainerConfig::default()
        },
        TrainerConfig {
            dictionary_size: 100,
            segment_len: 101,
            ..TrainerConfig::default()
        },
        TrainerConfig {
            max_sample_bytes: 0,
            ..TrainerConfig::default()
        },
        TrainerConfig {
            max_sample_bytes: usize::MAX,
            ..TrainerConfig::default()
        },
    ];
    for config in bad {
        assert!(config.validate().is_err(), "{config:?}");
        assert!(train_dictionary(&borrow(&samples), &config).is_err());
    }
    assert!(TrainerConfig::default().validate().is_ok());
}

#[test]
fn dictionary_ids_follow_the_content() {
    assert_eq!(dictionary_id_of(b"abc"), dictionary_id_of(b"abc"));
    assert_ne!(dictionary_id_of(b"abc"), dictionary_id_of(b"abd"));
    assert_ne!(dictionary_id_of(b"").0, 0);
}
