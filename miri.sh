#!/usr/bin/env bash
set -euo pipefail

# Runs the full test suite under Miri using cargo-nextest and captures output
# to a timestamped log.
#
# Notes:
# - Miri is single-threaded per process, but nextest runs each test in its own
#   process and can parallelize across tests. :contentReference[oaicite:1]{index=1}
# - nextest has its own reporting and filtering; libtest flags like
#   --test-threads/--nocapture/--report-time do not apply. :contentReference[oaicite:2]{index=2}
#
# Usage examples:
#   ./miri-nextest.sh
#   JOBS=32 ./miri-nextest.sh
#   ./miri-nextest.sh -j 16
#   ./miri-nextest.sh -p mycrate
#   ./miri-nextest.sh -E 'test(foo::bar)'     # nextest expression filtering (if you use it)
#
# Environment:
#   MIRIFLAGS   - passed through to Miri (unstable; depends on nightly)
#   JOBS        - default parallelism if -j/--jobs isn't passed
#   NEXTEST_PROFILE - nextest profile (default "default")

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

mkdir -p miri-logs

TS="$(date +%Y%m%d-%H%M%S)"
LOG="miri-logs/miri-nextest-${TS}.log"

export RUST_BACKTRACE=1
export MIRI_BACKTRACE=1
export MIRIFLAGS="${MIRIFLAGS:-}"
export NEXTEST_PROFILE="${NEXTEST_PROFILE:-default}"

# If user didn't pass -j/--jobs explicitly, set it from JOBS (or fall back to CPU count).
# We do this by augmenting args. (If you *do* pass -j, it wins.)
add_default_jobs_arg() {
  local -a args=("$@")

  # detect -j / --jobs already present
  local i
  for ((i=0; i<${#args[@]}; i++)); do
    case "${args[$i]}" in
      -j|--jobs|--jobs=*)
        printf '%s\0' "${args[@]}"
        return 0
        ;;
    esac
  done

  local jobs="${JOBS:-}"
  if [[ -z "$jobs" ]]; then
    if command -v nproc >/dev/null 2>&1; then
      jobs="$(nproc)"
    else
      # macOS fallback
      jobs="$(sysctl -n hw.ncpu 2>/dev/null || echo 1)"
    fi
  fi

  args+=("-j" "$jobs")
  printf '%s\0' "${args[@]}"
}

# Basic sanity: nextest must be installed (as a cargo subcommand).
# (We don't auto-install here.)
check_nextest() {
  if ! cargo nextest --version >/dev/null 2>&1; then
    echo "ERROR: cargo-nextest is not available (cargo nextest --version failed)." | tee -a "$LOG"
    echo "Install cargo-nextest, then re-run." | tee -a "$LOG"
    exit 2
  fi
}

{
  echo "=== Miri + nextest run: ${TS} ==="
  echo "pwd: $(pwd)"
  echo "rustc: $(rustc -V)"
  echo "cargo: $(cargo -V)"
  echo "nightly: $(cargo +nightly -V)"
  echo "miri: $(cargo +nightly miri --version || true)"
  echo "nextest: $(cargo nextest --version || true)"
  echo "RUST_BACKTRACE=${RUST_BACKTRACE}"
  echo "MIRI_BACKTRACE=${MIRI_BACKTRACE}"
  echo "MIRIFLAGS=${MIRIFLAGS}"
  echo "NEXTEST_PROFILE=${NEXTEST_PROFILE}"
  echo "JOBS=${JOBS:-}"
  echo
} | tee "$LOG"

check_nextest

# One-time setup step (idempotent; may update/download the sysroot).
(
  set -x
  cargo +nightly miri setup
) 2>&1 | tee -a "$LOG"

# Add default -j if not provided by caller.
# Using NUL delim to preserve exact args safely.
mapfile -d '' -t RUN_ARGS < <(add_default_jobs_arg "$@")

# Run tests under Miri using nextest.
# Keep standard cargo selection flags in these args, e.g. --all-features, -p, --workspace, etc.
(
  set -x
  cargo +nightly miri nextest run \
    --profile "$NEXTEST_PROFILE" \
    --all-features \
    "${RUN_ARGS[@]}"
) 2>&1 | tee -a "$LOG"

echo "=== Done. Log saved to: ${LOG} ===" | tee -a "$LOG"
