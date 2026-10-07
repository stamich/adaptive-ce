# ACE 0.4-buildfix4 architecture

## Planner V4.3 hot path

```text
input
 |
 v
PlanningContext::classify (once)
 |
 +-- Generic ---------> generic BlockProfile -> generic planner
 |
 +-- NumericGeneral --> generic BlockProfile + exact Numeric -> bounded V4.2 lane
 |
 +-- NumericFast -----> NumericFastEvidence
                         |
                         v
                 direct NUM1 fixed-step encode
```

The production engine never performs route classification or strong Numeric validation twice.

## Policy layers

```text
RoutePolicy       -> what is production-eligible
DominancePolicy   -> what product behavior prefers
PolicyOracle      -> offline measured reference for release quality
```

Global and route oracles remain diagnostics. Policy oracle defines release regret/recall.

## Format

Format 1.3 and NUM1 are unchanged. Direct NumericFast encoding emits the existing
DeltaOfDelta/bit_width=0 representation.
