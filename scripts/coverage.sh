#!/usr/bin/env bash
# Runs the tests and enforces the 90% line-coverage floor. See AGENTS.md (coverage).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo llvm-cov --workspace --fail-under-lines 90 "$@"
