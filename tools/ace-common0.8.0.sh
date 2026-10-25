# shellcheck shell=bash
# Shared helpers of the ACE 0.8.0 scripts; sourced, never executed.
#
#   ACE_VERSION   product / Cargo / result-file version
#   ACE_BASELINE  predecessor used by A/B and stored-baseline diagnostics
#   ACE_RESULTS   benchmark output directory (ACE_BENCH_OUT_DIR is exported to match)
#   ace_log       prefixed progress line
#   ace_step      prefixed step banner
#   ace_die       error line + exit 1

ACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Number formatting must not depend on the user's locale: under e.g. pl_PL the shell's printf
# rejects "138.03" for %.2f (it expects a decimal comma). LC_ALL would override LC_NUMERIC, so
# its value is kept for the character set only.
if [[ -n "${LC_ALL:-}" ]]; then
  export LC_CTYPE="$LC_ALL"
  unset LC_ALL
fi
export LC_NUMERIC=C
ACE_VERSION="0.8.0"
ACE_BASELINE="0.7.0"
ACE_RESULTS="${ACE_BENCH_OUT_DIR:-$ACE_ROOT/examples/results}"
export ACE_BENCH_OUT_DIR="$ACE_RESULTS"

ace_log()  { printf '[ACE %s] %s\n' "$ACE_VERSION" "$*"; }
ace_step() { printf '\n[ACE %s] ==== %s ====\n' "$ACE_VERSION" "$*"; }
ace_die()  { printf '[ACE %s] ERROR: %s\n' "$ACE_VERSION" "$*" >&2; exit 1; }

# Path of the release ace-bench / ace / ace-corpus binaries.
ace_bin() { printf '%s/release/%s' "${CARGO_TARGET_DIR:-$ACE_ROOT/target}" "$1"; }
