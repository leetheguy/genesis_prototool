#!/usr/bin/env bash
# The deployed checks (specs 010, 030, and 060): builds the browser bundle and genesis-serve, then
# drives the sample apps in headless Chromium: clicks and the mirror (hello.mjs), live edits
# (reload.mjs), calls to a mock API and PurpleFlow (calls.mjs), grid layout (layout.mjs), media (media.mjs), each (each.mjs), controls (controls.mjs), and forms (form.mjs).
set -euo pipefail
cd "$(dirname "$0")/.."
(cd apps/web && PATH="$HOME/.local/bin:$PATH" trunk build --quiet)
cargo build -q -p genesis-serve
(cd tests/deployed && npm install --silent)
node tests/deployed/hello.mjs
node tests/deployed/reload.mjs
node tests/deployed/calls.mjs
node tests/deployed/layout.mjs
node tests/deployed/media.mjs
node tests/deployed/each.mjs
node tests/deployed/controls.mjs
node tests/deployed/form.mjs
