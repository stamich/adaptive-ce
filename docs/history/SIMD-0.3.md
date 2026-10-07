# SIMD safety boundary

ACE 0.3 introduces `ace-simd` to prevent target-specific unsafe code from leaking across the project.

Safe public operations currently include runtime backend discovery, zero-byte counting and longest-common-prefix comparison. On x86_64, AVX2 is selected only after `is_x86_feature_detected!("avx2")`; all other targets use scalar implementations.

The LZ matcher uses the common-prefix primitive. The analyzer uses SIMD-assisted zero counting. Future delta, histogram and bit-shuffle implementations should be added behind the same boundary.
