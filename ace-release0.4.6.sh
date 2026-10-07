#!/usr/bin/env bash
# ACE 0.4.6 release pipeline (acceptance matrix of docs/RELEASE-CHECKLIST-0.4.6.md).
#
#   ./ace-release0.4.6.sh <baseline-source-tree> [--quick]
#
#   build/fmt/clippy/doc -> code audit -> release tests (proptest budget, ignored large files)
#   -> golden (auto + scalar) -> process determinism -> benchmarks -> interleaved A/B
#   -> Regression V3 (Unstable => retry <= 2) -> PERFORMANCE doc -> demo
#   -> deterministic package (x2) -> test from the archive
#
# --quick runs the same pipeline with smoke-sized measurements; its verdict is never a release.
# The filled checklist is written to examples/results/release-checklist-0.4.6.md.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.4.6.sh"
cd "$ACE_ROOT"

(( $# >= 1 )) || ace_die "usage: $0 <baseline-source-tree> [--quick]"
baseline_tree="$(cd "$1" && pwd)"; shift
quick=0; [[ "${1:-}" == "--quick" ]] && quick=1
bench_flags=(); ab_flags=()
if (( quick )); then bench_flags=(--quick); ab_flags=(--quick); fi

checklist=()
record() { checklist+=("| $1 | $2 | $3 |"); ace_log "$1: $2"; }

ace_step "1/11 build, fmt, clippy, rustdoc"
./ace-build0.4.6.sh --no-demo
record "Build / fmt / clippy / doc" PASS "ace-build0.4.6.sh"

ace_step "2/11 code audit (unsafe placement, panic policy)"
python3 tools/ace-code_audit0.4.6.py
record "Unsafe / panic audit" PASS "tools/ace-code_audit0.4.6.py"

ace_step "3/11 release tests (proptest budget ${ACE_PROPTEST_CASES:-10000}) + ignored large-file tests"
PROPTEST_CASES="${ACE_PROPTEST_CASES:-10000}" cargo test --release --workspace
cargo test --release --workspace -- --ignored
record "Unit + property + hardening tests" PASS "cargo test --release (incl. --ignored)"
record "Format readers 1.0-1.3 / malformed / limits" PASS "format_1_* / malformed_matrix tests"

ace_step "4/11 golden SHA-256 (auto backend and ACE_SIMD=scalar)"
cargo test --release -p ace-engine --test golden
ACE_SIMD=scalar cargo test --release -p ace-engine --test golden
record "Golden SHA-256 (auto + scalar)" PASS "crates/ace-engine/tests/golden.rs"

ace_step "5/11 determinism across processes and backends"
work="$ACE_ROOT/target/release-$ACE_VERSION"; rm -rf "$work"; mkdir -p "$work"
"$(ace_bin ace-corpus)" mixed 16M "$work/mixed.bin"
for profile in fast balanced dense; do
  hashes=()
  for run in 1 2 3 4 5 6 7 8 9 10; do
    threads=$(( run % 2 ? 1 : 0 ))
    "$(ace_bin ace)" compress --profile "$profile" --threads "$threads" "$work/mixed.bin" "$work/out.ace" >/dev/null
    hashes+=("$(sha256sum "$work/out.ace" | cut -d' ' -f1)")
  done
  ACE_SIMD=scalar "$(ace_bin ace)" compress --profile "$profile" "$work/mixed.bin" "$work/out.ace" >/dev/null
  hashes+=("$(sha256sum "$work/out.ace" | cut -d' ' -f1)")
  [[ "$(printf '%s\n' "${hashes[@]}" | sort -u | wc -l)" == 1 ]] || ace_die "$profile: non-deterministic output"
done
record "Determinism (threads x 10 processes x SIMD backend)" PASS "CLI SHA-256 + determinism_matrix test"

externals=(--external golden_sha256=pass --external determinism_matrix=pass
           --external format_readers=pass --external malformed_matrix=pass)
ab_json="$ACE_RESULTS/ab-${ACE_VERSION}-vs-${ACE_BASELINE}.json"

ace_step "6/11 benchmarks (Harness V3)"
./ace-benchmark0.4.6.sh all "${bench_flags[@]}" || true

ace_step "7/11 interleaved A/B vs $ACE_BASELINE + Regression V3"
status=1
for attempt in 1 2 3; do
  ./ace-ab0.4.6.sh "$baseline_tree" "${ab_flags[@]}" || true
  set +e
  python3 tools/ace-check_regressions0.4.6.py --results "$ACE_RESULTS" --baselines examples/baselines \
    --ab "$ab_json" "${externals[@]}" > "$work/regression.log"
  status=$?
  set -e
  tail -n +1 "$work/regression.log" | head -n 1
  (( status == 3 )) || break
  ace_log "measurement UNSTABLE (attempt $attempt/3): re-running release-performance"
  ./ace-benchmark0.4.6.sh release-performance "${bench_flags[@]}"
done
cat "$work/regression.log"
case "$status" in
  0) record "Quality / performance (A/B) / stability" PASS "Regression V3" ;;
  3) record "Quality / performance (A/B) / stability" UNSTABLE "re-run on a quiet machine" ;;
  4) record "Quality / performance (A/B) / stability" INCOMPLETE "quick plan or missing input" ;;
  *) record "Quality / performance (A/B) / stability" FAIL "see regression log" ;;
esac

ace_step "8/11 performance report"
report="docs/PERFORMANCE-${ACE_VERSION}.md"
(( quick )) && report="$ACE_RESULTS/PERFORMANCE-${ACE_VERSION}-quick.md"   # never overwrite docs
python3 tools/ace-benchmark_report0.4.6.py --markdown "$report" \
  --results "$ACE_RESULTS" --ab "$ab_json" \
  --regression "$ACE_RESULTS/benchmark-${ACE_VERSION}-regression.json"
record "Performance report" PASS "$report"

ace_step "9/11 demo"
./demo/ace-run-demo0.4.6.sh
record "Demo" PASS "demo/ace-run-demo0.4.6.sh"

ace_step "10/11 deterministic package"
python3 tools/ace-package0.4.6.py --verify-reproducible --output "$work/package"
record "Reproducible package + name audit" PASS "tools/ace-package0.4.6.py"

ace_step "11/11 test from the archive"
rm -rf "$work/unpacked" && mkdir -p "$work/unpacked"
python3 -c "import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" \
  "$work/package/ACE-${ACE_VERSION}.zip" "$work/unpacked"
(
  cd "$work/unpacked/ACE-${ACE_VERSION}"
  chmod +x ./*.sh demo/*.sh tools/*.py
  CARGO_TARGET_DIR="$work/unpacked-target" cargo test --release --workspace --quiet
  CARGO_TARGET_DIR="$work/unpacked-target" cargo test --release -p ace-engine --test golden --quiet
)
record "Tested from archive" PASS "unpacked ACE-${ACE_VERSION}.zip"

{
  echo "# ACE ${ACE_VERSION} — release checklist (filled by ace-release0.4.6.sh)"
  echo
  echo "Mode: $([[ $quick == 1 ]] && echo 'QUICK (not a release)' || echo release)"
  echo
  echo "| Area | Status | Source |"
  echo "|---|---|---|"
  printf '%s\n' "${checklist[@]}"
} > "$ACE_RESULTS/release-checklist-${ACE_VERSION}.md"
ace_log "checklist written to $ACE_RESULTS/release-checklist-${ACE_VERSION}.md"
(( status == 0 && quick == 0 )) || { ace_log "release verdict: NOT RELEASABLE (regression status $status, quick=$quick)"; exit 1; }
ace_log "release verdict: PASS"
