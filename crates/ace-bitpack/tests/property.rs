//! Property tests: every transform of `ace-bitpack` is an exact bijection for every lane.

use ace_bitpack::{
    delta, delta_of_delta, frame_of_reference, max_bit_width, pack, undelta, undelta_of_delta,
    unframe_of_reference, unpack, Lane,
};
use proptest::prelude::*;

/// Generates one property-test module per lane so the same properties cover u16, u32 and u64
/// without copy-pasting the test bodies.
macro_rules! lane_properties {
    ($module:ident, $lane:ty, $max_len:expr) => {
        mod $module {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(64))]

                /// Bit packing at the minimal width round-trips exactly.
                #[test]
                fn bitpack_roundtrip(values in proptest::collection::vec(any::<$lane>(), 0..$max_len)) {
                    let width = max_bit_width(&values);
                    let packed = pack(&values, width).unwrap();
                    prop_assert_eq!(unpack::<$lane>(&packed, values.len(), width).unwrap(), values);
                }

                /// ZigZag is a bijection.
                #[test]
                fn zigzag_roundtrip(value in any::<<$lane as Lane>::Signed>()) {
                    prop_assert_eq!(<$lane>::zigzag(value).unzigzag(), value);
                }

                /// First-order deltas reconstruct the sequence.
                #[test]
                fn delta_roundtrip(values in proptest::collection::vec(any::<$lane>(), 1..$max_len)) {
                    prop_assert_eq!(undelta(values[0], &delta(&values)), values);
                }

                /// Delta-of-delta reconstructs sequences of at least two values.
                #[test]
                fn dod_roundtrip(values in proptest::collection::vec(any::<$lane>(), 2..$max_len)) {
                    let (first_delta, dod) = delta_of_delta(&values);
                    prop_assert_eq!(undelta_of_delta(values[0], first_delta, &dod), values);
                }

                /// Frame-of-reference reconstructs the sequence.
                #[test]
                fn for_roundtrip(values in proptest::collection::vec(any::<$lane>(), 0..$max_len)) {
                    let (base, offsets) = frame_of_reference(&values);
                    prop_assert_eq!(unframe_of_reference(base, &offsets), values);
                }
            }
        }
    };
}

lane_properties!(lane_u16, u16, 1024usize);
lane_properties!(lane_u32, u32, 1024usize);
lane_properties!(lane_u64, u64, 512usize);
