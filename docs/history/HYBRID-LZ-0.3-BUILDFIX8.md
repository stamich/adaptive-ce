# Hybrid LZ Estimator — ACE 0.3-buildfix8

The Hybrid LZ Estimator combines two independent signals:

1. the cheap full-block analytical estimate from buildfix6;
2. a bounded deterministic micro-trial using the actual production transform/LZ/entropy pipeline.

Micro-trials are run only for LZ candidates that already survived Top-K. Disjoint windows are encoded
independently and their payload is projected to the full block. Entropy metadata is added once.

A reset window can underestimate long-distance LZ opportunities. Therefore a pessimistic micro-trial
receives less weight than the analytical estimate; an optimistic real-codec result receives more weight.
The exact weights are deterministic and profile-specific.

No full candidate trial encode is added to the planner hot path in buildfix8.
