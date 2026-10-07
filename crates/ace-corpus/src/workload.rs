//! The closed set of named workloads and their metadata.

use crate::generators;

/// One deterministic synthetic workload.
///
/// Names are stable identifiers used in benchmark JSON, golden files and on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Workload {
    /// All zero bytes.
    Zeros,
    /// Repeating `0,1,2,3` (four-symbol alphabet).
    LowCardinality,
    /// 4 KiB runs of one byte, value advancing by 17.
    Runs,
    /// Little-endian u32 counter with step 3 (NumericFast).
    U32Counter,
    /// u64 values with small varying positive steps.
    DeltaSeries,
    /// Repeated JSON record.
    StructuredJson,
    /// Incompressible xorshift bytes.
    Random,
    /// Quarters of zeros, u32 ramp, JSON and random bytes.
    Mixed,
    /// u64 millisecond-style timestamps with step 1000 + (i % 3) (NumericGeneral).
    U64TimestampsMs,
    /// u64 values with constant step 1000 (NumericFast).
    U64FixedStep,
    /// u64 nanosecond timestamps with up to 1 ms jitter (lane-relative prefilter).
    U64TimestampsNs,
    /// u32 sawtooth `100000 + (i % 4096)` (generic LZ wins).
    GaugeSawtooth,
    /// u32 counter with a large outlier jump every 1024 values.
    MonotonicOutliers,
    /// u32 counter with step `(i % 17) + 1`.
    DeltaVariable,
    /// u16 slowly varying samples that wrap around 65535.
    U16Wrapping,
}

impl Workload {
    /// Every workload, in documentation order.
    pub const ALL: [Workload; 15] = [
        Self::Zeros,
        Self::LowCardinality,
        Self::Runs,
        Self::U32Counter,
        Self::DeltaSeries,
        Self::StructuredJson,
        Self::Random,
        Self::Mixed,
        Self::U64TimestampsMs,
        Self::U64FixedStep,
        Self::U64TimestampsNs,
        Self::GaugeSawtooth,
        Self::MonotonicOutliers,
        Self::DeltaVariable,
        Self::U16Wrapping,
    ];

    /// Stable identifier.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zeros => "zeros",
            Self::LowCardinality => "low-cardinality",
            Self::Runs => "runs",
            Self::U32Counter => "u32-counter",
            Self::DeltaSeries => "delta-series",
            Self::StructuredJson => "structured-json",
            Self::Random => "random",
            Self::Mixed => "mixed",
            Self::U64TimestampsMs => "u64-timestamps",
            Self::U64FixedStep => "u64-fixed-step",
            Self::U64TimestampsNs => "u64-timestamps-ns",
            Self::GaugeSawtooth => "gauge-sawtooth",
            Self::MonotonicOutliers => "monotonic-outliers",
            Self::DeltaVariable => "delta-variable",
            Self::U16Wrapping => "u16-wrapping",
        }
    }

    /// Resolves a name; historical aliases (`numeric-u32`, `u64-timestamps-ms`) are accepted.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "numeric-u32" => Some(Self::U32Counter),
            "u64-timestamps-ms" => Some(Self::U64TimestampsMs),
            other => Self::ALL
                .into_iter()
                .find(|workload| workload.name() == other),
        }
    }

    /// Generates exactly `bytes` bytes of this workload.
    pub fn generate(self, bytes: usize) -> Vec<u8> {
        match self {
            Self::Zeros => vec![0u8; bytes],
            Self::LowCardinality => generators::low_cardinality(bytes),
            Self::Runs => generators::runs(bytes),
            Self::U32Counter => generators::u32_counter(bytes),
            Self::DeltaSeries => generators::delta_series(bytes),
            Self::StructuredJson => generators::structured_json(bytes),
            Self::Random => generators::random(bytes),
            Self::Mixed => generators::mixed(bytes),
            Self::U64TimestampsMs => generators::u64_timestamps_ms(bytes),
            Self::U64FixedStep => generators::u64_fixed_step(bytes),
            Self::U64TimestampsNs => generators::u64_timestamps_ns(bytes),
            Self::GaugeSawtooth => generators::gauge_sawtooth(bytes),
            Self::MonotonicOutliers => generators::monotonic_outliers(bytes),
            Self::DeltaVariable => generators::delta_variable(bytes),
            Self::U16Wrapping => generators::u16_wrapping(bytes),
        }
    }
}

impl std::fmt::Display for Workload {
    /// Writes the stable identifier.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names round-trip, aliases resolve, every generator honours the requested size.
    #[test]
    fn names_aliases_and_sizes() {
        for workload in Workload::ALL {
            assert_eq!(Workload::from_name(workload.name()), Some(workload));
            for bytes in [0usize, 1, 7, 4097, 70_001] {
                assert_eq!(workload.generate(bytes).len(), bytes, "{workload}");
            }
            assert_eq!(
                workload.generate(10_000),
                workload.generate(10_000),
                "{workload} deterministic"
            );
        }
        assert_eq!(
            Workload::from_name("numeric-u32"),
            Some(Workload::U32Counter)
        );
        assert_eq!(Workload::from_name("nope"), None);
    }
}
