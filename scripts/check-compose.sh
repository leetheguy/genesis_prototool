#!/usr/bin/env bash
# The Docker Compose deployed check (spec 010): builds the image, then runs the real stack (serve and
# dufs) as a throwaway project and checks its boundaries. See tests/deployed/compose.mjs.
set -euo pipefail
cd "$(dirname "$0")/.."
docker build -q -t genesis-serve . >/dev/null
node tests/deployed/compose.mjs
