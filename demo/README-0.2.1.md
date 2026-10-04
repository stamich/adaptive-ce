# ACE 0.2.1 demo

The demo focuses on the hardening work introduced after the 0.2 benchmark analysis rather than on a new wire format.

It demonstrates:

- confidence-aware candidate generation;
- candidate tiers in `ace explain`;
- deterministic Cost Model V2.1 metadata accounting;
- unchanged Format 1.1 writing and 1.0/1.1 reading;
- parallel deterministic compression;
- indexed range reads with physical-byte/overread diagnostics;
- planner and random-access JSON benchmark generation.

Run:

```bash
./demo/run-demo-0.2.1.sh
```
