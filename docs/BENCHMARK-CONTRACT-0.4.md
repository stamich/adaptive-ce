# ACE 0.4 benchmark contract — schema 2.0

Official files are named:

```text
examples/results/benchmark-0.4-buildfix4-<family>.json
```

Required top-level fields remain `schema_version`, `project`, `milestone`, `base`, `scope`,
`benchmark_contract_origin`, `environment`, `configuration`, and `workloads`.

Schema 2.0 is introduced because NumericProfile and block-policy telemetry are first-class structured
concepts rather than only optional scalar additions.

The release checker combines:
- legacy 0.3 quality/no-regression gates;
- numeric capability gates;
- deterministic output gate;
- throughput/latency tolerance against the hardened 0.3 golden baseline.

See `BENCHMARKS-0.4.md` for family definitions and exact gates.


Buildfix4 additionally requires every benchmark-result/baseline JSON filename to contain the word `benchmark` and a version identifier.
