//! Environment fingerprint: build toolchain, CPU, frequency policy, thermal state and load.
//!
//! Volatile values (frequency, temperature, load) are captured before and after each family
//! as [`RuntimeSnapshot`]s. Every value is best-effort — missing `/proc` or `/sys` entries
//! become `null`, never an error — so the harness runs anywhere while Linux release
//! machines get a complete fingerprint.

use std::fs;

use serde::Serialize;
use serde_json::Value;

use crate::json::JsonObjectBuilder;

/// `scaling_governor` of CPU 0.
const GOVERNOR_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
/// Current frequency of CPU 0 in kHz.
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
/// Maximum frequency of CPU 0 in kHz.
const MAX_FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq";
/// Load average per physical core above which an environment warning is emitted.
const MAX_QUIET_LOAD_PER_CORE: f64 = 0.5;

/// Volatile machine state captured around one benchmark family.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct RuntimeSnapshot {
    /// CPU 0 frequency in kHz.
    pub(crate) cpu_frequency_khz: Option<u64>,
    /// Hottest thermal zone in °C.
    pub(crate) max_thermal_c: Option<f64>,
    /// 1/5/15-minute load averages.
    pub(crate) loadavg: Option<[f64; 3]>,
}

/// Inherent methods of [`RuntimeSnapshot`].
impl RuntimeSnapshot {
    /// Reads the current frequency, temperature and load.
    pub(crate) fn capture() -> Self {
        Self {
            cpu_frequency_khz: read_u64(FREQUENCY_PATH),
            max_thermal_c: max_thermal_c(),
            loadavg: loadavg(),
        }
    }
}

/// Returns the environment fingerprint for a family measured between `before` and `after`.
///
/// Schema-2.0 fields (`os`, `arch`, `cpu_model`, …) stay at the top level; schema 2.1 adds
/// the `cpu`, `os_info`, `rust`, `build`, before/after and `warnings` entries.
pub(crate) fn environment_json(before: &RuntimeSnapshot, after: &RuntimeSnapshot) -> Value {
    let physical_cores = num_cpus::get_physical();
    let mut row = JsonObjectBuilder::new();
    row.field("os", std::env::consts::OS)
        .field("arch", std::env::consts::ARCH)
        .field("cpu_model", cpu_model())
        .field("physical_cores", physical_cores)
        .field("logical_cpus", num_cpus::get())
        .field("memory_bytes", memory_total_bytes())
        .field("build_profile", build_profile())
        .field("target_features", env!("ACE_BUILD_TARGET_FEATURES"))
        .value("cpu", cpu_json(before, after))
        .field(
            "simd_backend",
            format!("{:?}", ace_simd::selected_backend()),
        )
        .field("crc32c_backend", ace_simd::crc32c_backend_name())
        .field(
            "ace_simd_override",
            std::env::var(ace_simd::SIMD_OVERRIDE_ENV).ok(),
        )
        .value("os_info", os_json())
        .value("rust", rust_json())
        .value("build", build_json())
        .field("loadavg_before", before.loadavg)
        .field("loadavg_after", after.loadavg)
        .field("thermal_c_before", before.max_thermal_c)
        .field("thermal_c_after", after.max_thermal_c)
        .field("warnings", warnings(before, physical_cores));
    row.build()
}

/// CPU identity, detected features and frequency policy.
fn cpu_json(before: &RuntimeSnapshot, after: &RuntimeSnapshot) -> Value {
    let mut features = JsonObjectBuilder::new();
    features
        .field("avx2", cpu_has_avx2())
        .field("sse4_2", cpu_has_sse42());
    let mut row = JsonObjectBuilder::new();
    row.field("model", cpu_model())
        .field("physical_cores", num_cpus::get_physical())
        .field("logical_cpus", num_cpus::get())
        .value("features", features.build())
        .field("governor", read_trimmed(GOVERNOR_PATH))
        .field("max_freq_khz", read_u64(MAX_FREQUENCY_PATH))
        .field("freq_khz_before", before.cpu_frequency_khz)
        .field("freq_khz_after", after.cpu_frequency_khz);
    row.build()
}

/// Operating-system name and kernel release.
fn os_json() -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("name", std::env::consts::OS)
        .field("kernel", read_trimmed("/proc/sys/kernel/osrelease"));
    row.build()
}

/// Toolchain captured by `build.rs`.
fn rust_json() -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("rustc", env!("ACE_BUILD_RUSTC"))
        .field("target", env!("ACE_BUILD_TARGET"));
    row.build()
}

/// Compilation settings captured by `build.rs`.
fn build_json() -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("profile", env!("ACE_BUILD_PROFILE"))
        .field("opt_level", env!("ACE_BUILD_OPT_LEVEL"))
        .field("lto", env!("ACE_BUILD_LTO"))
        .field("codegen_units", env!("ACE_BUILD_CODEGEN_UNITS"))
        .field("target_features", env!("ACE_BUILD_TARGET_FEATURES"))
        .field("rustflags", env!("ACE_BUILD_RUSTFLAGS"))
        .field("debug_assertions", cfg!(debug_assertions));
    row.build()
}

/// Human-readable reasons why measurements on this machine may be noisy.
fn warnings(before: &RuntimeSnapshot, physical_cores: usize) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some([one_minute, _, _]) = before.loadavg {
        let limit = MAX_QUIET_LOAD_PER_CORE * physical_cores as f64;
        if one_minute > limit {
            warnings.push(format!(
                "loadavg {one_minute:.2} exceeds {limit:.2} (0.5 x physical cores)"
            ));
        }
    }
    if cfg!(debug_assertions) {
        warnings.push("debug build: timings are not representative".to_string());
    }
    if let Some(governor) = read_trimmed(GOVERNOR_PATH) {
        if governor != "performance" {
            warnings.push(format!("cpu governor is '{governor}', not 'performance'"));
        }
    }
    warnings
}

/// `"debug"` or `"release"` depending on `debug_assertions`.
pub(crate) fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

/// Whether the running CPU supports AVX2.
fn cpu_has_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// Whether the running CPU supports SSE4.2.
fn cpu_has_sse42() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("sse4.2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// First `model name` entry of `/proc/cpuinfo`.
fn cpu_model() -> Option<String> {
    fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("model name\t: ").map(str::to_string))
    })
}

/// `MemTotal` of `/proc/meminfo` in bytes.
fn memory_total_bytes() -> Option<u64> {
    fs::read_to_string("/proc/meminfo").ok().and_then(|text| {
        text.lines().find_map(|line| {
            line.strip_prefix("MemTotal:")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|kb| kb.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
    })
}

/// Hottest `thermal_zone*/temp` reading in °C.
fn max_thermal_c() -> Option<f64> {
    fs::read_dir("/sys/class/thermal")
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("thermal_zone")
        })
        .filter_map(|entry| fs::read_to_string(entry.path().join("temp")).ok())
        .filter_map(|text| text.trim().parse::<i64>().ok())
        .max()
        .map(|millicelsius| millicelsius as f64 / 1000.0)
}

/// The 1/5/15-minute load averages from `/proc/loadavg`.
fn loadavg() -> Option<[f64; 3]> {
    let text = read_trimmed("/proc/loadavg")?;
    let mut fields = text.split_whitespace().map(|v| v.parse::<f64>().ok());
    Some([fields.next()??, fields.next()??, fields.next()??])
}

/// Reads a small text file and trims it.
fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

/// Reads a small text file containing one unsigned integer.
fn read_u64(path: &str) -> Option<u64> {
    read_trimmed(path).and_then(|text| text.parse().ok())
}
