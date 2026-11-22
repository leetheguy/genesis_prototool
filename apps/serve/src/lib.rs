//! `genesis-serve`: hands out the renderer and each app's config, and writes the mirror.
//!
//! Each app is a folder in the configs folder, `configs/{app}/app.toml`
//! ([spec 010](../../../specs/010_architecture.md)). The routes:
//!
//! | Route | What |
//! |---|---|
//! | `GET /` | the list of apps, each a link to its page |
//! | `GET /status` | each app, `loaded` or `failed` with its problems, as JSON, so an agent can check its change took |
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
use futures_util::stream::{self, StreamExt};
use genesis_core::config::App;
use serde_json::Value;
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;

/// The configs folder's shared components folder (spec 090); a change in it reaches every app.
pub const SHARED_COMPONENTS: &str = "components";

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
    /// How long the configs folder must be quiet after a change before apps reload.
    pub debounce: Duration,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            configs: "configs".into(),
            state: "state".into(),
            dist: "apps/web/dist".into(),
            listen: "127.0.0.1:8080".into(),
            debounce: Duration::from_millis(150),
        }
    }
}

impl Settings {
    /// Reads `--configs DIR`, `--state DIR`, `--dist DIR`, `--listen ADDRESS`, and `--debounce MS`,
    /// each optional.
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
                "--debounce" => {
                    let ms = value()?;
                    let ms = ms
                        .parse()
                        .map_err(|_| format!("--debounce takes milliseconds, not {ms}"))?;
                    settings.debounce = Duration::from_millis(ms);
                }
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

/// What serve's routes share: the settings, and the news of which app's files changed.
struct Server {
    settings: Settings,
    /// An app's name each time its files change (`*` for a change outside any one app's folder).
    changes: broadcast::Sender<String>,
}

/// Keeps the configs folder watched for as long as it's held. Dropping it stops hot reload.
pub struct Watching(#[allow(dead_code)] notify::RecommendedWatcher);

/// serve's routes, with hot reload ([spec 060](../../../specs/060_hot_reload.md)): the configs
/// folder is watched, and once it's been quiet for `settings.debounce` after a change, every open
/// config stream of each app that changed gets the app's new config, or its problems.
///
/// Must be called inside a Tokio runtime. Fails if the configs folder can't be watched.
pub fn live_router(mut settings: Settings) -> Result<(Router, Watching), String> {
    settings.configs = settings
        .configs
        .canonicalize()
        .map_err(|e| format!("configs folder {}: {e}", settings.configs.display()))?;
    let (configs, debounce) = (settings.configs.clone(), settings.debounce);
    let server = Arc::new(Server {
        settings,
        changes: broadcast::channel(64).0,
    });
    let (paths, mut changed) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        // Reading a file to reload it is an access, not a change; counting it would reload forever.
        if let Some(event) = event.ok().filter(|e| !e.kind.is_access()) {
            for path in event.paths {
                let _ = paths.send(path);
            }
        }
    })
    .map_err(|e| format!("can't watch the configs folder: {e}"))?;
    notify::Watcher::watch(&mut watcher, &configs, notify::RecursiveMode::Recursive)
        .map_err(|e| format!("can't watch {}: {e}", configs.display()))?;
    let changes = server.changes.clone();
    tokio::spawn(async move {
        while let Some(first) = changed.recv().await {
            let mut apps = BTreeSet::from([app_of(&configs, &first)]);
            // Editors and file servers save in bursts: wait for the folder to go quiet.
            loop {
                match tokio::time::timeout(debounce, changed.recv()).await {
                    Ok(Some(path)) => {
                        apps.insert(app_of(&configs, &path));
                    }
                    Ok(None) => return,
                    Err(_quiet) => break,
                }
            }
            for app in apps {
                let _ = changes.send(app);
            }
        }
    });
    Ok((routes(server), Watching(watcher)))
}

/// Which app a changed path belongs to: the first folder under the configs folder, or `*` when
/// it's outside any app's folder, or in the shared `components/` folder every app may use
/// (spec 090).
fn app_of(configs: &Path, path: &Path) -> String {
    let first = path
        .strip_prefix(configs)
        .ok()
        .and_then(|rest| rest.components().next());
    match first.and_then(|c| c.as_os_str().to_str()) {
        Some(name) if plain(name) && name != SHARED_COMPONENTS => name.to_string(),
        _ => "*".to_string(),
    }
}

