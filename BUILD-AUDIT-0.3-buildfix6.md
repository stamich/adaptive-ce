# ACE 0.3-buildfix6 build audit

## Static checks performed in the generation environment

- parsed every `Cargo.toml` as TOML;
- verified every local Cargo `path` dependency exists;
- parsed every JSON artifact/configuration;
- compiled Python helper scripts with `py_compile`;
- validated shell scripts with `bash -n`;
- compiled both Java demo source layouts with `javac`;
- audited public Rust structs/enums/traits/functions/types/constants for adjacent Rustdoc;
- checked Rust delimiter balance with a scanner that ignores strings and comments;
- verified `cargo`/`rustc` are not installed in the generation environment.

## Required validation on a Rust-enabled machine

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix6.sh
./benchmark.sh all
```

The release benchmark is authoritative for Planner V3.3 tuning. In particular inspect:

- `oracle_top2_rate_after_sampling`;
- `oracle_top3_rate_after_sampling`;
- `quality_pool_recall`;
- `normalized_regret_bytes_per_block`;
- `predicted_size_regret_bytes_per_block`;
- BALANCED/DENSE compression ratio;
- BALANCED/DENSE throughput.
