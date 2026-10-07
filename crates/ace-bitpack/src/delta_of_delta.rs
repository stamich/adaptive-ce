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

#[cfg(test)]
mod tests {
    use super::*;

    /// DoD round-trips for counters and is all-zero for fixed steps, including a lane wrap.
    #[test]
    fn dod_roundtrip() {
        let values = vec![1000u32, 1003, 1006, 1009, 1012, 1016];
        let (first, dod) = delta_of_delta(&values);
        assert_eq!(undelta_of_delta(values[0], first, &dod), values);

        let wrapping: Vec<u16> = vec![65_533, 65_534, 65_535, 0, 1, 2];
        let (first, dod) = delta_of_delta(&wrapping);
        assert_eq!(dod, vec![0, 0, 0, 0]);
        assert_eq!(undelta_of_delta(wrapping[0], first, &dod), wrapping);
        assert_eq!(delta_of_delta::<u64>(&[7]), (0, vec![]));
    }
}
