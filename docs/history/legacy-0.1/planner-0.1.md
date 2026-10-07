# ACE 0.1 planner

The analyzer measures H0 entropy, zero ratio, useful repeated-byte run coverage, byte-delta entropy improvement,
sampled 4-byte fingerprint repetition and byte alphabet cardinality. A conservative incompressibility score may prune
all non-RAW candidates.

The planner generates a small candidate set from those signals. The evaluator trial-compresses at most the configured
sample size and scores normalized encoded-size and CPU-cost proxies according to FAST, BALANCED or DENSE policy.
A final size guard can still replace the selected candidate by RAW when full-block output does not meet the configured
minimum byte gain.
