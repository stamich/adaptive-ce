# Competitive analysis: Kanzi (flanglet/kanzi-cpp) vs ACE

Status: 2026-10-15, written at ACE 0.5.1. Facts marked **[page]** were read from the Kanzi
README pages on that date; **[author]** marks Kanzi's own benchmark figures (one author, one
machine per table); **[knowledge]** marks details recalled from general knowledge that were not
re-checked. ACE figures come from [`PERFORMANCE-0.5.1.md`](PERFORMANCE-0.5.1.md).

## 1. Summary

Kanzi and ACE both split input into blocks, compress them in parallel and pick transforms and an
entropy coder per block, so they look alike. They aim at different goals:

* **Kanzi** is a *ratio-first general-purpose compressor* with a very wide range (level 1 to
  level 9 = 2.7× to 5.1× on Silesia) built on BWT and context modelling for the top levels.
* **ACE** is a *structure-aware, deterministic, safe-to-decode block format*: its ratio comes from
  recognising integers, timestamps and floats (hundreds to thousands of ×) and its selling points
  are byte-identical output on every machine, an audited decoder with resource limits, and a
  block index for random access.

They overlap on text and mixed binary data, where Kanzi's higher levels are far ahead in ratio
and far behind in speed. They barely overlap on numeric telemetry, where Kanzi has no
numeric-specific codec **[page]** (only multimedia, DNA and UTF transforms are named).

## 2. Side by side

| Aspect | Kanzi | ACE 0.5.1 |
|---|---|---|
| Language / licence | C++ (C++98 – C++20), also Java and Go ports; Apache-2.0 **[page]** | Rust (MSRV 1.97), Apache-2.0 |
| Core idea | modular transforms + entropy codecs chosen at runtime; BWT and CM at high levels **[page]** | deterministic cost model picks the cheapest reversible pipeline per block (Planner V5) |
| Levels / profiles | 9 levels **[page]** | 3 profiles (FAST, BALANCED, DENSE) |
| Typical block | up to 32 MB at level 9 **[page]** | 256 KiB default, configurable (decoder limit 4 MiB) |
| Parallelism | multithreaded blocks, on by default **[page]** | parallel blocks, output identical for 1 – 32 workers (tested) |
| Random access | seekable bitstream: blocks decompress independently **[page]** | block index; 64 KiB warm read ≈ 22 µs, 4 KiB range reads |
| Integer / timestamp / float awareness | none named **[page]** | NUM1 (FOR, delta, delta-of-delta, bit packing), TS1 (Gorilla, RunDelta) |
| Integrity | optional checksums **[page]** | per-block CRC32C, validated before use |
| Determinism guarantee | not addressed **[page]** | contract: same bytes on every machine / thread count / SIMD backend; golden SHA-256 files |
| Hostile-input hardening | not addressed **[page]** | audited arithmetic, `DecodeLimits`, fail-before-allocate tests, 15 fuzz targets |
| Format compatibility | not stated **[page]**; "not compatible with standard formats" **[page]** | frozen files of every release since 0.2.1 decode on every build |
| Bindings | C and C++ API, ports in Java and Go **[page]** | CLI + Rust crates; C ABI and Java / Scala planned for 0.7 |
| Platforms | Windows, Linux, macOS, BSD **[page]** | Linux, x86_64 AVX2 / scalar (aarch64 planned for 0.7) |
| Maturity signals | 2,700+ commits, 222 stars **[page]**; no GitHub releases listed **[page]** | 0.5.1; tagged releases, baselines, release reports |

## 3. Numbers

Kanzi's Silesia table **[author]** (AMD Ryzen 9950X, 211,957,760 bytes; ratios and MB/s computed
from the table):

| Compressor | Ratio | Encode MB/s | Decode MB/s |
|---|---|---|---|
| kanzi -l 1 | 2.67× | 2,944 | 5,047 |
| kanzi -l 6 | 4.28× | 231 | 398 |
| kanzi -l 9 | 5.10× | 18 | 17 |
| zstd -T16 -2 | 3.05× | 35,000 | 19,000 |
| zstd -T16 -19 | 4.01× | 19 | 1,630 |
| xz -9 | 4.34× | 5 | 228 |

