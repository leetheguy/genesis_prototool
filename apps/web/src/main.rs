//! Genesis in the browser: the wasm entry that trunk builds and `serve` hands out.
//!
//! The page lives at `/{app}/` on `serve` ([spec 010](../../../specs/010_architecture.md)). It:
//!
//! 1. reads the app's name from the address,
//! 2. listens on `GET /api/{app}/config` (server-sent events): draws the app from the first
//!    `config` event and swaps each later one in live, keeping the state; shows a `problems`
//!    event over the last good screen (or alone, before there is one),
//! 3. makes the machine's calls through `POST /api/{app}/call` and hands each answer back
//!    ([spec 030](../../../specs/030_connections.md)),
//! 4. runs the machine's clock, so `delay` and `every` fire on time,
//! 5. posts the mirror to `POST /api/{app}/mirror` whenever something has happened
//!    ([spec 050](../../../specs/050_state.md), "The mirror").
//!
//! Built natively, it's an empty program; everything here is for `wasm32`.

#[cfg(target_arch = "wasm32")]
mod browser;

fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::start();
}
