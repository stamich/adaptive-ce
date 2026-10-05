# ACE 0.3-buildfix9 build audit

## Generation-environment checks

The following static checks were executed successfully:

- all 21 Cargo manifests parsed as TOML;
- every local Cargo `path` dependency resolves;
- all 42 JSON files parse;
- all 6 Python helper scripts pass `py_compile`;
- all shell scripts pass `bash -n`;
- both Java integration examples compile with `javac`;
- Rust delimiter scan passes while ignoring strings/comments;
- new and modified public Rust APIs were audited for adjacent Rustdoc;
- benchmark crate retains an explicit `ace-cost` dependency.

## Rust toolchain

`cargo` and `rustc` are not installed in the generation environment, therefore the authoritative Rust build/test must be run on the target development machine:

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix9.sh
./benchmark.sh all
```

## Expected benchmark focus

Inspect especially:

- FAST throughput and FAST analysis timing;
- BALANCED/DENSE ratio preservation;
- mean and p95 regret;
- Hybrid-LZ stage1/stage2/skip/sample-fraction telemetry;
- warm 64 KiB latency;
- parallel speedup and efficiency.
