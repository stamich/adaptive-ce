//! Deterministic adaptive segmenter (Format 1.6, `SEG1`).
//!
//! A large block often changes character part-way through: a log header followed by counters,
//! a table of text followed by binary records. One plan for the whole block must compromise; a
//! plan per segment does not. The segmenter decides *where* the character changes. It never
//! decides *whether* to segment — the block encoder compares the segmented size with the
//! unsegmented size and keeps the smaller (`docs/FORMAT-1.6.md`, writer rule).
//!
//! # Method
//!
//! 1. The input is cut into windows of [`SegmenterConfig::window_size`] bytes; a remainder
//!    shorter than a window is folded into the last window, so every window is at least one
//!    window long.
//! 2. Each window gets a [`WindowClass`]: the planner route of the existing
//!    [`RoutePolicy`] plus a coarse alphabet-size bucket.
//! 3. Neighbouring windows with the same class form a run.
//! 4. Runs shorter than [`SegmenterConfig::min_segment_size`] are merged into the larger of
//!    their neighbours, smallest first, until none is left.
//! 5. Runs longer than [`SegmenterConfig::max_segment_size`] are cut into equal pieces.
//! 6. If more than [`SegmenterConfig::max_segments`] remain, the input is cut uniformly
//!    instead.
//!
//! # Determinism
//!
//! The result is a pure function of `(input, config, segmenter config)`: it uses integer
//! arithmetic only, breaks every tie towards the earlier position, and never looks at the
//! thread count, the clock or the SIMD backend, so every host cuts the same boundaries.

use std::ops::Range;

use ace_core::{AceConfig, AceError, AceResult};

use crate::{PlannerRoute, RoutePolicy};

/// Coarse alphabet size of a window: the number of distinct byte values, bucketed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphabetBucket {
    /// One byte value (constant data).
    Constant,
    /// At most 16 distinct values.
    Tiny,
    /// At most 64 distinct values (text, hex, base-64).
    Small,
    /// At most 192 distinct values.
    Medium,
    /// More than 192 distinct values (binary, compressed, random).
    Wide,
}

/// What a window looks like to the planner: the route it would be given and its alphabet.
/// Two neighbouring windows with equal classes are encoded well by one plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowClass {
    /// Route of [`RoutePolicy::classify`] for the window.
    pub route: PlannerRoute,
    /// Distinct-byte bucket of the window.
    pub alphabet: AlphabetBucket,
}

/// Window and segment sizes of the segmenter.
///
/// All sizes are in bytes. `min_segment_size` and `max_segment_size` must be multiples of
/// `window_size`, because a segment is a whole number of windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmenterConfig {
    /// Granularity of the classification (default 64 KiB).
    pub window_size: usize,
    /// Smallest segment the segmenter produces, unless the input itself is shorter
    /// (default 256 KiB).
    pub min_segment_size: usize,
    /// Largest segment the segmenter produces, unless `max_segments` forces larger ones
    /// (default 4 MiB).
    pub max_segment_size: usize,
    /// Most segments in one block (default 1 024, the `DecodeLimits` default).
    pub max_segments: usize,
}

/// Defaults tuned for the 16 MiB blocks of the default configuration.
impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            window_size: 64 * 1024,
            min_segment_size: 256 * 1024,
            max_segment_size: 4 * 1024 * 1024,
            max_segments: 1024,
        }
    }
}

/// Inherent methods of [`SegmenterConfig`].
impl SegmenterConfig {
    /// Checks the relations the algorithm relies on.
    ///
    /// # Errors
    /// [`AceError::InvalidConfig`] when `window_size` is zero, the minimum or the maximum is not
    /// a non-zero multiple of the window, the maximum is smaller than twice the minimum (a
    /// segment cut in two must stay above the minimum), or fewer than two segments are allowed.
    pub fn validate(&self) -> AceResult<()> {
        let multiple = |size: usize| {
            self.window_size != 0 && size != 0 && size.is_multiple_of(self.window_size)
        };
        if !multiple(self.min_segment_size) || !multiple(self.max_segment_size) {
            return Err(AceError::InvalidConfig(
                "segment sizes must be non-zero multiples of the window size",
            ));
        }
        if self.max_segment_size / 2 < self.min_segment_size {
            return Err(AceError::InvalidConfig(
                "max segment size must be at least twice the min segment size",
            ));
        }
        if self.max_segments < 2 {
            return Err(AceError::InvalidConfig("at least two segments must be allowed"));
        }
        Ok(())
    }
}

/// A run of windows with one class: first window, window count, class.
#[derive(Debug, Clone, Copy)]
struct Run {
    /// Index of the first window.
    start: usize,
    /// Number of windows.
    windows: usize,
    /// Class of the run.
    class: WindowClass,
}

