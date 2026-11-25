//! Genesis's core: everything that isn't input or output.
//!
//! Genesis turns a folder of TOML (an *app*) into a live interface. This crate holds the parts that
//! only think: the config types, the event bus, and the state machine. It does no IO, so it
//! compiles both natively (for the server, the bench, and fast tests) and to WebAssembly (for the
//! renderer in the browser).
//!
//! See `specs/000_overview.md` for the whole model and `specs/100_events.md` for the bus.

pub mod bus;
pub mod condition;
pub mod config;
pub mod file;
pub mod markdown;
pub mod pattern;
pub mod style;
pub mod svg;
pub mod template;
