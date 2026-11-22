//! `genesis-serve`: hands out the renderer and each app's config, and writes the mirror.
//!
//! Each app is a folder in the configs folder, `configs/{app}/app.toml`
//! ([spec 010](../../../specs/010_architecture.md)). The routes:
//!
//! | Route | What |
//! |---|---|
//! | `GET /{app}/` | the renderer's page, for an app that exists |
//! | `GET /_genesis/*` | the renderer's bundle (`trunk build` output) |
//! | `GET /api/{app}/config` | server-sent events: `config` with the loaded app as JSON, or `problems` with each one's `app.toml:LINE:COLUMN` |
//! | `POST /api/{app}/mirror` | a client's state and trail entries, written to the state folder |
//! | `GET /health` | `200` when the server is up |
//!
//! The mirror ([spec 050](../../../specs/050_state.md), "The mirror") turns names from the outside
//! world into file paths, so app and client names must be plain (letters, digits, `_`, `-`), or the
//! request is refused before anything touches the disk. Each client's state is written whole
//! (to a temporary file, then renamed), and its events are appended as JSON lines, keeping the last
//! [`EVENTS_KEPT`].

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use genesis_core::config::App;
use serde_json::Value;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::services::ServeDir;

/// How many events a client's `.events.ndjson` keeps, newest last.
pub const EVENTS_KEPT: usize = 10_000;

/// Where serve finds things, and where it listens.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The configs folder: one folder per app.
    pub configs: PathBuf,
    /// The state folder, where the mirror is written.
    pub state: PathBuf,
    /// The renderer's bundle.
    pub dist: PathBuf,
    /// The address to listen on.
    pub listen: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            configs: "configs".into(),
            state: "state".into(),
            dist: "apps/web/dist".into(),
            listen: "127.0.0.1:8080".into(),
        }
    }
}

impl Settings {
    /// Reads `--configs DIR`, `--state DIR`, `--dist DIR`, and `--listen ADDRESS`, each optional.
    /// It listens on `127.0.0.1:8080` unless told otherwise.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Settings, String> {
        let mut settings = Settings::default();
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--configs" => settings.configs = value()?.into(),
                "--state" => settings.state = value()?.into(),
                "--dist" => settings.dist = value()?.into(),
                "--listen" => settings.listen = value()?,
                _ => {
                    return Err(format!(
                        "unknown option {flag}; serve takes --configs, --state, --dist, and --listen"
                    ));
                }
            }
        }
        Ok(settings)
    }
}

/// All of serve's routes, over `settings`.
pub fn router(settings: Settings) -> Router {
    let bundle = ServeDir::new(&settings.dist);
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/{app}", get(page))
        .route("/{app}/", get(page))
        .route("/api/{app}/config", get(config))
        .route("/api/{app}/mirror", post(mirror))
        .nest_service("/_genesis", bundle)
        .with_state(Arc::new(settings))
}

type Shared = State<Arc<Settings>>;

/// A refusal: its status, and a message saying why. It becomes the response as it is.
type Refusal = (StatusCode, String);

fn refuse(status: StatusCode, why: String) -> Response {
    (status, why).into_response()
}

/// The folder of the app `name`, when the name is plain and the app exists.
fn app_dir(settings: &Settings, name: &str) -> Result<PathBuf, Refusal> {
    if !plain(name) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{name:?} isn't a plain app name"),
        ));
    }
    let dir = settings.configs.join(name);
    if !dir.join("app.toml").is_file() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("there's no app named {name}"),
        ));
    }
    Ok(dir)
}

/// Letters, digits, `_`, and `-` only, so a name can never leave the folder it's joined to.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

async fn page(State(settings): Shared, UrlPath(name): UrlPath<String>) -> Response {
    if let Err(refused) = app_dir(&settings, &name) {
        return refused.into_response();
    }
    match tokio::fs::read_to_string(settings.dist.join("index.html")).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the renderer isn't built (run trunk build in apps/web): {e}"),
        ),
    }
}

async fn config(State(settings): Shared, UrlPath(name): UrlPath<String>) -> Response {
    let dir = match app_dir(&settings, &name) {
        Ok(dir) => dir,
        Err(refused) => return refused.into_response(),
    };
    let event = match load(&dir.join("app.toml")).await {
        Ok(app) => Event::default()
            .event("config")
            .data(serde_json::to_string(&app).unwrap_or_default()),
        Err(problems) => Event::default().event("problems").data(problems),
    };
    // One event, then the stream stays open: a closed stream would make the browser reconnect
    // and ask again every few seconds. Hot reload (spec 060) sends more events on it.
    let events: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(stream::once(async move { Ok(event) }).chain(stream::pending()));
    Sse::new(events).into_response()
}

/// Loads an app, or says what's wrong with it, one problem per line as `app.toml:LINE:COLUMN: ...`.
async fn load(file: &Path) -> Result<App, String> {
    let text = tokio::fs::read_to_string(file)
        .await
        .map_err(|e| format!("app.toml: {e}"))?;
    App::from_toml(&text).map_err(|problems| {
        let lines: Vec<String> = problems
            .iter()
            .map(|p| match (p.line, p.column) {
                (Some(line), Some(column)) => format!("app.toml:{line}:{column}: {}", p.message),
                _ => format!("app.toml: {}", p.message),
            })
            .collect();
        lines.join("\n")
    })
}

async fn mirror(
    State(settings): Shared,
    UrlPath(name): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    if let Err(refused) = app_dir(&settings, &name) {
        return refused.into_response();
    }
    let client = body["client"].as_str().unwrap_or_default();
    if !plain(client) {
        return refuse(
            StatusCode::BAD_REQUEST,
            format!("{client:?} isn't a plain client name"),
        );
    }
    let dir = settings.state.join(&name);
    let events: Vec<String> = body["events"]
        .as_array()
        .map(|e| e.iter().map(Value::to_string).collect())
        .unwrap_or_default();
    match write_mirror(&dir, client, &body["state"], &events).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the mirror couldn't be written: {e}"),
        ),
    }
}

/// Writes a client's state whole, and adds its events to the end of its event file.
async fn write_mirror(
    dir: &Path,
    client: &str,
    state: &Value,
    events: &[String],
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    write_whole(&dir.join(format!("{client}.json")), &state.to_string()).await?;
    let file = dir.join(format!("{client}.events.ndjson"));
    let old = match tokio::fs::read_to_string(&file).await {
        Ok(old) => old,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let lines: Vec<&str> = old
        .lines()
        .chain(events.iter().map(String::as_str))
        .collect();
    let kept = &lines[lines.len().saturating_sub(EVENTS_KEPT)..];
    write_whole(&file, &(kept.join("\n") + "\n")).await
}

/// Writes a file whole: to a temporary file beside it, then renamed over it, so a reader never
/// sees half of one.
async fn write_whole(file: &Path, text: &str) -> std::io::Result<()> {
    let temporary = file.with_extension("tmp");
    tokio::fs::write(&temporary, text).await?;
    tokio::fs::rename(&temporary, file).await
}
