/// Summary of repeated-byte runs discovered during block analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunStats {
    /// Number of runs with length at least four.
    pub run_count: usize,
    /// Number of bytes participating in such runs.
    pub repeated_bytes: usize,
    /// Length of the longest repeated-byte run.
    pub longest_run: usize,
}

/// Scans an input slice and returns statistics for repeated-byte runs of length at least four.
pub fn analyze_runs(input: &[u8]) -> RunStats {
    if input.is_empty() { return RunStats::default(); }
    let mut stats = RunStats::default();
    let mut start = 0usize;
    while start < input.len() {
        let byte = input[start];
        let mut end = start + 1;
        while end < input.len() && input[end] == byte { end += 1; }
        let len = end - start;
        if len >= 4 {
            stats.run_count += 1;
            stats.repeated_bytes += len;
            stats.longest_run = stats.longest_run.max(len);
        }
        start = end;
    }
    stats
}
