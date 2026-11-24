//! Genesis on the desktop: which page the Tauri window opens.
//!
//! The desktop shell is a native window on a `serve` page, loaded the way a browser loads it
//! ([spec 010](../../../specs/010_architecture.md), "Clients"). It keeps no copy of the renderer, so
//! the desktop and the browser can't drift apart. This crate's library is the one decision the shell
//! makes, kept apart from Tauri so it's tested without a window:
//!
//! ```text
//! genesis-desktop [--serve URL]
//! ```
//!
//! `serve` is `--serve`, else the `GENESIS_SERVE` environment variable, else `http://127.0.0.1:8080`.
//! The window opens `serve`'s root, where it draws the one app
//! ([spec 120](../../../specs/120_one_app.md)).
//!
//! ```
//! let url = genesis_desktop::window_url(vec![], None).unwrap();
//! assert_eq!(url.as_str(), "http://127.0.0.1:8080/");
//! ```

use url::Url;

/// The page the window opens, from the shell's arguments (after the program's name) and the
/// `GENESIS_SERVE` environment variable. Fails with why: an option it doesn't know, a serve address
/// that isn't `http` or `https`, or any other argument (there's one app, so there's no app to name).
pub fn window_url(args: Vec<String>, env_serve: Option<String>) -> Result<Url, String> {
    let mut serve = env_serve.unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--serve" => serve = args.next().ok_or("--serve needs an address")?,
            option if option.starts_with('-') => {
                return Err(format!("unknown option {option}; it takes --serve URL"));
            }
            _ => {
                return Err(format!(
                    "{arg:?} isn't an option; serve runs one app, so the window takes no app's name"
                ));
            }
        }
    }
    let base = Url::parse(&serve).map_err(|e| format!("{serve} isn't an address: {e}"))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(format!("{serve} isn't an http or https address"));
    }
    base.join("/").map_err(|e| e.to_string())
}
