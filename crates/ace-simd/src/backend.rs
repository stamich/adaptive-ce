//! Runtime SIMD backend detection and the `ACE_SIMD=scalar` override.
//!
//! Every accelerated kernel asks this module whether it may run. Setting the environment
//! variable `ACE_SIMD=scalar` before the first ACE call forces the portable code paths on any
//! CPU; the determinism matrix uses it to prove that accelerated and scalar paths produce
//! identical bytes.

use std::sync::OnceLock;

/// Environment variable that selects the backend (`scalar` forces portable code).
pub const SIMD_OVERRIDE_ENV: &str = "ACE_SIMD";

/// SIMD implementation selected at runtime (reported by diagnostics and benchmarks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdBackend {
    /// Portable scalar implementation.
    Scalar,
    /// x86_64 AVX2 implementation.
    Avx2,
}

/// Returns the SIMD backend selected on this host (after the override).
pub fn selected_backend() -> SimdBackend {
    if avx2_available() {
        SimdBackend::Avx2
    } else {
        SimdBackend::Scalar
    }
}

/// True when `ACE_SIMD=scalar` was set when ACE first queried the backend (read once).
pub fn scalar_forced() -> bool {
    static FORCED: OnceLock<bool> = OnceLock::new();
    *FORCED.get_or_init(|| {
        std::env::var(SIMD_OVERRIDE_ENV).is_ok_and(|value| value.eq_ignore_ascii_case("scalar"))
    })
}

/// True when the AVX2 kernels may be called.
#[inline]
pub(crate) fn avx2_available() -> bool {
    !scalar_forced() && cpu_has_avx2()
}

/// True when the SSE4.2 CRC32C kernel may be called.
#[inline]
pub(crate) fn sse42_available() -> bool {
    !scalar_forced() && cpu_has_sse42()
}

/// Raw CPU capability check for AVX2 (the detection macro caches its result).
#[inline(always)]
fn cpu_has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Raw CPU capability check for SSE4.2.
#[inline(always)]
fn cpu_has_sse42() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("sse4.2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}
