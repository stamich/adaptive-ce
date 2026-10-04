# ACE roadmap

## 0.1 — adaptive vertical slice
RAW/RLE/LZ, Delta, Huffman, analyzer, deterministic planner, independent blocks and corruption checks.

## 0.2 / 0.2.1 — random access and planner hardening
Scalar rANS, Format 1.2 block index, parallel blocks, dictionary abstractions, benchmark JSON contract, candidate recall/regret instrumentation and profile calibration.

## 0.3 — performance architecture
Planner V3 analytical estimation, deterministic Top-K sample verification, planner fast paths, rANS4x, safe SIMD dispatch, reusable worker scratch, Format 1.2 and bounded streaming.

## 0.4 — adaptive boundaries and training
Content-defined chunking, dictionary training, online planner statistics and longer-range fingerprints while preserving bounded dependencies.

## 0.5 — ecosystem integration
Stable C ABI / Panama FFM, Java/Scala bindings and AdaptiveDB compression manager integration.

## 0.6 — semantic/GraphNet extensions
Graph/temporal transforms, negotiated compression policies and optional GraphNet physical encoding.

## 0.7+
Self-tuning costs, adaptive dictionaries, hot/warm/cold policies and replica-aware compression.

## 0.3-buildfix6 — Planner V3.3 quality envelope

- separate analytical/sample/blended size estimates;
- quality-first candidate qualification before final scalar cost;
- profile-specific quality slack;
- oracle rank and quality-pool telemetry;
- no wire-format changes;
- zero full candidate trial encodes retained.

## 0.3-buildfix8 — selective rollback / hybrid LZ

- restore buildfix6 analyzer and analytical estimator;
- keep buildfix7 calibration telemetry and zero-heavy guard;
- replace formula-heavy LZ V2 with bounded real-codec micro-trials after Top-K;
- evaluate quality outcomes before considering any optional selective full-trial fallback.
