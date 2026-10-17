//! Tests of `ace_entropy` `rans_model` (formerly the inline unit tests of `src`).

use ace_entropy::*;

/// The slot lookup table must agree with the reference linear scan for every slot.
#[test]
fn slot_table_matches_reference_scan() {
    let mut data = vec![b'a'; 5_000];
    data.extend((0u8..=255).cycle().take(3_000));
    data.extend(std::iter::repeat_n(b'z', 77));
    for input in [data, vec![9u8; 100], vec![1u8, 2]] {
        let model = NormalizedFrequencyTable::from_input(&input).unwrap();
        let table = model.slot_lookup_table();
        for slot in 0..RANS_SCALE {
            assert_eq!(table[slot as usize], model.symbol_for_slot(slot).unwrap());
        }
    }
}

/// Verifies that normalization always sums to the fixed scale.
#[test]
fn normalization_sum() {
    let data: Vec<u8> = (0u8..=255).cycle().take(100_000).collect();
    let model = NormalizedFrequencyTable::from_input(&data).unwrap();
    assert_eq!(
        model.frequencies.iter().map(|&x| x as u32).sum::<u32>(),
        RANS_SCALE
    );
}
