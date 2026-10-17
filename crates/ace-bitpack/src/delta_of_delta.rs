//! Second-order wrapping deltas ("delta-of-delta", DoD), ideal for regular timestamps.

use crate::Lane;

/// Returns `(first_delta, dod)` where `dod[i] = d[i+1] - d[i]` in the lane ring.
///
/// Sequences shorter than two values have neither deltas nor DoD values.
pub fn delta_of_delta<T: Lane>(values: &[T]) -> (T::Signed, Vec<T::Signed>) {
    if values.len() < 2 {
        return (T::Signed::default(), Vec::new());
    }
    let first_delta = values[1].wrapping_sub(values[0]);
    let mut previous = first_delta;
    let mut out = Vec::with_capacity(values.len() - 2);
    for pair in values[1..].windows(2) {
        let step = pair[1].wrapping_sub(pair[0]);
        out.push(step.wrapping_sub(previous).to_signed());
        previous = step;
    }
    (first_delta.to_signed(), out)
}

/// Reconstructs values from the first value, first delta and DoD stream.
pub fn undelta_of_delta<T: Lane>(first: T, first_delta: T::Signed, dod: &[T::Signed]) -> Vec<T> {
    undelta_of_delta_iter(first, first_delta, dod.iter().copied()).collect()
}

/// Lazy form of [`undelta_of_delta`]: yields `first`, `first + first_delta`, then one value per
/// DoD entry.
pub fn undelta_of_delta_iter<T: Lane>(
    first: T,
    first_delta: T::Signed,
    dod: impl Iterator<Item = T::Signed>,
) -> impl Iterator<Item = T> {
    let first_step = T::from_signed(first_delta);
    let second = first.wrapping_add(first_step);
    let rest = dod.scan((second, first_step), |(current, step), change| {
        *step = step.wrapping_add(T::from_signed(change));
        *current = current.wrapping_add(*step);
        Some(*current)
    });
    [first, second].into_iter().chain(rest)
}
