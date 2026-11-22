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
```

Then:

- `scripts/precommit.sh`: every local check (AGENTS.md, **precommit**).
- `scripts/coverage.sh`: the tests, with the 90% line-coverage floor.

`.cargo/config.toml` caps builds at two parallel jobs, because the first box had about 1.5 GB of free
memory. Raise it on a bigger machine.
