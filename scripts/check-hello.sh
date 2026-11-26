#!/usr/bin/env bash
# The deployed checks (specs 010, 030, 060, and 110): builds the browser bundle and genesis-serve, then
# drives the sample apps in headless Chromium: clicks and the mirror (hello.mjs), live edits
# (reload.mjs), calls to a mock API and PurpleFlow (calls.mjs), grid layout (layout.mjs), media (media.mjs), each (each.mjs), controls (controls.mjs), forms (form.mjs), files (file.mjs), dialogs (dialog.mjs), markdown (markdown.mjs), components (components.mjs), show and disabled (show.mjs), checks on controls (checks.mjs), formats (format.mjs), toast and confirm (confirm.mjs), and pushes from an agent (push.mjs).
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
node tests/deployed/file.mjs
node tests/deployed/dialog.mjs
node tests/deployed/markdown.mjs
node tests/deployed/components.mjs
node tests/deployed/show.mjs
node tests/deployed/checks.mjs
node tests/deployed/format.mjs
node tests/deployed/confirm.mjs
node tests/deployed/push.mjs
