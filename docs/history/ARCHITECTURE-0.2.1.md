# ACE 0.2.1 architecture

ACE 0.2.1 deliberately preserves the 0.2 data path and Format 1.2 while hardening planner quality and observability.

```text
source block
   -> BlockAnalyzer
   -> BlockProfile
   -> CandidateGenerator V2.1
        -> Mandatory
        -> Likely
        -> Exploratory
   -> Cost Model V2.1
        -> encoded bytes + metadata bytes
        -> static encode/decode work
        -> memory estimate
   -> selected PhysicalCompressionPlan
   -> transform -> codec -> entropy
   -> RAW size guard
   -> deterministic ordered writer
   -> AIDX + ACET
```

The decoder remains planner-free. Random-access APIs use the Format 1.2 index to locate only intersecting blocks. Parallel workers may finish in arbitrary order, but file assembly remains ordered by block id so thread count cannot change serialized bytes.
