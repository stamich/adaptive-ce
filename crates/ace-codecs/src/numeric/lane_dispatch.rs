//! Bridges the run-time [`NumericWidth`](ace_core::NumericWidth) stored in NUM1 headers to the
//! compile-time [`Lane`](ace_bitpack::Lane) types used by the generic algorithms.

/// Calls `$function::<lane>($args...)` for the lane type matching `$width`.
///
/// One `match` per call site keeps every inner loop monomorphized (fixed-size loads, no
/// per-value branching) while the algorithms themselves are written only once.
macro_rules! dispatch_lane {
    ($width:expr, $function:ident ( $($arg:expr),* $(,)? )) => {
        match $width {
            ace_core::NumericWidth::U16 => $function::<u16>($($arg),*),
            ace_core::NumericWidth::U32 => $function::<u32>($($arg),*),
            ace_core::NumericWidth::U64 => $function::<u64>($($arg),*),
        }
    };
}

pub(crate) use dispatch_lane;

/// Returns the [`NumericWidth`](ace_core::NumericWidth) describing lane type `T`.
pub(crate) fn width_of<T: ace_bitpack::Lane>() -> ace_core::NumericWidth {
    ace_core::NumericWidth::from_byte_width(T::BYTES as u8)
        .expect("ace-bitpack lanes are exactly the NUM1 widths 2, 4 and 8")
}
