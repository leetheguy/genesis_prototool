//! Genesis in the browser: the wasm entry that trunk builds and `serve` hands out.
//!
//! The page lives at `/` on `serve`, which runs one app ([spec 010](../../../specs/010_architecture.md),
//! [spec 120](../../../specs/120_one_app.md)). It:
//!
//! 1. listens on `GET /api/config` (server-sent events): draws the app from the first
//!    `config` event and swaps each later one in live, keeping the state; shows a `problems`
//!    event over the last good screen (or alone, before there is one),
//! 2. makes the machine's calls through `POST /api/call` and hands each answer back
//!    ([spec 030](../../../specs/030_connections.md)),
//! 3. keeps the session in local storage and brings it back on the next open
//!    ([spec 050](../../../specs/050_state.md), "Persisting"),
//! 4. runs the machine's clock, so `delay` and `every` fire on time,
//! 5. posts the mirror to `POST /api/mirror` whenever something has happened
//!    ([spec 050](../../../specs/050_state.md), "The mirror").
//!
//! Built natively, it's an empty program; everything here is for `wasm32`.

#[cfg(target_arch = "wasm32")]
mod browser;

fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::start();
}
