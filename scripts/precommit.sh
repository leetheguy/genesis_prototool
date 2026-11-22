#!/usr/bin/env bash
# Every local check, in the order that fails fastest. See AGENTS.md (precommit).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
# The browser entry is wasm-only, so it's linted for wasm, and trunk builds it with the CSS.
cargo clippy -p genesis-web --target wasm32-unknown-unknown -- -D warnings
cargo machete
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
(cd apps/web && PATH="$HOME/.local/bin:$PATH" trunk build --quiet)
echo "precommit: all green"
