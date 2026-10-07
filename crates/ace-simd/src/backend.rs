//! Runtime SIMD backend detection.

/// SIMD implementation selected at runtime (reported by diagnostics and benchmarks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdBackend {
    /// Portable scalar implementation.
    Scalar,
    /// x86_64 AVX2 implementation.
    Avx2,
}

/// Returns the SIMD backend selected on this host.
pub fn selected_backend() -> SimdBackend {
    if avx2_available() {
        SimdBackend::Avx2
    } else {
        SimdBackend::Scalar
    }
}

/// True when the AVX2 kernels may be called (the detection macro caches its result).
#[inline(always)]
pub(crate) fn avx2_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}
