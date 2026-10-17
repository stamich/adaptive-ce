# ACE 0.6.1 — release notes (Numeric & Float II)

0.6.1 is the codec half of the 0.6 plan. It adds Patched FOR to NUM1, RunDelta for small non-zero
deltas, the ALP decimal-float codec in TS1, and makes DENSE never larger than BALANCED. Unlike
0.6.0 it **changes bytes**: blocks that gain a better codec shrink, every other block is
unchanged. It also moves every unit test out of `src/` into `crates/<crate>/tests/`.

Previous release: [`RELEASE-NOTES-0.6.0.md`](RELEASE-NOTES-0.6.0.md).

## Compatibility

| | 0.6.1 vs 0.6.0 |
|---|---|
| Written bytes | smaller for the workloads in the table below; identical for all others (golden 0.5.0 and 0.4.6 still reproduced there) |
| Formats read | 1.0 – 1.5 |
| Formats written | 1.3, 1.4, or **1.5** when a block uses Patched FOR (NUM1 mode 4) or ALP (TS1 mode 4); the writer declares the lowest version the payloads need |
| Old readers | a file that uses neither new mode is byte-compatible with 0.6.0 readers; a 1.5 file is rejected by 0.6.0 with "written by ACE 0.6.1 or newer" |
| Rust API | unchanged; `Stats` counts ALP blocks, `BlockReport` names the new modes |
| CLI | unchanged |
| Benchmark documents | schema 2.2 (unchanged) |

## Results

Golden 0.6.1, 1 MiB per workload, bytes written by 0.6.0 → 0.6.1:

| Workload | Profile | 0.6.0 | 0.6.1 | Feature |
|---|---|---:|---:|---|
| `monotonic-outliers` | all | 590 328 | 2 668 | Patched FOR |
| `gauge-sawtooth` | BALANCED, DENSE | 53 912 | 976 | Patched FOR |
| `int-counter-reset` | BALANCED, DENSE | 394 379 | 49 768 | Patched FOR, small-delta RunDelta |
| `delta-variable` | all | 164 356 | 102 684 | Patched FOR |
| `f64-financial-price` | BALANCED | 206 204 | 189 572 | ALP |
| `zeros`, `runs`, `mixed`, `f64-special` | DENSE | — | −0.1 % to −23 % | DENSE never larger than BALANCED |

Over the 27 workloads of 0.5.0 the total shrinks by 12.0 % (FAST), 15.5 % (BALANCED) and 15.6 %
(DENSE). The four new Corpus V5 workloads (`int-slow-drift`, `int-geometric-delta`,
`f64-decimal-price`, `f32-decimal-sensor`) are guarded by the 0.6.1 file only.

## What changed

* **Patched FOR (NUM1 mode 4)**: frame of reference with a narrow slot width plus an exception
  list for the few values that do not fit. Exact estimator, allocation-free; the planner chooses
  it only when strictly smaller. Estimated only for widths whose base size is within 25 % of the
  best base size and only when 16-value chunks of a source differ by at least 8 bits.
* **RunDelta for small deltas**: the run prefilter also admits lanes that repeat at least half the
  time and move by at most 255 when they change (counters with resets, slow drift, geometric
  steps). No format change; the exact RunDelta size still competes under the dominance rule.
* **ALP (TS1 mode 4)**: decimals as integers (`d = round(v·10^e·10^-f)`), frame of reference,
  exceptions kept verbatim. Chosen by the Float lane and FloatFast only for blocks that are not
  already FloatFast-dominant and whose 64-value sample is at least half decimal-shaped.
  [`TS1-CODEC.md`](TS1-CODEC.md), [`FORMAT-1.5.md`](FORMAT-1.5.md).
* **Chimp / Chimp128: not added.** Measured in the benchmark crate against the engine as the
  baseline, they would shrink 0 of 144 blocks (rule: at least 5 % more blocks). Decision and
  data: [`BENCHMARKS.md`](BENCHMARKS.md).
* **DENSE never larger than BALANCED**: DENSE also plans each block as BALANCED and stores the
  smaller framed result (ties keep DENSE). Cost: DENSE encode runs at about 0.55× of 0.6.0
  speed; decode is unaffected.
* **Format 1.5** with payload-aware minimal versions ([`FORMAT-1.5.md`](FORMAT-1.5.md)).
* **Tests**: no `#[cfg(test)]` module remains in any `src/` file (enforced by the code audit);
  tests use public items only and live in `crates/<crate>/tests/`. `ace-bench` has a library
  target. New: golden 0.6.1, compat fixtures `examples/compat/0.6.1/`, 18 fuzz targets, the
  `format-1-5` and `chimp-evaluation` benchmark families and their Regression V3 gates.

## Performance trade-offs

First run on the 32-thread reference machine (A/B against 0.6.0, interleaved, 3 batches):
DENSE encode 0.55× (DENSE decode 1.05×), `delta-variable` encode 0.66× (bytes −37 %),
`monotonic-outliers` encode 0.80× (bytes −99.5 %), `int-counter-reset` encode 1.52×, decimal-price
encode 1.40× and decode 3.49×. The first run also showed FAST / BALANCED encode of the mixed
workload at 0.92× / 0.94× and `f64_noisy` encode at 0.79× with identical output; both were
traced to the size counting of the new candidates and reduced (early-exit RunDelta count, cheaper
ALP sample test) in the commit that follows it, so the next A/B run on that machine decides.
Second reference run: 16 of 18 cases pass; FAST encode 0.937× and BALANCED encode 0.951× of 0.6.0 for identical output
(extra planning work on the Mixed workload, part of it the exact RunDelta size count of one block), `f64_noisy` encode
0.98×. These were fixed in the code, not in the floors: the Patched FOR plan is reused by the encoder, a sampled pre-test skips the exact pass on blocks without outliers, the ALP count has a counting shortcut and cheaper rounding. The floors and their reasons are in [`BENCHMARKS.md`](BENCHMARKS.md).
Third reference run (32 logical CPUs, 3 batches): 18 of 18 A/B cases pass at their restored floors. FAST encode 0.973×, BALANCED encode 0.981× (identical output), `f64_noisy` 0.997×, `delta-variable` 0.799× (bytes −37 %), `monotonic-outliers` 0.956×, DENSE encode 0.568×, DENSE decode 1.047×, `int-counter-reset` 1.75×, decimal-price encode 1.72× and decode 3.46×; the other cases are 0.97× – 1.00×.

## Not covered here

Fuzz campaign on the reference machine (18 targets × 10 min, including `alp_decode`, `alp_roundtrip` and `numeric_patched_decode`): no crash, no sanitizer finding. The `ts1_roundtrip` and `ts1_decode` logs both show 5487 s of wall time for a 600 s budget and libFuzzer reported one "slow unit" of 4893 s for `ts1_roundtrip`; both targets stretched by the same amount, which points to the machine being suspended during the run, not to an input. The unit is not part of the archive, so this is not proven; a re-run of the two targets is the check.
