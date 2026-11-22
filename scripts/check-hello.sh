#!/usr/bin/env bash
# The deployed checks (specs 010 and 060): builds the browser bundle and genesis-serve, then drives
# the hello app in headless Chromium: clicks and the mirror (hello.mjs), and live edits (reload.mjs).
set -euo pipefail
cd "$(dirname "$0")/.."
(cd apps/web && PATH="$HOME/.local/bin:$PATH" trunk build --quiet)
cargo build -q -p genesis-serve
(cd tests/deployed && npm install --silent)
node tests/deployed/hello.mjs
node tests/deployed/reload.mjs
