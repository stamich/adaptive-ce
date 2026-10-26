# ACE unsafe audit

Canonical audit of the current release (0.8.0). Earlier versions: [`history/`](history/).

`unsafe_code = "forbid"` is a workspace lint. Three crates override it:

| Crate | Why | Lints kept |
|---|---|---|
| `ace-simd` (production) | runtime-dispatched x86_64 (AVX2, SSE4.2) and aarch64 (NEON, ARMv8 CRC) kernels | `unsafe_op_in_unsafe_fn = deny`, `clippy::undocumented_unsafe_blocks = deny`, `missing_safety_doc = deny`, panic lints |
| `ace-ffi` (production, 0.7.0) | exports the C ABI of `include/ace.h`: it dereferences pointers and function pointers handed over by the host | `unsafe_op_in_unsafe_fn = deny`, `undocumented_unsafe_blocks = deny`, `missing_safety_doc = deny`, panic lints |
| `ace-bench` (benchmark harness and its tests, not shipped as a library) | counting `GlobalAlloc`; the `ffi` family calls the exported C functions | `unsafe_op_in_unsafe_fn = deny`, `undocumented_unsafe_blocks = deny`, `missing_safety_doc = deny` |

**0.7.0 adds the `ace-ffi` crate**, the only place with `unsafe` outside `ace-simd`. Every exported function is `unsafe extern "C"` with a `# Safety` section, every `unsafe` block has a `// SAFETY:` comment, and the audit allows `unsafe` in `crates/ace-ffi/src/` and `crates/ace-ffi/tests/` only. The rules the host must follow are in [FFI.md](FFI.md) and [NATIVE-INTEROP.md](NATIVE-INTEROP.md). The audit script also checks the C ABI crate: every exported function runs under `guard` / `guard_value` (except five infallible accessors), is declared in `include/ace.h` and is called by a test, and the fuzz targets `ffi_decode` and `ffi_handles` exist; the table is in [PANIC-AUDIT.md](PANIC-AUDIT.md).

0.5.0 and 0.5.1 added no production `unsafe`. 0.5.1 adds one test-only allocator
(`crates/ace-bench/tests/limits_before_allocation.rs`, see below). `tools/ace-code_audit0.8.0.py`
scans production code, build scripts and tests and fails if `unsafe` appears outside the
audited files or without a `// SAFETY:` comment; the generated inventory is in
[`PANIC-AUDIT.md`](PANIC-AUDIT.md).

## Kernels

| Kernel | File | Precondition | How it is guaranteed | Equivalence test |
|---|---|---|---|---|
| `avx2::count_zeroes` | `ace-simd/src/avx2.rs` | AVX2; loads inside the slice | `scan.rs` calls it only after `is_x86_feature_detected!("avx2")` (cached); loads at `offset` with `offset + 32 <= len` | scalar comparison tests in `ace-simd` |
| `avx2::common_prefix_len` | same | AVX2; `limit <= min(len)` | caller passes `limit = min(left.len(), right.len(), max)`; loads guarded by `offset + 32 <= limit` | scalar comparison tests |
| `neon::count_zeroes` (0.8.0) | `ace-simd/src/neon.rs` | NEON; loads inside the slice | `scan.rs` calls it only after `is_aarch64_feature_detected!("neon")` (cached) and not under `ACE_SIMD=scalar`; loads at `offset` with `offset + 16 <= len`; the byte-lane accumulator is folded every 255 vectors | scalar comparison tests in `ace-simd`, ARM64 CI job |
| `neon::common_prefix_len` (0.8.0) | same | NEON; `limit <= min(len)` | same caller rule as the AVX2 kernel; loads guarded by `offset + 16 <= limit` | scalar comparison tests, ARM64 CI job |
| `crc32c::armv8::crc32c_raw` (0.8.0; `unsafe fn` with `#[target_feature(enable = "crc")]`) | `ace-simd/src/crc32c.rs` | ARMv8 CRC32 instructions | `crc32c_hardware` calls it only when `is_aarch64_feature_detected!("crc")`; the intrinsics have no memory operands | reference vectors (the same tests as SSE4.2), ARM64 CI job |
| `crc32c::sse42::crc32c_raw` (safe `#[target_feature]` fn; `unsafe` only at the call site) | `ace-simd/src/crc32c.rs` | SSE4.2 | `crc32c_hardware` calls it only when SSE4.2 is detected; the intrinsics have no memory operands | reference vectors, 3-way merge vs single chain, portable `crc32c` crate |

With the Rust 1.98.1 MSRV (target_feature 1.1, stable since 1.86) value-only intrinsics are safe inside
`#[target_feature]` functions, so `unsafe` blocks cover only the unaligned AVX2 loads and the
calls into the kernels after runtime detection. The kernels keep `chunks_exact` (clippy 1.98's
`chunks_exact_to_as_chunks` is allowed there) so that 0.5.1 leaves their code generation
untouched.

## Backend override

`ACE_SIMD=scalar` disables both kernels for the process (`ace_simd::scalar_forced`). The
golden test passes with and without it, proving byte identity of the accelerated paths.

## Allocators

| Allocator | Use | Guarantee |
|---|---|---|
| `ace-bench/src/alloc.rs` | `memory` and `allocation-profile` families | forwards every `GlobalAlloc` call unchanged to `System`; bookkeeping touches only atomics and never allocates; counting only inside an `AllocationProbe` |
| `ace-bench/src/families/ffi.rs` | `ffi` family | calls the exported `ace_engine_new`, `ace_compress`, `ace_decompress`, `ace_buffer_*` and `ace_engine_free` of `ace-ffi` exactly as a C host does; every call passes a live engine handle and slices valid for their length, and every `AceBuffer` is released once |
| `ace-bench/tests/limits_before_allocation.rs` (0.5.1, test binary only) | measures that forged sizes fail before allocation | same forwarding design; tracks live and peak bytes with atomics |
