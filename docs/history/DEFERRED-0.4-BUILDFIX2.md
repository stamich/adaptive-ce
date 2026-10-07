# Deferred from ACE 0.4-buildfix2

Buildfix2 intentionally keeps Format 1.3 / `NUM1` bytes unchanged.

The following ideas remain candidates only if post-buildfix2 measurements show they are still needed:

- Numeric random-access checkpoints / range-local DoD reconstruction;
- specialized scalar unpack loops for bit widths 1/2/4/8;
- AVX2/NEON BitPack;
- decode-cost dominance policy beyond the current zero/constant NumericFast exclusion;
- deeper NumericGeneral optimization for variable-delta timestamps.

Reason: buildfix2 is a planner/runtime hardening release. Introducing a new checkpoint metadata
layout before measuring route-first planning and zero-width decode would mix wire-format work with
the performance regression fix and make attribution harder.
