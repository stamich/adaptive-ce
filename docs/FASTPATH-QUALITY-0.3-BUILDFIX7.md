# ACE 0.3-buildfix7 — Fast-path quality guard

Buildfix6 showed that an all-zero block could be encoded as roughly 520 bytes by RAW+rANS while the unconditional RLE/None fast path produced roughly 4096 bytes.

Buildfix7 changes policy:
- FAST: retains the RLE/None zero-heavy shortcut;
- BALANCED: zero-heavy blocks use the general planner;
- DENSE: zero-heavy blocks use the general planner;
- incompressible RAW fast path remains available to all profiles;
- FAST-only strong-LZ and strong-delta shortcuts remain unchanged.

This preserves the semantic distinction between a speed-first FAST profile and quality-aware BALANCED/DENSE profiles.
