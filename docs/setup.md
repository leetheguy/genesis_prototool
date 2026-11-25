# Setup

The steps that got the toolchain working on a fresh Linux box (2026-11-22). Follow them in order.

```bash
# Rust (stable), with the components precommit and coverage use, and the wasm target for the renderer
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
  --component clippy,rustfmt,llvm-tools-preview
. ~/.cargo/env
rustup target add wasm32-unknown-unknown

# The quality tools, as prebuilt binaries (building them from source takes a long time)
curl -L --proto '=https' --tlsv1.2 -sSf \
  https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall --no-confirm cargo-machete cargo-llvm-cov cargo-mutants cargo-audit

# The browser build: trunk (bundles the wasm) and Tailwind's standalone binary (no Node needed).
# apps/web/build-css.sh fetches daisyUI's plugin file itself, at the version it pins.
cargo binstall --no-confirm trunk@0.22.2

# The desktop window (genesis-desktop's `app` feature): WebKitGTK and the libraries Tauri 2 links.
# Xvfb lets it run on a box with no screen.
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev xvfb
mkdir -p ~/.local/bin
curl -sSL -o ~/.local/bin/tailwindcss \
  https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.5/tailwindcss-linux-x64
chmod +x ~/.local/bin/tailwindcss
```

Then:

- `scripts/precommit.sh`: every local check (AGENTS.md, **precommit**).
- `scripts/coverage.sh`: the tests, with the 90% line-coverage floor.
- `cd apps/web && trunk build`: the browser bundle, into `apps/web/dist/` (precommit builds it too).

`.cargo/config.toml` caps builds at two parallel jobs, because the first box had about 1.5 GB of free
memory. Raise it on a bigger machine.

## CI

`.github/workflows/ci.yml` sets up the same toolchain on GitHub's runners: Rust stable with the wasm
target, `cargo-machete` and `cargo-llvm-cov` (prebuilt, through `taiki-e/install-action`), and trunk and
Tailwind at the versions above (`scripts/ci-tools.sh`). The browser checks get Chromium from
`npx playwright-core install --with-deps chromium-headless-shell` and find it through `GENESIS_CHROME`.
