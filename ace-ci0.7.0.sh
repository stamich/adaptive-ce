#!/usr/bin/env bash
# Vendor-neutral CI entry point for ACE 0.7.0.
#
#   ./ace-ci0.7.0.sh pr        build gates + code audit + include/ace.h drift check (needs
#                              cbindgen 0.29.0) + C smoke test (needs a C compiler) + quick release-performance + demo
#   ./ace-ci0.7.0.sh release   full ace-release0.7.0.sh (needs ACE_AB_BASELINE_TREE)
#   ./ace-ci0.7.0.sh fuzz      all 20 fuzz targets, ACE_FUZZ_SECONDS each (default 60);
#                              ACE_FUZZ_CMIN=1 minimises every corpus afterwards
#   ./ace-ci0.7.0.sh fuzz-smoke  all 20 fuzz targets, ACE_FUZZ_SMOKE_SECONDS each (default 25)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.7.0.sh"
cd "$ACE_ROOT"

case "${1:-}" in
  pr)
    ./ace-build0.7.0.sh --no-demo
    python3 tools/ace-code_audit0.7.0.py
    tools/ace-check_header0.7.0.sh
    tools/ace-check_c0.7.0.sh
    python3 tools/ace-genbindings0.7.0.py --check
    ./ace-benchmark0.7.0.sh release-performance --quick
    ./demo/ace-run-demo0.7.0.sh
    ;;
  release)
    [[ -n "${ACE_AB_BASELINE_TREE:-}" ]] || ace_die "set ACE_AB_BASELINE_TREE to the $ACE_BASELINE source tree"
    ./ace-release0.7.0.sh "$ACE_AB_BASELINE_TREE"
    ;;
  fuzz|fuzz-smoke)
    # Without nightly + cargo-fuzz the mode is skipped locally; CI sets ACE_FUZZ_REQUIRED=1 so
    # a missing toolchain fails the job instead of passing it silently.
    if ! cargo +nightly fuzz --version >/dev/null 2>&1; then
      [[ "${ACE_FUZZ_REQUIRED:-0}" == 1 ]] && ace_die "cargo-fuzz / nightly toolchain not installed"
      ace_log "$1: SKIPPED (cargo-fuzz / nightly toolchain not installed)"
      exit 0
    fi
    if [[ "$1" == fuzz-smoke ]]; then
      seconds="${ACE_FUZZ_SMOKE_SECONDS:-25}"
    else
      seconds="${ACE_FUZZ_SECONDS:-60}"
    fi
    targets=$(cargo +nightly fuzz list --fuzz-dir fuzz)
    [[ $(wc -w <<<"$targets") -eq 20 ]] || ace_die "expected 20 fuzz targets, found: $targets"
    for target in $targets; do
      ace_step "$1 $target (${seconds}s)"
      cargo +nightly fuzz run --fuzz-dir fuzz "$target" -- -max_total_time="$seconds"
      if [[ "${ACE_FUZZ_CMIN:-0}" == 1 ]]; then
        cargo +nightly fuzz cmin --fuzz-dir fuzz "$target"
      fi
    done
    ;;
  *)
    ace_die "usage: $0 {pr|release|fuzz|fuzz-smoke}"
    ;;
esac
ace_log "ci '$1' completed"
