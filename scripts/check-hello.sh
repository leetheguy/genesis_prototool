#!/usr/bin/env bash
# The hello deployed check (spec 010): builds the browser bundle and genesis-serve, then drives
# the hello app in headless Chromium and checks the screen and the mirror. See tests/deployed/hello.mjs.
set -euo pipefail
cd "$(dirname "$0")/.."
(cd apps/web && PATH="$HOME/.local/bin:$PATH" trunk build --quiet)
cargo build -q -p genesis-serve
(cd tests/deployed && npm install --silent)
node tests/deployed/hello.mjs