/// Classifies one window: the planner route plus the alphabet bucket.
pub fn classify_window(window: &[u8], config: &AceConfig) -> WindowClass {
    let mut seen = [false; 256];
    for &byte in window {
        seen[usize::from(byte)] = true;
    }
    let distinct = seen.iter().filter(|&&present| present).count();
    let alphabet = match distinct {
        0..=1 => AlphabetBucket::Constant,
        2..=16 => AlphabetBucket::Tiny,
        17..=64 => AlphabetBucket::Small,
        65..=192 => AlphabetBucket::Medium,
        _ => AlphabetBucket::Wide,
    };
    WindowClass {
        route: RoutePolicy::classify(window, config).route,
        alphabet,
    }
}

/// Cuts `input` into segments and returns their byte ranges, which tile `0..input.len()`.
///
/// An empty input gives no ranges. An input shorter than two windows, or one whose windows all
/// have the same class and fit in one segment, gives one range covering the whole input — "do
/// not segment".
///
/// # Errors
/// [`AceError::InvalidConfig`] for an invalid [`SegmenterConfig`].
pub fn segment_boundaries(
    input: &[u8],
    config: &AceConfig,
    segmenter: &SegmenterConfig,
) -> AceResult<Vec<Range<usize>>> {
    segmenter.validate()?;
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let window = segmenter.window_size;
    let windows = (input.len() / window).max(1);
    let window_bytes = |index: usize| {
        let start = index * window;
        let end = if index + 1 == windows { input.len() } else { start + window };
        &input[start..end]
    };
    let mut runs: Vec<Run> = Vec::new();
    for index in 0..windows {
        let class = classify_window(window_bytes(index), config);
        match runs.last_mut() {
            Some(last) if last.class == class => last.windows += 1,
            _ => runs.push(Run { start: index, windows: 1, class }),
        }
    }
    merge_small_runs(&mut runs, segmenter.min_segment_size / window);
    let mut pieces = split_large_runs(&runs, segmenter.max_segment_size / window);
    if pieces.len() > segmenter.max_segments {
        pieces = uniform_pieces(windows, segmenter);
    }
    let byte_offset = |window_index: usize| {
        if window_index == windows {
            input.len()
        } else {
            window_index * window
        }
    };
    Ok(pieces
        .iter()
        .map(|&(start, count)| byte_offset(start)..byte_offset(start + count))
        .collect())
}

/// Merges runs shorter than `min_windows` into their larger neighbour, smallest first.
fn merge_small_runs(runs: &mut Vec<Run>, min_windows: usize) {
    while runs.len() > 1 {
        // The smallest short run; the earliest one wins a tie.
        let Some(index) = runs
            .iter()
            .enumerate()
            .filter(|(_, run)| run.windows < min_windows)
            .min_by_key(|(position, run)| (run.windows, *position))
            .map(|(position, _)| position)
        else {
            break;
        };
        let into = if index == 0 {
            1
        } else if index + 1 == runs.len() || runs[index - 1].windows >= runs[index + 1].windows {
            index - 1
        } else {
            index + 1
        };
        let small = runs.remove(index);
        let target = if into > index { into - 1 } else { into };
        let kept = &mut runs[target];
        kept.windows += small.windows;
        kept.start = kept.start.min(small.start);
        coalesce(runs);
    }
}

/// Joins neighbouring runs that ended up with the same class.
fn coalesce(runs: &mut Vec<Run>) {
    let mut joined: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs.drain(..) {
        match joined.last_mut() {
            Some(last) if last.class == run.class => last.windows += run.windows,
            _ => joined.push(run),
        }
    }
    *runs = joined;
}

/// Cuts every run longer than `max_windows` into equal pieces; returns `(first window, count)`.
fn split_large_runs(runs: &[Run], max_windows: usize) -> Vec<(usize, usize)> {
    let mut pieces = Vec::with_capacity(runs.len());
    for run in runs {
        let parts = run.windows.div_ceil(max_windows);
        pieces.extend(even_split(run.start, run.windows, parts));
    }
    pieces
}

/// The fallback when the classification yields too many segments: equal pieces of the whole
/// input, as small as `max_segment_size` allows but never more than `max_segments`.
fn uniform_pieces(windows: usize, segmenter: &SegmenterConfig) -> Vec<(usize, usize)> {
    let max_windows = segmenter.max_segment_size / segmenter.window_size;
    let parts = windows
        .div_ceil(max_windows)
        .max(2)
        .min(segmenter.max_segments)
        .min(windows);
    even_split(0, windows, parts)
}

/// Splits `windows` windows starting at `start` into `parts` pieces whose counts differ by at
/// most one (the earlier pieces are the larger ones).
fn even_split(start: usize, windows: usize, parts: usize) -> Vec<(usize, usize)> {
    let parts = parts.clamp(1, windows.max(1));
    let (base, extra) = (windows / parts, windows % parts);
    let mut next = start;
    (0..parts)
        .map(|index| {
            let count = base + usize::from(index < extra);
            let piece = (next, count);
            next += count;
            piece
        })
        .collect()
}
