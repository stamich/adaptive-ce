# ACE 0.4 architecture — current hardening release

The base ACE 0.4 design remains valid. The current implementation is **0.4-buildfix4 / Planner V4.3**.
See `ARCHITECTURE-0.4-BUILDFIX2.md` for the route-first performance-hardening changes.

---

# ACE 0.4 architecture

ACE 0.4 preserves the hardened 0.3.1 generic lane and adds an independent numeric lane.

```text
source bytes
   |
   +--> BlockAnalyzer (generic 0.3.1 features)
   |
   +--> NumericAnalyzer
   |      +-- u32 interpretation
   |      +-- u64 interpretation
   |      +-- monotonic/delta/DoD/bit-width statistics
   |
   +--> optional file-level BlockSizeAdvisor
          |
          v
CandidateGenerator V4
   |      +-- RAW/RLE/LZ/entropy candidates (unchanged)
   |      +-- Numeric candidate (Format 1.3)
   v
Planner V4
   |      +-- hardened analytical ranking
   |      +-- adaptive Top-K
   |      +-- Hybrid LZ micro-trials
   |      +-- Numeric deterministic sample verification
   |      +-- QualityEnvelope
   v
PhysicalCompressionPlan
   |
   +-- generic codecs -> entropy
   |
   +-- Numeric codec
          +-- FOR + BitPack
          +-- Delta + ZigZag + BitPack
          +-- Delta-of-Delta + ZigZag + BitPack
   v
Format 1.3 block + unchanged AIDX/ACET
```

The numeric payload is self-describing. This deliberately avoids adding transform-specific metadata
channels to the outer block header and keeps all existing 1.0-1.2 decode behavior isolated.
