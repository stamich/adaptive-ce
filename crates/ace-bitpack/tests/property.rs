use ace_bitpack::{
    delta_i32, delta_i64, delta_of_delta_i32, delta_of_delta_i64, max_bit_width_u32,
    max_bit_width_u64, pack_u32, pack_u64, undelta_i32, undelta_i64, undelta_of_delta_i32,
    undelta_of_delta_i64, unpack_u32, unpack_u64, unzigzag_u32, unzigzag_u64, zigzag_i32,
    zigzag_i64,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Arbitrary u32 vectors survive fixed-width scalar bit packing exactly.
    #[test]
    fn arbitrary_u32_bitpack_roundtrip(values in proptest::collection::vec(any::<u32>(), 0..1024)) {
        let width = max_bit_width_u32(&values);
        let packed = pack_u32(&values, width).unwrap();
        prop_assert_eq!(unpack_u32(&packed, values.len(), width).unwrap(), values);
    }

    /// Arbitrary u64 vectors survive fixed-width scalar bit packing exactly.
    #[test]
    fn arbitrary_u64_bitpack_roundtrip(values in proptest::collection::vec(any::<u64>(), 0..512)) {
        let width = max_bit_width_u64(&values);
        let packed = pack_u64(&values, width).unwrap();
        prop_assert_eq!(unpack_u64(&packed, values.len(), width).unwrap(), values);
    }

    /// ZigZag is a bijection for arbitrary signed 32-bit integers.
    #[test]
    fn arbitrary_i32_zigzag_roundtrip(value in any::<i32>()) {
        prop_assert_eq!(unzigzag_u32(zigzag_i32(value)), value);
    }

    /// ZigZag is a bijection for arbitrary signed 64-bit integers.
    #[test]
    fn arbitrary_i64_zigzag_roundtrip(value in any::<i64>()) {
        prop_assert_eq!(unzigzag_u64(zigzag_i64(value)), value);
    }

    /// First-order u32 deltas reconstruct the original sequence exactly.
    #[test]
    fn arbitrary_u32_delta_roundtrip(values in proptest::collection::vec(any::<u32>(), 1..512)) {
        prop_assert_eq!(undelta_i32(values[0], &delta_i32(&values)), values);
    }

    /// First-order u64 deltas reconstruct the original sequence exactly.
    #[test]
    fn arbitrary_u64_delta_roundtrip(values in proptest::collection::vec(any::<u64>(), 1..256)) {
        prop_assert_eq!(undelta_i64(values[0], &delta_i64(&values)), values);
    }

    /// Delta-of-delta u32 representation reconstructs sequences with at least two values.
    #[test]
    fn arbitrary_u32_dod_roundtrip(values in proptest::collection::vec(any::<u32>(), 2..512)) {
        let (first_delta, dod) = delta_of_delta_i32(&values);
        prop_assert_eq!(undelta_of_delta_i32(values[0], first_delta, &dod), values);
    }

    /// Delta-of-delta u64 representation reconstructs sequences with at least two values.
    #[test]
    fn arbitrary_u64_dod_roundtrip(values in proptest::collection::vec(any::<u64>(), 2..256)) {
        let (first_delta, dod) = delta_of_delta_i64(&values);
        prop_assert_eq!(undelta_of_delta_i64(values[0], first_delta, &dod), values);
    }
}
