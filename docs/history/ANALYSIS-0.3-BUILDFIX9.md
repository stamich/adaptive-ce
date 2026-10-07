# FAST Analyzer Lite — ACE 0.3-buildfix9

`AnalysisLevel` separates analysis work by public compression profile. FAST omits sampled first-order entropy (`entropy_h1`) because no planner decision consumes it. Every feature used by candidate generation, early RAW, CandidateEstimator and CostModel remains bit-identical to Standard analysis. BALANCED and DENSE keep the full analyzer.

This is a CPU optimization only; it does not alter file format, decoding or candidate semantics.
