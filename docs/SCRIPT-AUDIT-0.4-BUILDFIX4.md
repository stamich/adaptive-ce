# ACE 0.4-buildfix4 script/dependency audit

This audit was added after the buildfix4 naming migration exposed duplicated `ace-` prefixes in
script-to-script references.

## Canonical script inventory

### Root shell scripts

- `ace-build0.4-buildfix4.sh`
- `ace-benchmark0.4-buildfix4.sh`
- `ace-benchmark-compare0.4-buildfix4.sh`

### Demo

- `demo/ace-run-demo0.4-buildfix4.sh`

### Python tools

- `tools/ace-benchmark_compare0.4-buildfix4.py`
- `tools/ace-benchmark_report0.4-buildfix4.py`
- `tools/ace-calibrate_cost_model0.4-buildfix4.py`
- `tools/ace-check_regressions0.4-buildfix4.py`
- `tools/ace-generate_corpus0.4-buildfix4.py`
- `tools/ace-generate_hardening_corpus0.4-buildfix4.py`
- `tools/ace-generate_numeric_corpus0.4-buildfix4.py`
- `tools/ace-validate_benchmark_json0.4-buildfix4.py`

Every script:
- starts with `ace-`;
- includes `0.4-buildfix4`;
- has no duplicated `ace-` prefix.

## Dependency graph

```text
ace-build0.4-buildfix4.sh
  -> demo/ace-run-demo0.4-buildfix4.sh
       -> tools/ace-generate_numeric_corpus0.4-buildfix4.py
       -> ace-benchmark0.4-buildfix4.sh
  -> ace-benchmark0.4-buildfix4.sh
       -> tools/ace-check_regressions0.4-buildfix4.py
       -> tools/ace-validate_benchmark_json0.4-buildfix4.py
       -> tools/ace-benchmark_report0.4-buildfix4.py

ace-benchmark-compare0.4-buildfix4.sh
  -> tools/ace-benchmark_compare0.4-buildfix4.py
```

## Benchmark result naming

Rust benchmark output:

```text
examples/results/benchmark-0.4-buildfix4-<family>.json
```

Regression output:

```text
examples/results/benchmark-0.4-buildfix4-regression.json
```

The validator now enforces `benchmark-<milestone>-<family>.json`, matching the actual writer and
shell runner.

## Demo families verified against Rust benchmark

- `policy-oracle-v2`
- `numeric-general`
- `planner-hotpath`
- `random-access-plan-diff`
- `numeric-fastpath`
- `planner-route`
- `planner`
- `numeric`
- `numeric-ablation`
- `all`

## Audit checks

- all shell files pass `bash -n`;
- all Python files parse with Python AST;
- every direct script dependency exists;
- every current-version script reference resolves;
- no active script/document contains a duplicated `ace-` prefix;
- every benchmark baseline/result JSON under `examples/baselines` and `examples/results` contains
  `benchmark` in its filename;
- all baseline JSON files referenced by the regression checker exist.
