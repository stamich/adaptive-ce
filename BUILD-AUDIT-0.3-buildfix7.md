# ACE 0.3-buildfix7 build audit

Static release checks performed in the generation environment:
- TOML parse for all Cargo manifests;
- local Cargo path-dependency existence;
- JSON parse for milestone and bundled baseline artifacts;
- Python `py_compile`;
- shell `bash -n`;
- Java demo `javac` where present;
- public Rustdoc audit for newly added public items;
- source delimiter scan that ignores strings/comments;
- ZIP CRC integrity and SHA-256.

`cargo`/`rustc` are not installed in the generation environment, therefore the authoritative machine validation remains:
```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix7.sh
./benchmark.sh all
```

Generation-environment result: TOML/JSON/Python/shell/local-path audits passed. A Rust compiler is unavailable in this environment, so no claim of successful `cargo build` is made.
