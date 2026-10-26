#!/usr/bin/env bash
# Builds libace_ffi, compiles crates/ace-ffi/tests/c/smoke.c against include/ace.h with warnings
# as errors, links it dynamically and statically, and runs both. Also checks that the header
# parses as C++.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/ace-common0.8.0.sh"
cd "$ACE_ROOT"

CC="${CC:-cc}"
command -v "$CC" >/dev/null || ace_die "no C compiler ($CC)"

ace_step "building libace_ffi"
cargo build -p ace-ffi
target="${CARGO_TARGET_DIR:-target}/debug"
[[ -f "$target/libace_ffi.so" || -f "$target/libace_ffi.dylib" ]] || ace_die "no cdylib in $target"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
flags=(-std=c11 -Wall -Wextra -Werror -pedantic -Iinclude)

ace_step "C smoke test (dynamic)"
"$CC" "${flags[@]}" crates/ace-ffi/tests/c/smoke.c -L"$target" -lace_ffi -o "$work/smoke-dynamic"
LD_LIBRARY_PATH="$target" DYLD_LIBRARY_PATH="$target" "$work/smoke-dynamic"

ace_step "C smoke test (static)"
"$CC" "${flags[@]}" crates/ace-ffi/tests/c/smoke.c "$target/libace_ffi.a" -lpthread -ldl -lm -o "$work/smoke-static"
"$work/smoke-static"

if command -v "${CXX:-c++}" >/dev/null; then
  ace_step "header as C++"
  printf '#include "ace.h"\nint main() { return static_cast<int>(ace_abi_version()) == ACE_ABI_VERSION ? 0 : 1; }\n' \
    >"$work/header.cpp"
  "${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror -Iinclude "$work/header.cpp" -L"$target" -lace_ffi -o "$work/header-cpp"
  LD_LIBRARY_PATH="$target" DYLD_LIBRARY_PATH="$target" "$work/header-cpp"
fi
ace_log "C ABI smoke tests passed"
