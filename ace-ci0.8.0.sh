#!/usr/bin/env bash
# Vendor-neutral CI entry point for ACE 0.8.0.
#
#   ./ace-ci0.8.0.sh pr        build gates + code audit + include/ace.h drift check (needs
#                              cbindgen 0.29.0) + C smoke test (needs a C compiler) + quick release-performance + demo
#   ./ace-ci0.8.0.sh aarch64   NEON selected and equal to scalar; golden bytes of the engine, the facade
#                              and the C ABI on this machine, with NEON and with ACE_SIMD=scalar
#                              (ACE_EXPECT_ARCH=aarch64 makes the job fail on any other CPU)
#   ./ace-ci0.8.0.sh jvm       Gradle build and tests of jvm/ (needs gradle 9.1+ and JDK 22 or newer)
#   ./ace-ci0.8.0.sh native    C ABI tests and the Java SDK tests on this OS (macOS, Windows, Linux);
#                              the library is built for this machine and packaged under native/<os>-<arch>/
#   ./ace-ci0.8.0.sh release   full ace-release0.8.0.sh (needs ACE_AB_BASELINE_TREE)
#   ./ace-ci0.8.0.sh fuzz      all 20 fuzz targets, ACE_FUZZ_SECONDS each (default 60);
#                              ACE_FUZZ_CMIN=1 minimises every corpus afterwards
#   ./ace-ci0.8.0.sh fuzz-smoke  all 20 fuzz targets, ACE_FUZZ_SMOKE_SECONDS each (default 25)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.8.0.sh"
cd "$ACE_ROOT"

case "${1:-}" in
  pr)
    ./ace-build0.8.0.sh --no-demo
    python3 tools/ace-code_audit0.8.0.py
    tools/ace-check_header0.8.0.sh
    tools/ace-check_c0.8.0.sh
    python3 tools/ace-genbindings0.8.0.py --check
    ./ace-benchmark0.8.0.sh release-performance --quick
    ./demo/ace-run-demo0.8.0.sh
    ;;
  aarch64)
    if [[ -n "${ACE_EXPECT_ARCH:-}" && "$(uname -m)" != "$ACE_EXPECT_ARCH" && "$(uname -m)" != "arm64" ]]; then
      ace_die "expected a $ACE_EXPECT_ARCH machine, running on $(uname -m)"
    fi
    ace_step "SIMD kernels on $(uname -m): NEON must be selected and agree with the scalar code"
    cargo test --release -p ace-simd
    cargo build --release -p ace-cli
    "$(ace_bin ace)" version | tee /dev/stderr | grep -q "simd backend:   Neon" \
      || ace_die "the NEON backend was not selected on this machine"
    ace_step "golden bytes on $(uname -m) (NEON)"
    cargo test --release -p ace-engine --test golden
    cargo test --release -p ace --test facade
    cargo test --release -p ace-ffi --test golden
    ace_step "golden bytes on $(uname -m) (ACE_SIMD=scalar)"
    ACE_SIMD=scalar cargo test --release -p ace-engine --test golden
    ACE_SIMD=scalar cargo test --release -p ace-ffi --test golden
    ;;
  jvm)
    command -v gradle >/dev/null || ace_die "gradle is not installed"
    java -version
    ace_step "gradle build (jvm/)"
    gradle -p jvm build --no-daemon --console=plain
    ;;
  native)
    # The same checks as `jvm`, limited to the Java module so that a platform without a Scala
    # toolchain in CI still proves that its library loads, packs and round-trips.
    ace_step "C ABI tests on $(uname -s) $(uname -m)"
    cargo test --release -p ace-ffi
    command -v gradle >/dev/null || ace_die "gradle is not installed"
    java -version
    ace_step "gradle :ace-native:build"
    gradle -p jvm :ace-native:build --no-daemon --console=plain
    ;;
  release)
    [[ -n "${ACE_AB_BASELINE_TREE:-}" ]] || ace_die "set ACE_AB_BASELINE_TREE to the $ACE_BASELINE source tree"
    ./ace-release0.8.0.sh "$ACE_AB_BASELINE_TREE"
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
    ace_die "usage: $0 {pr|aarch64|jvm|native|release|fuzz|fuzz-smoke}"
    ;;
esac
ace_log "ci '$1' completed"
