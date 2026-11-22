#!/bin/sh
# Builds style/genesis.css: Tailwind CSS 4, with daisyUI 5 and all of its themes, from the classes
# genesis-ui's components use. Versions are pinned here; see docs/setup.md for the Tailwind binary.
set -eu
cd "$(dirname "$0")"
DAISYUI_VERSION=5.8.2
mkdir -p style/vendor
if [ ! -f "style/vendor/daisyui-$DAISYUI_VERSION.mjs" ]; then
  curl -sSfL -o "style/vendor/daisyui-$DAISYUI_VERSION.mjs" \
    "https://github.com/saadeghi/daisyui/releases/download/v$DAISYUI_VERSION/daisyui.mjs"
fi
cp "style/vendor/daisyui-$DAISYUI_VERSION.mjs" style/vendor/daisyui.mjs
TAILWIND="$(command -v tailwindcss || echo "$HOME/.local/bin/tailwindcss")"
"$TAILWIND" -i style/input.css -o style/genesis.css --minify
