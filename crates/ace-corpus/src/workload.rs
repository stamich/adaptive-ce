//! The closed set of named workloads and their metadata.

use crate::{float_generators, generators};

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
    /// Corpus V4: f64 constant 21.5.
    F64Constant,
    /// Corpus V4: f64 plateaus of 1 000 samples moving by ±0.5.
    F64Step,
    /// Corpus V4: f64 integrated small increments (slowly changing series).
    F64Smooth,
    /// Corpus V4: f64 temperature sensor quantised to 0.1.
    F64SensorTemperature,
    /// Corpus V4: f64 price random walk (`cents / 100`).
    F64FinancialPrice,
    /// Corpus V4: f64 smooth series with noise in the 20 lowest mantissa bits.
    F64Noisy,
    /// Corpus V4: random NaN-free f64 bit patterns (fallback case).
    F64Random,
    /// Corpus V4: f64 special values (±0, NaN payloads, ±∞, subnormals, extremes).
    F64Special,
    /// Corpus V4: f32 slowly changing series.
    F32Smooth,
    /// Corpus V4: f32 sensor quantised to 0.01.
    F32Sensor,
    /// Corpus V4: u32 value that rarely changes.
    IntSparseChange,
    /// Corpus V4: u64 counter with periodic resets.
    IntCounterReset,
}

impl Workload {
    /// Corpus V4 (ACE 0.5.0): floating-point series and sparse-change integers.
    pub const CORPUS_V4: [Workload; 12] = [
        Self::F64Constant,
        Self::F64Step,
        Self::F64Smooth,
        Self::F64SensorTemperature,
        Self::F64FinancialPrice,
        Self::F64Noisy,
        Self::F64Random,
        Self::F64Special,
        Self::F32Smooth,
        Self::F32Sensor,
        Self::IntSparseChange,
        Self::IntCounterReset,
    ];

    /// Every workload: the 0.4.x corpus ([`Self::CORPUS_V3`]) followed by [`Self::CORPUS_V4`].
    pub const ALL: [Workload; 27] = [
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
        Self::F64Constant,
        Self::F64Step,
        Self::F64Smooth,
        Self::F64SensorTemperature,
        Self::F64FinancialPrice,
        Self::F64Noisy,
        Self::F64Random,
        Self::F64Special,
        Self::F32Smooth,
        Self::F32Sensor,
        Self::IntSparseChange,
        Self::IntCounterReset,
    ];

    /// The 0.4.x corpus (Corpus V3), covered by the 0.4.6 golden file.
    pub const CORPUS_V3: [Workload; 15] = [
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
            Self::F64Constant => "f64-constant",
            Self::F64Step => "f64-step",
            Self::F64Smooth => "f64-smooth",
            Self::F64SensorTemperature => "f64-sensor-temperature",
            Self::F64FinancialPrice => "f64-financial-price",
            Self::F64Noisy => "f64-noisy",
            Self::F64Random => "f64-random",
            Self::F64Special => "f64-special",
            Self::F32Smooth => "f32-smooth",
            Self::F32Sensor => "f32-sensor",
            Self::IntSparseChange => "int-sparse-change",
            Self::IntCounterReset => "int-counter-reset",
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
            Self::F64Constant => float_generators::f64_constant(bytes),
            Self::F64Step => float_generators::f64_step(bytes),
            Self::F64Smooth => float_generators::f64_smooth(bytes),
            Self::F64SensorTemperature => float_generators::f64_sensor_temperature(bytes),
            Self::F64FinancialPrice => float_generators::f64_financial_price(bytes),
            Self::F64Noisy => float_generators::f64_noisy(bytes),
            Self::F64Random => float_generators::f64_random(bytes),
            Self::F64Special => float_generators::f64_special(bytes),
            Self::F32Smooth => float_generators::f32_smooth(bytes),
            Self::F32Sensor => float_generators::f32_sensor(bytes),
            Self::IntSparseChange => float_generators::int_sparse_change(bytes),
            Self::IntCounterReset => float_generators::int_counter_reset(bytes),
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
