# ACE 0.4-buildfix2 architecture

## Goal

Buildfix2 does not add a new compression algorithm. It reduces planner work around the already
successful Format 1.3 Numeric codec.

## Route-first architecture

```text
block
 |
 v
NumericPrefilter
 |
 +-----------------------+-----------------------+
 |                       |                       |
 v                       v                       v
Generic             NumericGeneral          NumericFast
 |                       |                       |
generic analyzer         generic analyzer        full-block
generic candidates       reduced/lazy Numeric    structure validation
V3.6 estimator           exact Numeric estimate  |
Top-K/Hybrid LZ          bounded generic search  v
 |                       |                   direct Numeric plan
 +-----------+-----------+                       |
             v                                   |
        QualityEnvelope                          |
             |                                   |
             +----------------+------------------+
                              v
                    one production encode
```

## Strong NumericFast invariant

NumericFast is admitted only for:
- high-confidence sampled numeric structure;
- monotonic sequence;
- non-zero deltas;
- almost entirely small deltas;
- almost constant first-order delta;
- complete-block validation of the same fixed-step structure.

This intentionally excludes zero/constant runs.

## Generic compatibility

The generic route retains:
- Planner V3.6 analytical estimator;
- Hybrid LZ;
- QualityEnvelope;
- deterministic tie-breaking;
- zero full-block candidate trials.

The only change is that a speculative Numeric candidate is removed when the cheap prefilter rejects
numeric structure.

## Numeric analyzer

Full analysis uses:
- one bounded value vector for the chosen interpretation sample;
- stack-resident delta bit-width histogram;
- stack-resident DoD bit-width histogram;
- streaming monotonic/zero/small-delta counters.

The previous temporary delta/DoD/width vectors and percentile sorting are removed.

## Decoder

`bit_width == 0` is handled directly for u32/u64 FOR, Delta and Delta-of-Delta. General widths keep
the existing scalar BitPack decoder and therefore retain wire compatibility.
