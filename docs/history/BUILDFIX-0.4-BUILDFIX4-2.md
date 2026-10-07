# ACE 0.4-buildfix4-buildfix2

## Root cause

`random_access_plan_diff_family` stored requested range sizes as `usize`, but
`AceIndexedDecoder::read_range` accepts `Range<u64>`.

The previous call:

```rust
decoder.read_range(0..len)?
```

therefore produced E0308 (`expected u64, found usize`).

## Fix

The range is now converted explicitly:

```rust
decoder.read_range(0_u64..len as u64)?
```

This is appropriate because all benchmark range lengths are small fixed constants and safely fit in
`u64`.

## Scope

This buildfix changes benchmark compilation only. Planner V4.3, PolicyOracle V2, NumericFast,
NumericGeneral, Format 1.3 and benchmark schema 2.0 are unchanged.
