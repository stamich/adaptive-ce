# shellcheck shell=bash
# Shared helpers of the ACE 0.5.0 scripts; sourced, never executed.
#
#   ACE_VERSION   product / Cargo / result-file version
#   ACE_BASELINE  predecessor used by A/B and stored-baseline diagnostics
#   ACE_RESULTS   benchmark output directory (ACE_BENCH_OUT_DIR is exported to match)
#   ace_log       prefixed progress line
#   ace_step      prefixed step banner
#   ace_die       error line + exit 1

ACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ACE_VERSION="0.5.0"
ACE_BASELINE="0.4.6"
ACE_RESULTS="${ACE_BENCH_OUT_DIR:-$ACE_ROOT/examples/results}"
export ACE_BENCH_OUT_DIR="$ACE_RESULTS"

ace_log()  { printf '[ACE %s] %s\n' "$ACE_VERSION" "$*"; }
ace_step() { printf '\n[ACE %s] ==== %s ====\n' "$ACE_VERSION" "$*"; }
ace_die()  { printf '[ACE %s] ERROR: %s\n' "$ACE_VERSION" "$*" >&2; exit 1; }

# Path of the release ace-bench / ace / ace-corpus binaries.
ace_bin() { printf '%s/target/release/%s' "$ACE_ROOT" "$1"; }