ACE on its own mixed 16 MiB corpus (Ryzen 9 5950X, `performance` governor, 32 threads): FAST 3.68× at
334 MB/s encode / 4,921 MB/s decode; BALANCED 3.73× at 222 / 4,703; DENSE 3.72× at 166 / ~4,400.

**These two sets of numbers must not be compared directly:** different corpora, machines, thread
counts and measurement methods (ACE: interleaved A/B, median of medians; Kanzi: one run per row).
What can be said:

* ACE's speed class is Kanzi level 1 to 2 / zstd mid levels; its decode speed is the strength.
* ACE's three profiles span ≈ 3.7× – 3.7×: **DENSE gives no extra ratio over BALANCED on the
  mixed corpus** (3.722 vs 3.726). Kanzi's levels give a real ratio/time trade-off from 2.7× to
  5.1×. ACE has no profile that buys ratio with time on text-like data.
* On numeric series ACE has an advantage no general compressor has (u32 counters ≈ 2,300×, constant
  f64 ≈ 2,400×); Kanzi was not designed for these.

## 4. Where Kanzi is ahead

1. **Ratio headroom on text and mixed data**: BWT + CM reach 4.3 – 5.1× on Silesia; LZ + Huffman
   / rANS (ACE) cannot.
2. **Reach**: three language implementations, C API, Windows / macOS / BSD, a wiki and
   `lzbench`-style comparisons that users can reproduce.
3. **Content-aware transforms** for text, DNA, multimedia and executables **[knowledge]**.

## 5. Where ACE is ahead

1. **Numeric and time-series data** (NUM1, TS1, RunDelta, planner estimators).
2. **Determinism as a contract**, tested to 32 workers and across SIMD backends.
3. **Decoder safety**: limits contract, hardening audit, fuzzing in CI, compatibility fixtures.
4. **Random access and decode speed** as design goals (block index; 4.7 – 4.9 GB/s decode).
5. **Release engineering**: frozen golden files, A/B performance gates, reproducible package.

## 6. Implications for the ACE roadmap

* **Do not chase Kanzi's ratio with BWT / CM.** It would cost decode speed, random access
  granularity and determinism complexity, which are ACE's positioning. If a ratio-first profile is
  wanted later, treat it as an optional post-1.0 backend (own format mode, own gates).
* **Fix the DENSE gap (0.6.0, Phase A).** Either make DENSE buy ratio on text-like data (larger
  window, optimal parsing, two-pass rANS models) or document that it only matters for numeric
  data. Today it costs ≈ 35 % more encode time for ≈ 0 % more ratio on the mixed corpus.
* **Add a public-corpus family** (Silesia and enwik8) to the benchmarks so ACE can be placed on
  the same axes as Kanzi, zstd, xz and lz4 (`lzbench`-compatible table: ratio, encode MB/s,
  decode MB/s, threads). Keep the ACE-specific corpus for numeric lanes.
* **Message**: "structure-aware, deterministic, hardened" rather than "better ratio".
  Publish where ACE is not the best choice (large text archives).
* **Reach**: Kanzi's C API and ports show the demand; the 0.7 C ABI and Java SDK are justified.
  Add Windows / macOS CI before 1.0 if the audience is wider than Linux servers.

## 7. Not covered here

Closer competitors for ACE's numeric lanes should be benchmarked next: Pcodec, Blosc2 (shuffle /
delta filters, chunked random access), BtrBlocks / ALP-style cascades and the ClickHouse codecs
(Delta, DoubleDelta, Gorilla, T64) **[knowledge]**, plus zstd and lz4 as the baseline every user
compares against. A head-to-head run (same machine, same files, same thread count) is needed
before any claim in the README.

Sources: [kanzi-cpp README](https://github.com/flanglet/kanzi-cpp),
[kanzi README](https://github.com/flanglet/kanzi/blob/master/README.md).
