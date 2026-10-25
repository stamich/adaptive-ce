# Adaptive Compression Engine (ACE)

[![Build Status](https://github.com/stamich/adaptive-ce/actions/workflows/ace-ci0.8.0.yml/badge.svg?branch=main)](https://github.com/stamich/adaptive-ce/actions/workflows/ace-ci0.8.0.yml)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![Coverage](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=coverage)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![Lines of Code](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=ncloc)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![Reliability Rating](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=reliability_rating)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![Security Rating](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=security_rating)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![Maintainability Rating](https://sonarcloud.io/api/project_badges/measure?project=stamich_adaptive-ce&metric=sqale_rating)](https://sonarcloud.io/summary/new_code?id=stamich_adaptive-ce)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

ACE is a lossless, schema-free, adaptive block compressor written in Rust. It looks at every
block of the input — text, structured records, integer counters and timestamps, floating-point
series — chooses the cheapest reversible pipeline for it with a deterministic cost model, and
stores the result in a self-describing container with a block index for random access.

The same input and configuration always produce the same bytes, on every machine, thread count
and SIMD backend. Every byte read from a file is validated before it is trusted.

**Current release: 0.7.0 — Embedding.** A C ABI (`include/ace.h`, `libace_ffi`), a Java SDK on
Panama and a Scala 3 SDK let JVM and C programs use ACE. The bytes are those of 0.6.1 (Format 1.5:
Patched frame-of-reference for integers with outliers, ALP for decimal floats, small-delta
RunDelta, DENSE never larger than BALANCED), reproduced through every binding. Files written by
every earlier version stay readable. See [`docs/RELEASE-NOTES-0.7.0.md`](docs/RELEASE-NOTES-0.7.0.md)
and, coming from 0.6.1, [`docs/MIGRATION-0.6.1-TO-0.7.0.md`](docs/MIGRATION-0.6.1-TO-0.7.0.md).

## What it does well

| Data | How ACE stores it | Example (BALANCED) |
|---|---|---|
| text, logs, JSON, mixed binary | LZ / RLE / delta + Huffman, rANS or 4-lane rANS, chosen per block | mixed corpus 16 MiB: ≈ 3.7× |
| integer counters, timestamps, IDs | NUM1: frame-of-reference, delta or delta-of-delta + bit packing (u16 / u32 / u64), Patched FOR for outliers | u32 counter: ≈ 2 300×; u64 timestamps: ≈ 32×; counter with resets: ≈ 21×; monotonic with outliers: ≈ 400× |
| floating-point series | TS1 Gorilla XOR, or ALP for decimal values; bit-exact for f64 / f32 (NaN payloads, −0.0, subnormals) | constant f64: ≈ 2 400×; decimal prices: ≈ 5× |
| rarely changing values | TS1 RunDelta (Elias-gamma runs + ZigZag deltas) | sparse-change integers: ≈ 235× |
| incompressible data | stored RAW (never larger than the input plus framing) | random: 1.0× |

Measured numbers and methodology: [`docs/PERFORMANCE-0.7.0.md`](docs/PERFORMANCE-0.7.0.md)
(reference machine AMD Ryzen 9 5950X; 0.7.0 writes the same bytes), [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md).

## Quick start

```bash
cargo build --release                                  # builds the `ace` binary
./target/release/ace compress data.bin data.ace        # --profile fast | balanced | dense
./target/release/ace decompress data.ace restored.bin
./target/release/ace version                           # version, formats, backends, git revision
./demo/ace-run-demo0.8.0.sh                            # one-minute product tour
```

Requirements: Rust 1.98.1 or newer (MSRV); the repository pins the build toolchain to 1.98.1
in `rust-toolchain.toml` (never use 1.98.0). Python 3.9+ for the benchmark and release tools.

## Command line

```bash
ace compress in.bin out.ace --profile balanced --threads 0   # 0 = all CPUs; output is identical for any count
ace compress in.bin out.ace --disable-float                  # 0.4.6 decisions, always Format 1.3
ace compress-stream in.bin out.ace                           # bounded memory, same bytes
ace decompress out.ace restored.bin
ace verify out.ace                                           # decode everything, check every CRC
ace inspect out.ace --blocks                                 # format version, per-block codecs
ace explain in.bin                                           # routes and planner decisions per block
ace read-range out.ace 1000000 65536 part.bin                # random access: decodes only the touched blocks
ace decode-block out.ace 7 block.bin
ace version
```

Exit status: 0 success (also when the output pipe is closed early, e.g. `ace explain … | head`),
1 error (message on stderr), 2 usage error. An output path that names the input file is refused.

## Library

```rust
use std::io::Cursor;
use ace::{Config, DecodeLimits, Engine, Profile};

let engine = Engine::new(Config::builder().profile(Profile::Balanced).build()?)?;
let packed = engine.compress(&input)?;
let restored = engine.decompress(&packed)?;              // or decompress_into(&packed, &mut buf)

let mut reader = engine.open_range_reader(Cursor::new(&packed))?;
let slice = reader.read_range(4_000..4_100)?;            // decodes only the touched blocks

let strict = Engine::default()                           // untrusted input: explicit limits
    .with_decode_limits(DecodeLimits::builder().max_output_size(64 << 20).build());
```

The public API is the `ace` crate: `Engine`, `Config`, `DecodeLimits`, `RangeReader`, `Stats`
and `Error` (with `ErrorKind` to act on). Every other crate is an internal building block
([`docs/API-INVENTORY.md`](docs/API-INVENTORY.md)). The example is compiled and run as a
doctest of `ace`; coming from 0.5: [`docs/MIGRATION-0.5-TO-0.6.md`](docs/MIGRATION-0.5-TO-0.6.md).
## C and JVM

```c
#include "ace.h"                                     /* include/ace.h, generated by cbindgen */
AceEngine *engine = NULL;  AceBuffer *packed = NULL;
ace_engine_new(NULL, NULL, &engine);                 /* NULL = defaults */
ace_compress(engine, data, len, &packed, NULL);
/* ace_buffer_data(packed), ace_buffer_len(packed) ... */
ace_buffer_free(packed);  ace_engine_free(engine);
```

```java
try (Engine engine = Engine.create(Config.builder().profile(Profile.BALANCED).build())) {
    byte[] packed = engine.compress(input);              // or MemorySegment in, NativeBuffer out
    byte[] restored = engine.decompress(packed);
}
```

```scala
Ace.use(AceConfig(profile = Profile.Dense)) { codec =>   // Either[AceError, A]
  codec.compress(input).flatMap(codec.decompress)
}
```

The C ABI conventions are in [`docs/NATIVE-INTEROP.md`](docs/NATIVE-INTEROP.md), the functions in
[`docs/FFI.md`](docs/FFI.md). The JVM needs JDK 22 or newer and `--enable-native-access=ALL-UNNAMED`;
build with `gradle -p jvm build` (it compiles `libace_ffi` with Cargo). Modules: `ace-native`
(Java), `ace-model`, `ace-sdk`, `ace-stream`, `ace-tier` (Scala 3). Demos:
[`demo/jvm/`](demo/jvm/README.md).

## How it works

```text
input ─► blocks ─► route (NumericFast | FloatFast | FloatGeneral | NumericGeneral | Generic)
                                   │
          Planner V5: analysis, analytical estimates, sampled verification, cost model,
          TS1 dominance policy — deterministic, no wall clock, fixed tie-breaks
                                   │
     transforms ─► codec (RAW / RLE / LZ / NUM1 / TS1) ─► entropy (Huffman / rANS / rANS4x)
                                   │
   container: header · blocks (each with CRC-32C) · block index · trailer
   Format 1.3, 1.4 when a block uses TS1, 1.5 when it uses Patched FOR or ALP (the minimal version the file needs)
```

Blocks are independent, so compression runs in parallel and any byte range can be decoded
without reading the blocks before it. Details: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
[`docs/PLANNER-V5.md`](docs/PLANNER-V5.md), [`docs/FORMAT.md`](docs/FORMAT.md),
[`docs/TS1-CODEC.md`](docs/TS1-CODEC.md).

## Guarantees

| Guarantee | How it is enforced |
|---|---|
| Lossless, bit-exact | every block carries the CRC-32C of its bytes; property tests over arbitrary inputs and float bit patterns |
| Deterministic output | golden SHA-256 files (auto and scalar SIMD), determinism matrix (1 – 32 threads, API paths, processes) |
| Old files stay readable | frozen files written by 0.2.1, 0.3.0, 0.4.4, 0.4.6, 0.5.0 and 0.6.1 decode on every build ([`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md)) |
| Hostile input is safe | `DecodeLimits` checked before every allocation, checked arithmetic in every parser, no panic in production code (and none across the C ABI), 22 fuzz targets ([`docs/SECURITY.md`](docs/SECURITY.md), [`docs/DECODER-AUDIT.md`](docs/DECODER-AUDIT.md)) |
| Minimal `unsafe` | only the audited SIMD / CRC kernels and the C ABI boundary (`ace-ffi`) ([`docs/UNSAFE-AUDIT.md`](docs/UNSAFE-AUDIT.md), [`docs/PANIC-AUDIT.md`](docs/PANIC-AUDIT.md)) |

## Development

```bash
./ace-build0.8.0.sh                                  # fmt, clippy, tests, release build, rustdoc (incl. private items), demo
./ace-ci0.8.0.sh pr | aarch64 | jvm | fuzz-smoke | fuzz   # CI entry points (GitHub Actions wraps them)
./ace-benchmark0.8.0.sh all [--quick] [--repeat 3]   # Harness V3 + Regression V3
./ace-ab0.8.0.sh /path/to/ace-0.6.1                  # interleaved A/B against the previous release
./ace-benchmark-compare0.8.0.sh OLD NEW              # any two result files or directories
./ace-release0.8.0.sh /path/to/ace-0.6.1             # release pipeline ([`docs/RELEASE.md`](docs/RELEASE.md))
gradle -p jvm build                                  # Java and Scala SDKs (JDK 22+, builds libace_ffi)
```

| Path | Content |
|---|---|
| `crates/` | 20 crates: the `ace` facade, 18 internal ones and `ace-ffi` (the C ABI) ([`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)) |
| `ace-*0.7.0.sh`, `demo/`, `tools/` | build, benchmark, A/B, release and CI entry points; Python tools |
| `include/` | `ace.h`, the C header (generated, drift-checked in CI) |
| `examples/golden/` | golden SHA-256 (0.6.1, reproduced by 0.7.0; frozen 0.5.0 and 0.4.6) |
| `examples/compat/` | frozen compatibility files of earlier releases |
| `examples/baselines/` | accepted benchmark results per release (`BASELINE.json`) |
| `fuzz/` | `cargo-fuzz` project, 22 targets ([`fuzz/README.md`](fuzz/README.md)) |
| `jvm/`, `demo/jvm/` | Java (Panama) and Scala 3 SDKs over the C ABI, and their demos |
| `docs/` | current documentation; `docs/history/` keeps earlier versions |
| `TASKS-0.7.0.md`, `MILESTONE-0.7.0.json` | implementation order and machine-readable summary of this release (earlier ones in `docs/history/`) |

## Roadmap

0.6.0 — the `ace` API facade, decode and allocation recovery; 0.6.1 — decimal floats (ALP), Patched FOR,
RunDelta for small deltas, DENSE never larger than BALANCED, Format 1.5;
0.7.0 — C ABI, Java (Panama) and Scala SDK (this release); 0.8 — adaptive segmentation and columnar blocks
(Format 2.0); 0.9 — integration with AdaptiveDB and GraphNet; 1.0 — stable API and format.
See [`ROADMAP.md`](ROADMAP.md) and [`CHANGELOG.md`](CHANGELOG.md).

## License

Apache-2.0 (see `LICENSE`).
