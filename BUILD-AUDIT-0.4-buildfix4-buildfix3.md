# ACE 0.4-buildfix4-buildfix3 build/script audit

## Scope
Full repair and verification of every script used by build, demo and benchmark workflows.

## Canonical scripts
- `ace-benchmark-compare0.4-buildfix4.sh`
- `ace-benchmark0.4-buildfix4.sh`
- `ace-build0.4-buildfix4.sh`
- `demo/ace-run-demo0.4-buildfix4.sh`
- `tools/ace-benchmark_compare0.4-buildfix4.py`
- `tools/ace-benchmark_report0.4-buildfix4.py`
- `tools/ace-calibrate_cost_model0.4-buildfix4.py`
- `tools/ace-check_regressions0.4-buildfix4.py`
- `tools/ace-generate_corpus0.4-buildfix4.py`
- `tools/ace-generate_hardening_corpus0.4-buildfix4.py`
- `tools/ace-generate_numeric_corpus0.4-buildfix4.py`
- `tools/ace-validate_benchmark_json0.4-buildfix4.py`

## Verification
- every shell/Python script starts with `ace-`
- every shell/Python script contains `0.4-buildfix4`
- no duplicated prefix remains in active scripts/current docs
- all direct script dependencies exist and are referenced by canonical name
- every demo benchmark family is supported by the Rust benchmark binary
- Rust writer and Python validator agree on `benchmark-<milestone>-<family>.json`
- all regression baseline files referenced by the checker exist
- all benchmark baseline/result JSON names contain `benchmark` and a version
- E0689 and E0308 compile fixes remain present

## Static validation
- TOML parsed: 24
- JSON parsed: 93
- Python AST parsed: 8
- shell scripts checked with `bash -n`: 4
- Rust files scanned: 97
- validation errors: 0
- cargo available: False
- rustc available: False

Rust compilation was not executed in this environment.

Authoritative command:

```bash
./ace-build0.4-buildfix4.sh
```
