//! Genesis in the browser: the wasm entry that trunk builds and `serve` hands out.
//!
//! The page lives at `/{app}/` on `serve` ([spec 010](../../../specs/010_architecture.md)). It:
//!
//! 1. reads the app's name from the address,
//! 2. listens on `GET /api/{app}/config` (server-sent events) and draws the app from the first
//!    `config` event, or shows the problems from a `problems` event,
//! 3. runs the machine's clock, so `delay` and `every` fire on time,
//! 4. posts the mirror to `POST /api/{app}/mirror` whenever something has happened
//!    ([spec 050](../../../specs/050_state.md), "The mirror").
//!
//! Built natively, it's an empty program; everything here is for `wasm32`.

#[cfg(target_arch = "wasm32")]
mod browser;

fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::start();
}
