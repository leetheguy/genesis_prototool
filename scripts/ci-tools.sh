#!/usr/bin/env bash
# Installs the browser build's tools for CI (.github/workflows/ci.yml): trunk and Tailwind's
# standalone binary, at the versions docs/setup.md pins (TRUNK_VERSION and TAILWIND_VERSION).
set -euo pipefail
mkdir -p "$HOME/.local/bin"
curl -sSfL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
  | tar -xz -C "$HOME/.local/bin"
curl -sSfL -o "$HOME/.local/bin/tailwindcss" \
  "https://github.com/tailwindlabs/tailwindcss/releases/download/v${TAILWIND_VERSION}/tailwindcss-linux-x64"
chmod +x "$HOME/.local/bin/tailwindcss"
echo "$HOME/.local/bin" >> "${GITHUB_PATH:-/dev/null}"
