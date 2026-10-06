# ACE 0.4 numeric/time-series compression

## Goal

Corpus V2 in ACE 0.3.1 showed the largest remaining quality/performance gap on monotonic and
small-delta integer sequences. ACE 0.4 adds a schema-free integer lane without requiring callers to
supply a table schema.

## Detection

`ace-analysis::analyze_numeric` evaluates little-endian u32 and u64 interpretations over a bounded
sample. It records monotonicity, delta/DoD small-value ratios, p95 bit widths, exception ratio,
value count and tail bytes. Detection is advisory; malformed or unstructured bytes still retain the
complete generic planner fallback.

## Physical codec

`CodecId::Numeric` is available only in Format 1.3. Its payload begins with a fixed `NUM1` header
and stores width, mode, bit width, value count, tail length, first/base value, first delta and packed
payload length.

Three modes are implemented for both u32 and u64:

- Frame-of-Reference + BitPack
- Delta + ZigZag + BitPack
- Delta-of-Delta + ZigZag + BitPack

The encoder computes exact deterministic size estimates for all modes and chooses the smallest.
Trailing bytes that do not form a complete integer are copied verbatim and restored exactly.

## Why scalar first

0.4 intentionally uses scalar bit packing. Correct format semantics, planner selection and ratio are
validated before AVX2/NEON pack/unpack work. SIMD optimization is a candidate for 0.4.x.
