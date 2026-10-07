# ACE 0.4.6 — Unsafe audit

`unsafe_code = "forbid"` is a workspace lint. Two crates override it:

| Crate | Why | Lints kept |
|---|---|---|
| `ace-simd` (production) | runtime-dispatched x86_64 kernels | `unsafe_op_in_unsafe_fn = deny`, `clippy::undocumented_unsafe_blocks = deny`, panic lints |
| `ace-bench` (benchmark harness, not shipped as a library) | counting `GlobalAlloc` | `unsafe_op_in_unsafe_fn = deny`, `undocumented_unsafe_blocks = deny` |

`tools/ace-code_audit0.4.6.py` fails if `unsafe` appears anywhere else or without a
`// SAFETY:` comment; the generated inventory is in `docs/PANIC-AUDIT-0.4.6.md`.

## Kernels

| Kernel | File | Precondition | How it is guaranteed | Equivalence test |
|---|---|---|---|---|
| `avx2::count_zeroes` | `ace-simd/src/avx2.rs` | AVX2; loads inside the slice | `scan.rs` calls it only after `is_x86_feature_detected!("avx2")` (cached); loads at `offset` with `offset + 32 <= len` | scalar comparison tests in `ace-simd` |
| `avx2::common_prefix_len` | same | AVX2; `limit <= min(len)` | caller passes `limit = min(left.len(), right.len(), max)`; loads guarded by `offset + 32 <= limit` | scalar comparison tests |
| `crc32c::sse42::crc32c_raw` (+ `crc_u64`, `crc_u8`) | `ace-simd/src/crc32c.rs` | SSE4.2 | `crc32c_hardware` returns `None` unless SSE4.2 is detected; kernels have no memory operands beyond the input slice | reference vectors, 3-way merge vs single chain, portable `crc32c` crate |

`#[allow(unused_unsafe)]` on the kernel bodies: newer compilers treat some intrinsics as safe
inside `#[target_feature]` functions, MSRV 1.75 still requires the block.

## Backend override

`ACE_SIMD=scalar` disables both kernels for the process (`ace_simd::scalar_forced`). The
golden test passes with and without it, proving byte identity of the accelerated paths.

## Benchmark allocator

`ace-bench/src/alloc.rs` forwards every `GlobalAlloc` call unchanged to `System`; the
bookkeeping touches only atomics and never allocates. Counting is enabled only inside an
`AllocationProbe`.
