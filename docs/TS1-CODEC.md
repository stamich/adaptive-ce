# TS1 — time-series codec (ACE 0.5.0)

`CodecId::TimeSeries` (Format 1.4) stores one block as a TS1 payload. Three modes share one
header; the payload is a complete bitstream (no entropy stage, no transform).

| Mode | Id | Lanes | For |
|---|---|---|---|
| `GorillaF64` | 1 | 8 bytes | IEEE-754 binary64 series |
| `GorillaF32` | 2 | 4 bytes | IEEE-754 binary32 series |
| `RunDelta` | 3 | 2 / 4 / 8 bytes | values that rarely change (floats compared bit-wise, integers) |

Ids 4–15 are reserved (Chimp, ALP, …). Lossless means **bit-exact**: `-0.0`, NaN payloads,
±Inf and subnormals round-trip (`to_bits` equality is what the tests compare).

## Payload layout (little-endian)

| Offset | Size | Field | Rule |
|---:|---:|---|---|
| 0 | 4 | magic `TS1\0` | |
| 4 | 1 | version = 1 | |
| 5 | 1 | mode | 1–3 |
| 6 | 1 | lane_bytes | 8 (GorillaF64), 4 (GorillaF32), 2 / 4 / 8 (RunDelta) |
| 7 | 1 | flags | bit 0 `window_reuse` (Gorilla only); other bits must be 0 |
| 8 | 4 | value_count | complete lane values |
| 12 | 1 | tail_len | `original_len % lane_bytes`, < lane_bytes |
| 13 | 3 | reserved | must be 0 |
| 16 | 8 | first_bits | raw bits of the first value (must fit the lane) |
| 24 | 4 | stream_bits | bit length of the stream |
| 28 | n | stream | `ceil(stream_bits / 8)` bytes, LSB-first |
| 28 + n | tail_len | tail | raw trailing bytes |

The reader validates every field and `value_count · lane_bytes + tail_len == block size`,
`28 + n + tail_len == payload size` with checked arithmetic **before** allocating the output.
`ace inspect --blocks` prints mode, lane, values, tail and bits per value from the header
alone (`ts1_inspect`).

## Bitstream

`ace_bitpack::{BitWriter, BitReader}` (public since 0.5.0): LSB-first, 64-bit accumulator,
`write_bits(value, width ≤ 64)`, `read_bits(width) -> Option<u64>` (`None` past the end).
NUM1 bit packing uses the same module (its bytes are unchanged). Huffman keeps its own
MSB-first reader (canonical codes need MSB-first order).

## Gorilla XOR

For `v[0] … v[n-1]` (raw bits, width `W` = 64 or 32), `v[0]` lives in the header:

```text
x = v[i] XOR v[i-1]
x == 0                                 -> '0'
x != 0, reuse on, fits previous window -> '1' '0' + Mw bits of (x >> (W - Lw - Mw))
x != 0, otherwise                      -> '1' '1' + L (Lbits) + (M - 1) (Mbits) + M bits of (x >> T)
                                          L = min(leading_zeros(x), Lmax), T = trailing_zeros(x),
                                          M = W - L - T; window := (L, M)
```

"Fits" means `L ≥ Lw` and `T ≥ W − Lw − Mw`. Without `window_reuse` the selector bit after
`'1'` is omitted.

| Parameter | f64 | f32 |
|---|---:|---:|
| `Lmax` / `Lbits` | 31 / 5 | 15 / 4 |
| `Mbits` (stores `M − 1`, so `M ∈ 1…W`) | 6 | 5 |

Storing `M − 1` instead of Gorilla's "64 encoded as 0" keeps every field directly meaningful;
the size is the same. The decoder rejects `L + M > W`, a stream that ends before
`value_count` values and unused trailing bits.

## RunDelta

```text
first value in the header
for every change:  gamma(run) gamma(zigzag(delta))   run = unchanged values before the change
terminator:        gamma(run) gamma(0)                run = unchanged values at the end
```

* `gamma(v)` is Elias-gamma of `v + 1` computed in 128 bits (`v = u64::MAX` is representable):
  `N` zeros, a one, the low `N` bits — `2N + 1` bits, one bit for `0`.
* `delta = v[i] − v[i−1]` modulo `2^lane_bits`, sign-extended, then ZigZag. A change has
  `zigzag(delta) ≥ 1`, so `0` is an unambiguous terminator.
* There is no global bit width: a rare large jump costs only its own bits (unlike NUM1 Delta).
* The decoder rejects runs or changes beyond `value_count`, deltas wider than the lane, a
  missing terminator and trailing bits.

## Size and speed

`ts1_encoded_len(input, layout)` runs the same encoder against a bit counter (`BitSink`), so
the exact size can never disagree with `ts1_encode_with`; `ts1_stream_bits` exposes the stream
length used by the planner's Gorilla sample estimator. Indicative codec speed on the
development VM (single thread): RunDelta on constant data ≈ 10 GB/s encode / 2 GB/s decode,
Gorilla f64 1.3–2.5 GB/s / 0.75–1 GB/s ([`FLOAT-CALIBRATION-0.5.0.md`](FLOAT-CALIBRATION-0.5.0.md)).

## API (`ace-codecs`)

| Item | Purpose |
|---|---|
| `TimeSeriesMode`, `TimeSeriesLayout` (`GORILLA_F64`, `GORILLA_F32`, `run_delta(n)`, `ALL`, `label`) | mode + lane width |
| `ts1_encode_with(input, layout)` | encode with one layout |
| `ts1_encoded_len`, `ts1_stream_bits` | exact size without writing |
| `ts1_encode_best(input, layouts)`, `ts1_encode(input)` | smallest of several / all layouts |
| `ts1_decode(payload, expected_size)` | validated decode |
| `ts1_inspect(payload)` → `TimeSeriesPayloadInfo` | header only |

Tests: `crates/ace-codecs/src/time_series/tests.rs` (header corruption table, special values,
tails, window flag, every RunDelta lane, gamma extremes), `crates/ace-codecs/tests/ts1_properties.rs`
(arbitrary f64 / f32 bits and sparse integers, exact-size property); fuzz targets `ts1_decode`,
`ts1_roundtrip`, `bitstream_roundtrip`.
