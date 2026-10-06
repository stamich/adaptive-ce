# ACE 0.4 build audit

## Scope

ACE 0.4 is based on the fully gated ACE 0.3.1 hardened release. It introduces schema-free integer specialization, a new self-describing Numeric primary codec, Format 1.3, Planner V4 and file-level automatic block-size advice while preserving the generic 0.3.1 path.

## Functional additions

- `ace-bitpack`: scalar u32/u64 bit packing, ZigZag, Delta, Delta-of-Delta and Frame-of-Reference primitives.
- `ace-analysis`: numeric profile/detection and deterministic block-size advisor.
- `ace-codecs`: self-describing `NUM1` u32/u64 Numeric codec.
- `ace-core`: `CodecId::Numeric`, `NumericWidth`, `NumericEndian`, `BlockSizePolicy`, `AccessHint`, numeric statistics.
- `ace-planner`: numeric candidate lane evaluated through the hardened V3.6 stages as Planner V4.
- `ace-format`: Format 1.3 writer plus 1.0/1.1/1.2/1.3 reader compatibility; AIDX/ACET unchanged.
- `ace-engine`/`ace-cli`: numeric telemetry, explain output and file-level Auto block-size policy.
- benchmark schema 2.0 with `numeric`, `numeric-ablation` and `block-policy` families.
- Corpus V3 numeric generator and new fuzz targets.

## Static validation

- TOML files parsed: 24
- JSON files parsed: 57
- Python files compiled with `py_compile`: 8
- shell scripts checked with `bash -n`: 9
- Rust files scanned: 88
- remaining Rust `json!` macro sites: 0
- new public Rustdoc warnings: 0
- Cargo path-dependency errors: 0
- release-identity/schema errors: 0
- bundled 0.3.1 hardening benchmark observations: 4
- validation errors: 0

No Java or Scala production API was changed in 0.4, so existing Javadoc/Scaladoc remains applicable. New production API is Rust and is documented with Rustdoc.

## Toolchain availability

- cargo available in generation environment: False
- rustc available in generation environment: False
- rustfmt available in generation environment: False

The generation environment does not provide a Rust toolchain. Authoritative validation must therefore be run on a Rust-enabled machine:

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.4.sh
./benchmark.sh all
```

Optional fuzz smoke:

```bash
cargo fuzz run numeric_decode -- -max_total_time=60
cargo fuzz run bitpack_decode -- -max_total_time=60
cargo fuzz run container_decode -- -max_total_time=60
```

## Compatibility

- workspace version: 0.4.0
- benchmark schema: 2.0
- writer format: 1.3
- reader formats: 1.0 / 1.1 / 1.2 / 1.3
- AIDX/ACET layout: unchanged
- default block policy: Fixed 256 KiB (backward behavioral default)
