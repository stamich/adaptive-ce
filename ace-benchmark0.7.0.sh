#!/usr/bin/env bash
# ACE 0.7.0 benchmarks: Harness V3 families + validation + report (+ Regression V3 for `all`).
#
#   ./ace-benchmark0.7.0.sh [<family>|all] [--quick] [--isolated] [--ab FILE] [--repeat N]
#
#   --quick     ACE_BENCH_QUICK=1 smoke plan (1 batch; not release-grade, stability skipped)
#   --isolated  pin to physical cores with taskset (if present) and print tuning hints
#   --ab FILE   interleaved A/B document passed to Regression V3 (from ace-ab0.7.0.sh)
#   --repeat N  run the selection N times into $ACE_RESULTS/run-<i>/, check repeatability
#               (tools/ace-repeatability0.7.0.py) and write the median documents into
#               $ACE_RESULTS — the release baseline is `all --repeat 3` on the reference machine
#
# Exit code of `all` is the Regression V3 code: 0 pass, 1 fail, 3 unstable, 4 incomplete or
# diagnostic (a run on a CPU governor other than `performance` is never a pass). With --repeat a
# failed repeatability check (exit 1) wins over the regression code.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.7.0.sh"
cd "$ACE_ROOT"

family="all"; isolated=0; ab_args=(); repeat=1
while (( $# )); do
  case "$1" in
    --quick) export ACE_BENCH_QUICK=1 ;;
    --isolated) isolated=1 ;;
    --ab) ab_args=(--ab "$2"); shift ;;
    --repeat) repeat="$2"; shift ;;
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
repeat_status=0
if (( repeat > 1 )); then
  runs=()
  for (( run = 1; run <= repeat; run++ )); do
    ace_step "run $run/$repeat"
    rm -rf "$ACE_RESULTS/run-$run" && mkdir -p "$ACE_RESULTS/run-$run"
    ACE_BENCH_OUT_DIR="$ACE_RESULTS/run-$run" "${runner[@]}" "$(ace_bin ace-bench)" "$family"
    runs+=("$ACE_RESULTS/run-$run")
  done
  set +e
  python3 tools/ace-repeatability0.7.0.py "${runs[@]}" --median-out "$ACE_RESULTS"
  repeat_status=$?
  set -e
else
  "${runner[@]}" "$(ace_bin ace-bench)" "$family"
fi

if [[ "$family" == "all" ]]; then
  python3 tools/ace-validate_benchmark_json0.7.0.py "$ACE_RESULTS"/benchmark-${ACE_VERSION}-*.json
  set +e
  python3 tools/ace-check_regressions0.7.0.py --results "$ACE_RESULTS" \
    --baselines examples/baselines "${ab_args[@]}"
  status=$?
  set -e
  python3 tools/ace-benchmark_report0.7.0.py "$ACE_RESULTS/benchmark-${ACE_VERSION}-release-performance.json"
  (( repeat_status == 1 )) && exit 1
  exit "$status"
fi
result="$ACE_RESULTS/benchmark-${ACE_VERSION}-${family}.json"
python3 tools/ace-validate_benchmark_json0.7.0.py "$result"
python3 tools/ace-benchmark_report0.7.0.py "$result"
exit "$repeat_status"
