#!/usr/bin/env bash
# Regenerates include/ace.h with cbindgen and fails when the committed header differs.
#
#   tools/ace-check_header0.8.0.sh           check (CI)
#   tools/ace-check_header0.8.0.sh --write   regenerate include/ace.h
#
# Needs cbindgen 0.29.0:  cargo install cbindgen --locked --version 0.29.0
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/ace-common0.8.0.sh"
cd "$ACE_ROOT"

CBINDGEN_VERSION="0.29.0"
command -v cbindgen >/dev/null || ace_die "cbindgen not found: cargo install cbindgen --locked --version $CBINDGEN_VERSION"
[[ "$(cbindgen --version)" == "cbindgen $CBINDGEN_VERSION" ]] \
  || ace_die "cbindgen $CBINDGEN_VERSION required, found $(cbindgen --version)"

generated="$(mktemp)"
trap 'rm -f "$generated"' EXIT
cbindgen --config cbindgen.toml --crate ace-ffi --lockfile Cargo.lock --output "$generated" 2>/dev/null

if [[ "${1:-}" == "--write" ]]; then
  mkdir -p include
  cp "$generated" include/ace.h
  ace_log "include/ace.h written"
  exit 0
fi

if ! diff -u include/ace.h "$generated"; then
  ace_die "include/ace.h is out of date: run tools/ace-check_header0.8.0.sh --write and commit the result"
fi
ace_log "include/ace.h matches crates/ace-ffi"
