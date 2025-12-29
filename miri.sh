#!/usr/bin/env bash
set -euo pipefail

# Runs the full test suite under Miri and captures all output to a timestamped log.
#
# Notes:
# - There are no built-in timeouts here; it will run until completion unless your
#   shell/CI runner enforces a job timeout.
# - Miri is much more reliable with a single test thread.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

mkdir -p miri-logs

TS="$(date +%Y%m%d-%H%M%S)"
LOG="miri-logs/miri-${TS}.log"

export RUST_BACKTRACE=1
export MIRI_BACKTRACE=1

# Extra pointer tracking tends to improve diagnostics when Miri finds UB.
export MIRIFLAGS="${MIRIFLAGS:-}-Zmiri-tag-raw-pointers -Zmiri-track-raw-pointers"

{
  echo "=== Miri run: ${TS} ==="
  echo "pwd: $(pwd)"
  echo "rustc: $(rustc -V)"
  echo "cargo: $(cargo -V)"
  echo "nightly: $(cargo +nightly -V)"
  echo "miri: $(cargo +nightly miri --version || true)"
  echo "RUST_BACKTRACE=${RUST_BACKTRACE}"
  echo "MIRI_BACKTRACE=${MIRI_BACKTRACE}"
  echo "MIRIFLAGS=${MIRIFLAGS}"
  echo
} | tee "$LOG"

# One-time setup step (idempotent; may update/download the sysroot).
(
  set -x
  cargo +nightly miri setup
) 2>&1 | tee -a "$LOG"

# Run all tests under Miri; force serial execution and keep output.
(
  set -x
  cargo +nightly miri test --all-features \
    -- -Zunstable-options --report-time --test-threads=1 --nocapture
) 2>&1 | tee -a "$LOG"

echo "=== Done. Log saved to: ${LOG} ===" | tee -a "$LOG"
