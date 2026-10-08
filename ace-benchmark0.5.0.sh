#!/usr/bin/env bash
# ACE 0.5.0 benchmarks: Harness V3 families + validation + report (+ Regression V3 for `all`).
#
#   ./ace-benchmark0.5.0.sh [<family>|all] [--quick] [--isolated] [--ab FILE]
#
#   --quick     ACE_BENCH_QUICK=1 smoke plan (1 batch; not release-grade, stability skipped)
#   --isolated  pin to physical cores with taskset (if present) and print tuning hints
#   --ab FILE   interleaved A/B document passed to Regression V3 (from ace-ab0.5.0.sh)
#
# Exit code of `all` is the Regression V3 code: 0 pass, 1 fail, 3 unstable, 4 incomplete.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.5.0.sh"
cd "$ACE_ROOT"

family="all"; isolated=0; ab_args=()
while (( $# )); do
  case "$1" in
    --quick) export ACE_BENCH_QUICK=1 ;;
    --isolated) isolated=1 ;;
    --ab) ab_args=(--ab "$2"); shift ;;
    -*) ace_die "unknown option $1" ;;
    *) family="$1" ;;
  esac
  shift
done

cargo build --release -p ace-bench --quiet
runner=()
if (( isolated )); then
  if command -v taskset >/dev/null; then
    cores=$(( $(nproc) > 2 ? $(nproc) / 2 : 1 ))
    runner=(taskset -c "0-$((cores - 1))")
    ace_log "pinned to CPUs 0-$((cores - 1))"
  fi
  ace_log "hint: use the 'performance' governor and stop builds/indexers while benchmarking"
fi

mkdir -p "$ACE_RESULTS"
"${runner[@]}" "$(ace_bin ace-bench)" "$family"

if [[ "$family" == "all" ]]; then
  python3 tools/ace-validate_benchmark_json0.5.0.py "$ACE_RESULTS"/benchmark-${ACE_VERSION}-*.json
  set +e
  python3 tools/ace-check_regressions0.5.0.py --results "$ACE_RESULTS" \
    --baselines examples/baselines "${ab_args[@]}"
  status=$?
  set -e
  python3 tools/ace-benchmark_report0.5.0.py "$ACE_RESULTS/benchmark-${ACE_VERSION}-release-performance.json"
  exit "$status"
fi
result="$ACE_RESULTS/benchmark-${ACE_VERSION}-${family}.json"
python3 tools/ace-validate_benchmark_json0.5.0.py "$result"
python3 tools/ace-benchmark_report0.5.0.py "$result"
