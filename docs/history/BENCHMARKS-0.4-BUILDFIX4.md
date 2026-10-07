# ACE 0.4-buildfix4 benchmark contract

Schema remains 2.0.

## Filename contract

Every benchmark JSON is named:

```text
benchmark-<version>-<family>.json
```

Current results:

```text
benchmark-0.4-buildfix4-<family>.json
```

Historical baseline JSON files are also prefixed with `benchmark-`.

## New families

### policy-oracle-v2
Reports global, route and policy oracles plus all three regrets, preference and dominance reason.

### planner-hotpath
Reports route classification, generic analysis, planning, encoding and wall-clock timing.

### random-access-plan-diff
Reports current random-access plan distribution and warm 4K/16K/64K latency for comparison with
the bundled buildfix2 baseline.

## Hard gates

Policy metrics are hard gates. Global/route regret are diagnostics. Performance remains relative to
buildfix2 to prevent metric changes from masking runtime regressions.
