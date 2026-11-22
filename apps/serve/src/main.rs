//! `genesis-serve [--configs DIR] [--state DIR] [--dist DIR] [--listen ADDRESS]`.
//!
//! See the library's docs for the routes. It listens on `127.0.0.1:8080` unless told otherwise.

use genesis_serve::{Settings, router};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let settings = match Settings::from_args(std::env::args().skip(1)) {
        Ok(settings) => settings,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
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
    if let Err(e) = axum::serve(listener, router(settings)).await {
        eprintln!("serve stopped: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
