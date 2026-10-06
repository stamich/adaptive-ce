# ACE Roadmap

## 0.4-buildfix4 — Policy Oracle Closure & Runtime Regression Fix

Buildfix4 is intended to close the ACE 0.4 hardening line.

Closure criteria:
- policy recall/regret gates pass;
- FAST/BALANCED/DENSE remain >=95% buildfix2;
- NumericFast returns >=95% buildfix2 throughput;
- warm64K returns <=110% buildfix2;
- NumericGeneral keeps buildfix3 targets;
- NumericFast false-positive corpus is zero;
- Format 1.3/determinism/fuzzing remain healthy.

If these gates pass, the next release should be functional ACE 0.5 rather than Planner V4.4.

## ACE 0.5 candidates

- Gorilla/XOR float/time-series compression;
- decimal-specific transforms;
- dictionary learning/reuse;
- SIMD BitPack after planner/runtime overhead is closed;
- numeric checkpoints only if production random-access evidence justifies format complexity.

No Format 1.4 work is part of buildfix4.
