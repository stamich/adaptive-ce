# Golden files 0.4.6

`GOLDEN.json` (schema `ace-golden-1`): for every `ace-corpus` workload (1 MiB) and every
profile (fast / balanced / dense) the SHA-256 of the generated input, of the ACE container and
of the decoded bytes. Only hashes are stored; the data is regenerated deterministically.

```bash
cargo test -p ace-engine --test golden                    # verify (auto SIMD backend)
ACE_SIMD=scalar cargo test -p ace-engine --test golden    # verify the portable paths
ACE_GOLDEN_UPDATE=1 cargo test -p ace-engine --test golden  # regenerate (semantic change only)
```

A mismatch of `ace_sha256` is a semantic change and is not allowed within 0.4.x
(`docs/history/ARCHITECTURE-FREEZE-0.4.6.md`).

**Since 0.5.0 this file is frozen**: it is never regenerated. `corpus_v3_matches_0_4_6_golden`
checks that 0.5.0 still produces exactly these bytes for Corpus V3 (the Float lane never
touches the 0.4.x corpus). `ACE_GOLDEN_UPDATE=1` rewrites only `examples/golden/0.5.0/`.
