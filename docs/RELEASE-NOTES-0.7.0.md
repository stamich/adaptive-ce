# ACE 0.7.0 — release notes (Embedding)

0.7.0 lets other languages use ACE. It adds a C ABI (`crates/ace-ffi`, `include/ace.h`), a Java
SDK on Panama (`java.lang.foreign`) and a Scala 3 SDK, native packaging inside the JAR, and the
checks that keep all of them honest. **The bytes do not change:** ACE 0.7.0 writes exactly the
files of 0.6.1 (Format 1.5, the same planner), and `examples/golden/0.6.1/` is reproduced by the
Rust API, the C ABI, the Java SDK and the Scala SDK.

Previous release: [`RELEASE-NOTES-0.6.1.md`](RELEASE-NOTES-0.6.1.md).

## Compatibility

| | 0.7.0 vs 0.6.1 |
|---|---|
| Written bytes | identical (golden 0.6.1, 31 workloads × 3 profiles) |
| Formats read / written | 1.0 – 1.5 / 1.3, 1.4, 1.5 (unchanged) |
| Rust API (`ace`) | unchanged |
| CLI | unchanged |
| C ABI | new: version 1, 26 functions, `include/ace.h` |
| JVM | new: JDK 22 or newer (tested on 22 and 25), Scala 3.3 LTS |
| Benchmark documents | schema 2.2 (unchanged); new family `ffi` |

## What is new

### C ABI v1

* `crates/ace-ffi` builds `libace_ffi` (`.so`, `.dylib`, `.dll`) and `libace_ffi.a`;
  `include/ace.h` is generated from it by cbindgen and CI fails when the committed header
  differs.
* 26 functions: engine, one-shot compress and decompress (also into a caller buffer), streaming
  with read and write callbacks, random access (`AceRangeReader`), the planner report, and
  accessors for the version, the last error and buffers. Conventions (status codes, thread-local
  last error, opaque handles, library-owned buffers, `struct_size` on `AceConfig` and
  `AceLimits`): [`NATIVE-INTEROP.md`](NATIVE-INTEROP.md); function list and JSON shapes:
  [`FFI.md`](FFI.md).
* No panic crosses the boundary: every entry point except five infallible accessors runs under
  `catch_unwind` and reports `ACE_ERR_PANIC`. Callbacks are `extern "C-unwind"`, so a Rust host
  callback that panics is contained as well.
* `ace_abi_version()` is checked by every binding before it looks up another symbol.
* `unsafe` is confined to `ace-ffi` and `ace-simd` (plus the benchmark harness); the audits
  ([`UNSAFE-AUDIT.md`](UNSAFE-AUDIT.md), [`PANIC-AUDIT.md`](PANIC-AUDIT.md)) check the new
  crate: guards, header declarations, tests and fuzz targets for every exported function.

### JVM

| Module | Language | Content |
|---|---|---|
| `jvm/ace-native` | Java | generated Panama bindings (`tools/ace-genbindings0.7.0.py`), library loading, the Java SDK: `Engine`, `Config`, `DecodeLimits`, `RangeReader`, `Stats`, zero-copy `MemorySegment` calls and `byte[]` convenience calls |
| `jvm/ace-model` | Scala 3 | pure types: configuration, statistics, the `AceError` ADT |
| `jvm/ace-sdk` | Scala 3 | `Either[AceError, A]` API, `Ace.use` scopes, off-heap input and output, range readers |
| `jvm/ace-stream` | Scala 3 | `InputStream`, `OutputStream` and block `Iterator` adapters (no stream-library dependency) |
| `jvm/ace-tier` | Scala 3 | hot / warm / cold encoder settings and neutral metrics (ratio, encode and decode ns per byte); no AdaptiveDB dependency |

Native handles are closed explicitly; a `Cleaner` releases forgotten ones as a safety net. The
native library is looked up in this order: system property `ace.native.path`, environment
variable `ACE_NATIVE_PATH`, the resource `native/<os>-<arch>/` inside the JAR (extracted to a
temporary file), then the library path. Demos: [`../demo/jvm/`](../demo/jvm/README.md).

### Tests, fuzzing and CI

* Rust: every C function through the `extern "C"` API, a misuse matrix, panic containment, the
  golden file through the C ABI, a C smoke program (dynamic, static and C++ header builds).
* JVM: round trips under every profile, zero-copy and `decompressInto`, streams, range reads,
  misuse (double close, use after close, null arguments, concurrent close), a native-leak check,
  packaging from a clean directory, and the golden file for Java and Scala.
* Fuzz: 20 targets (new: `ffi_decode`, `ffi_handles`).
* CI: header drift, generated-bindings drift and the C smoke test in the `pr` job; a native
  aarch64 job that reproduces the golden bytes (scalar kernels, correctness only; no NEON
  backend); a JVM job on JDK 22 and 25.
* Benchmarks: family `ffi` (Rust facade against the C ABI on the mixed 16 MiB workload and on
  64 B – 16 KiB buffers) and the JVM benchmark `FfiBenchmark`; Regression V3 section `ffi`:
  C ABI ≥ 0.97× of the facade on 16 MiB, JVM zero-copy path ≥ 0.90×, call overhead reported.

## What did not change

Planner V5, every codec, the container formats and the Rust API. Differences against 0.6.1 are
confined to speed measurements of the existing paths, which the interleaved A/B run guards
(no case below 0.95×).

## Known limits

* The JVM needs `--enable-native-access=ALL-UNNAMED` (or the module name).
* A `RangeReader` keeps a one-block cache: one thread at a time. An `Engine` may be shared.
* `ace_compress_stream` needs the input size up front and a fixed block size policy, as the
  Rust streaming encoder does; the compressed file is assembled in memory before it is written.
* Native libraries for macOS and Windows are not built in CI yet; the JAR packaging layout
  supports them (`native/macos-aarch64/`, `native/windows-x86_64/`).
* The NEON backend for aarch64 is not part of 0.7.0.

## Qualification

Release run on the reference machine (AMD Ryzen 9 5950X, `performance` governor, revision
3492a07, clean tree); details in [`PERFORMANCE-0.7.0.md`](PERFORMANCE-0.7.0.md), data in
`examples/baselines/0.7.0/`.

| Check | Verdict |
|---|---|
| Regression V3 | **pass**: 99 pass, 12 diagnostic, 0 fail / unstable / skipped |
| Interleaved A/B vs 0.6.1 | **pass**, 20 / 20 cases, minimum ratio 0.991, byte-identical output |
| Repeatability | **pass**, 3 runs × 31 families, deterministic values identical |
| C ABI vs Rust facade (16 MiB) | compress 1.008×, decompress 1.008× (gate ≥ 0.97×) |
| JVM zero-copy vs facade (16 MiB) | compress 1.020×, decompress 0.936× (gate ≥ 0.90×) |
| Fuzz | 20 / 20 targets × 600 s, no crash, 4.66 × 10⁹ executions; the 0.6.1 `ts1_*` wall-time anomaly did not reproduce ([`FUZZ.md`](../examples/baselines/0.7.0/FUZZ.md)) |

JVM zero-copy decompression moved between 0.90× and 0.97× of the facade over the qualification
runs: a native buffer returned to the caller is released through a shared arena, which costs
0.1 – 0.4 ms on a 3.4 ms call. The 0.90× gate held in the release run; it is not a throughput
regression of the C ABI, which matches the facade.
