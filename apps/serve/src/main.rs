//! `genesis-serve [--configs DIR] [--state DIR] [--logs DIR] [--dist DIR] [--listen ADDRESS] [--debounce MS]`.
//!
//! See the library's docs for the routes. It listens on `127.0.0.1:8080` unless told otherwise,
//! and reloads apps live as their files change (`--debounce MS`, default 150). From its environment:
//! `GENESIS_CREDS_NAME` is `{{ creds.NAME }}`, and `GENESIS_FILES_URL` with `GENESIS_AGENT_TOKEN`
//! opens `/fs/`.

use genesis_serve::{Settings, live_router};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let settings = match Settings::from_args(std::env::args().skip(1)) {
        Ok(settings) => settings.with_env(std::env::vars()),
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    let (router, _watching) = match live_router(settings.clone()) {
        Ok(live) => live,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(1);
        }
    };
    let listener = match tokio::net::TcpListener::bind(&settings.listen).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("can't listen on {}: {e}", settings.listen);
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "genesis-serve on http://{}/ (configs: {})",
        settings.listen,
        settings.configs.display()
    );
    if let Err(e) = axum::serve(listener, router).await {
        eprintln!("serve stopped: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
