# ACE 0.4-buildfix4-buildfix1

## Root cause

Rust could not infer the integer type of `absolute` in:

```rust
let (mut absolute, mut relative) = match config.profile { ... };
```

The subsequent call:

```rust
absolute.saturating_mul(2)
```

requires a concrete integer type, producing E0689.

## Fix

The binding is now explicit:

```rust
let (mut absolute, mut relative): (u64, f64) = match config.profile {
    CompressionProfile::Fast => (4096_u64, 0.02_f64),
    CompressionProfile::Balanced => (2048_u64, 0.01_f64),
    CompressionProfile::Dense => (512_u64, 0.005_f64),
};
```

This is consistent with `DominanceEnvelope::max_absolute_size_loss: u64` and
`max_relative_size_loss: f64`.

## Script naming rule

All project shell and Python scripts now:
1. start with `ace-`;
2. include `0.4-buildfix4` in the filename.

Examples:

```text
ace-build0.4-buildfix4.sh
ace-benchmark0.4-buildfix4.sh
ace-benchmark-compare0.4-buildfix4.sh
demo/ace-run-demo0.4-buildfix4.sh
tools/ace-check_regressions0.4-buildfix4.py
```
