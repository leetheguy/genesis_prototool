//! Genesis on the desktop: which page the Tauri window opens.
//!
//! The desktop shell is a native window on a `serve` page, loaded the way a browser loads it
//! ([spec 010](../../../specs/010_architecture.md), "Clients"). It keeps no copy of the renderer, so
//! the desktop and the browser can't drift apart. This crate's library is the one decision the shell
//! makes, kept apart from Tauri so it's tested without a window:
//!
//! ```text
//! genesis-desktop [--serve URL] [APP]
//! ```
//!
//! `serve` is `--serve`, else the `GENESIS_SERVE` environment variable, else `http://127.0.0.1:8080`.
//! With an app it opens that app's page; without one, `serve`'s list of apps.
//!
//! ```
//! let url = genesis_desktop::window_url(vec!["hello".into()], None).unwrap();
//! assert_eq!(url.as_str(), "http://127.0.0.1:8080/hello/");
//! ```

use url::Url;

/// The page the window opens, from the shell's arguments (after the program's name) and the
/// `GENESIS_SERVE` environment variable. Fails with why: an option it doesn't know, a serve address
/// that isn't `http` or `https`, or an app name that isn't plain (letters, digits, `_`, `-`).
pub fn window_url(args: Vec<String>, env_serve: Option<String>) -> Result<Url, String> {
    let mut serve = env_serve.unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let mut app = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--serve" => serve = args.next().ok_or("--serve needs an address")?,
            option if option.starts_with('-') => {
                return Err(format!(
                    "unknown option {option}; it takes --serve URL and an app's name"
                ));
            }
            _ => app = Some(arg),
        }
    }
    let base = Url::parse(&serve).map_err(|e| format!("{serve} isn't an address: {e}"))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(format!("{serve} isn't an http or https address"));
    }
    let path = match app {
        Some(name)
            if name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') =>
        {
            format!("{name}/")
        }
        Some(name) => return Err(format!("{name:?} isn't a plain app name")),
        None => String::new(),
    };
    base.join(&format!("/{path}")).map_err(|e| e.to_string())
}