/// serve's routes, without hot reload: each config stream sends the app once.
pub fn router(settings: Settings) -> Router {
    routes(Arc::new(Server {
        settings,
        changes: broadcast::channel(1).0,
    }))
}

fn routes(server: Arc<Server>) -> Router {
    let bundle = ServeDir::new(&server.settings.dist);
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/", get(list))
        .route("/status", get(status))
        .route("/{app}", get(page))
        .route("/{app}/", get(page))
        .route("/api/{app}/config", get(config))
        .route("/api/{app}/mirror", post(mirror))
        .nest_service("/_genesis", bundle)
        .with_state(server)
}

type Shared = State<Arc<Server>>;

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

/// Every app's name: each folder in the configs folder with a plain name and an `app.toml`, in order.
fn app_names(settings: &Settings) -> Vec<String> {
    let entries = std::fs::read_dir(&settings.configs)
        .into_iter()
        .flatten()
        .flatten();
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| plain(name) && settings.configs.join(name).join("app.toml").is_file())
        .collect();
    names.sort();
    names
}

async fn list(State(server): Shared) -> Html<String> {
    let names = app_names(&server.settings);
    // Names are plain (letters, digits, _ and -), so they're safe in HTML as they are.
    let items: String = names
        .iter()
        .map(|n| format!(r#"<li><a class="link" href="/{n}/">{n}</a></li>"#))
        .collect();
    let body = if names.is_empty() {
        "<p>No apps yet. Add a folder with an app.toml to the configs folder.</p>".to_string()
    } else {
        format!("<ul>{items}</ul>")
    };
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Genesis</title></head><body><h1>Genesis apps</h1>{body}</body></html>"
    ))
}

async fn status(State(server): Shared) -> Json<Value> {
    let settings = &server.settings;
    let mut apps = serde_json::Map::new();
    for name in app_names(settings) {
        let report = match load(&settings.configs.join(&name).join("app.toml")).await {
            Ok(_) => serde_json::json!({ "status": "loaded" }),
            Err(problems) => {
                serde_json::json!({ "status": "failed", "problems": problems.lines().collect::<Vec<_>>() })
            }
        };
        apps.insert(name, report);
    }
    Json(serde_json::json!({ "apps": apps }))
}

async fn page(State(server): Shared, UrlPath(name): UrlPath<String>) -> Response {
    let settings = &server.settings;
    if let Err(refused) = app_dir(settings, &name) {
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

async fn config(State(server): Shared, UrlPath(name): UrlPath<String>) -> Response {
    let file = match app_dir(&server.settings, &name) {
        Ok(dir) => dir.join("app.toml"),
        Err(refused) => return refused.into_response(),
    };
    // Listen before the first load, so a change during it isn't missed.
    let news = BroadcastStream::new(server.changes.subscribe());
    let first = {
        let file = file.clone();
        stream::once(async move { app_event(&file).await })
    };
    // After the first event the stream stays open: a closed stream would make the browser
    // reconnect and ask again every few seconds. Each change to this app sends it again.
    let later = news.filter_map(move |news| {
        let (file, name) = (file.clone(), name.clone());
        async move {
            match news {
                Ok(app) if app != name && app != "*" => None,
                // Ok for this app, or a lag that may have dropped news for it: send it again.
                _ => Some(app_event(&file).await),
            }
        }
    });
    Sse::new(first.chain(later).map(Ok::<_, Infallible>)).into_response()
}

/// The event an app's file makes now: `config` with the app, or `problems`.
async fn app_event(file: &Path) -> Event {
    match load(file).await {
        Ok(app) => Event::default()
            .event("config")
            .data(serde_json::to_string(&app).unwrap_or_default()),
        Err(problems) => Event::default().event("problems").data(problems),
    }
}

/// Loads an app, or says what's wrong with it, one problem per line as `app.toml:LINE:COLUMN: ...`.
async fn load(file: &Path) -> Result<App, String> {
    let text = tokio::fs::read_to_string(file)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "app.toml not found".to_string(),
            _ => format!("app.toml: {e}"),
        })?;
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
    State(server): Shared,
    UrlPath(name): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    let settings = &server.settings;
    if let Err(refused) = app_dir(settings, &name) {
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
