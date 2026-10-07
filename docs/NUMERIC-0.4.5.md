# Numeric path in ACE 0.4.5

## Pipeline
`PlanningContext::classify` -> `numeric_prefilter` (cheap, bounded sampling) -> route:

| Route | Meaning | Numeric candidate |
|---|---|---|
| `NumericFast` | fixed-step counter validated on the whole block | returned directly (no search) |
| `NumericGeneral` | numeric structure plausible | **always** present since 0.4.5, in FAST/BALANCED/DENSE |
| `Generic` | no numeric structure | only the profile-driven (byte-delta) candidate |

The exact NUM1 size comes from `estimate_numeric` (no trial encode), so Numeric competes with
generic candidates on its real size.

## Lane-relative "small delta"
`small_delta_bits = lane_bits / 2` (u16: 8, u32: 16, u64: 32). Rationale: a delta that fits in half
the lane still saves >= 50 % when bit-packed. The pre-0.4.5 fixed 16-bit limit rejected u64
nanosecond clocks with ~1 ms jitter (deltas ~20 bits).

## Lane-ring arithmetic
Deltas are computed modulo `2^(8*B)` and sign-extended (`lane_delta_const::<B>`), so a u16 counter
that goes 65535 -> 0 has delta +1.

## NUM1 payload (self-describing, 40-byte header)
Header carries: lane width in bytes (**2, 4 or 8**), mode (FOR / Delta / DoD), packed bit width,
value count, tail byte count, first value, first delta, packed length. Followed by the bit-packed
ZigZag stream and verbatim tail bytes. Decoders validate every field before allocating.

### Compatibility note (important)
Width `2` is new in 0.4.5. The container version stays 1.3, so a 0.4.4 reader meeting a u16 block
fails with a malformed-numeric-header error. 0.4.5 reads all 0.4.4 files. If cross-version
exchange matters, a writer option that disables u16 is on the 0.4.1 backlog.

## Telemetry
`ace inspect file.ace --blocks` prints, per numeric block,
`numeric width=U64 mode=DoD bit_width=10 values=32768 tail=0 bits_per_value=10.00`
and a final `numeric blocks=N original=... payload=... ratio=...` line.
