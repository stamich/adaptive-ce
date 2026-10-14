# ACE 0.5.0 — Unsafe audit

`unsafe_code = "forbid"` is a workspace lint. Two crates override it:

**0.5.0 adds no `unsafe`.** The TS1 codec (Gorilla, RunDelta), the public bitstream of
`ace-bitpack`, the float / run prefilters and Planner V5 are safe Rust: every shift is
bounded by the lane width, every read goes through the bounds-checked `BitReader`, and the
inventory produced by the audit tool is unchanged (16 sites, all in the two files below).

| Crate | Why | Lints kept |
|---|---|---|
| `ace-simd` (production) | runtime-dispatched x86_64 kernels | `unsafe_op_in_unsafe_fn = deny`, `clippy::undocumented_unsafe_blocks = deny`, panic lints |
| `ace-bench` (benchmark harness, not shipped as a library) | counting `GlobalAlloc` | `unsafe_op_in_unsafe_fn = deny`, `undocumented_unsafe_blocks = deny` |

`tools/ace-code_audit0.5.0.py` fails if `unsafe` appears anywhere else or without a
`// SAFETY:` comment; the generated inventory is in `docs/PANIC-AUDIT-0.5.0.md`.

## Kernels

| Kernel | File | Precondition | How it is guaranteed | Equivalence test |
|---|---|---|---|---|
| `avx2::count_zeroes` | `ace-simd/src/avx2.rs` | AVX2; loads inside the slice | `scan.rs` calls it only after `is_x86_feature_detected!("avx2")` (cached); loads at `offset` with `offset + 32 <= len` | scalar comparison tests in `ace-simd` |
| `avx2::common_prefix_len` | same | AVX2; `limit <= min(len)` | caller passes `limit = min(left.len(), right.len(), max)`; loads guarded by `offset + 32 <= limit` | scalar comparison tests |
| `crc32c::sse42::crc32c_raw` (safe `#[target_feature]` fn; `unsafe` only at the call site) | `ace-simd/src/crc32c.rs` | SSE4.2 | `crc32c_hardware` calls it only when SSE4.2 is detected; the intrinsics have no memory operands | reference vectors, 3-way merge vs single chain, portable `crc32c` crate |

With the Rust 1.97 MSRV (target_feature 1.1) value-only intrinsics are safe inside
`#[target_feature]` functions, so `unsafe` blocks cover only the unaligned AVX2 loads and the
calls into the kernels after runtime detection.

## Backend override

`ACE_SIMD=scalar` disables both kernels for the process (`ace_simd::scalar_forced`). The
golden test passes with and without it, proving byte identity of the accelerated paths.

## Benchmark allocator

`ace-bench/src/alloc.rs` forwards every `GlobalAlloc` call unchanged to `System`; the
bookkeeping touches only atomics and never allocates. Counting is enabled only inside an
`AllocationProbe`.
