#!/usr/bin/env bash
# Every local check, in the order that fails fastest. See AGENTS.md (precommit).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo machete
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
echo "precommit: all green"
