# ACE 0.3-buildfix8 build audit

## Baseline provenance

The buildfix8 source tree was reconstructed from the complete `ace-milestone0.3-buildfix6.zip`
archive. It was not copied from buildfix7.

Static rejection checks confirm production crates contain none of the buildfix7 LZ Estimator V2
symbols (`lz_primary_ratio_v2`, `lz_entropy_factor_v2`, `sampled_match_p95`,
`sampled_match_coverage`, `long_match_ratio`).

## Static validation performed

- all Cargo manifests parse as TOML;
- every local Cargo `path` dependency exists;
- rust-benchmark directly declares `ace-cost`;
- all JSON files parse;
- Python helper scripts pass `py_compile`;
- shell scripts pass `bash -n`;
- Java examples pass a `javac` smoke compile when `javac` is available;
- public Rust structs/enums/traits/functions/types/constants have adjacent Rustdoc;
- Rust delimiter scan passes while ignoring comments and string literals;
- ZIP integrity and SHA-256 are generated at release packaging time.

## Toolchain limitation

`cargo` and `rustc` are not installed in the generation environment, so the authoritative validation
must still be run on a Rust-enabled machine:

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix8.sh
./benchmark.sh all
```

## Most important buildfix8 benchmark signals

- Top-K recall should recover to buildfix6 levels;
- `quality_pool_recall` and regret should improve without LZ V2;
- hybrid sampled bytes/block quantify micro-trial cost;
- hybrid disagreement identifies blocks where reset-window evidence conflicts with analytics;
- MAE/MAPE/bias remain diagnostic only.
