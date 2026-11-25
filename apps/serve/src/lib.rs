//! `genesis-serve`: hands out the renderer and the app's config, makes its calls, and writes the mirror.
//!
//! The configs folder is the app, with `app.toml` at its top ([spec 120](../../../specs/120_one_app.md),
//! [spec 010](../../../specs/010_architecture.md)). The routes:
//!
//! | Route | What |
//! |---|---|
//! | `GET /` | the renderer's page, drawing the app |
//! | `GET /agents` | the configs folder's `AGENTS.md`, the guide for agents, with `BASE` filled in ([spec 110](../../../specs/110_agents.md)) |
//! | `GET /status` | `{"status": "loaded"}`, or `"failed"` with its `problems`, as JSON, so an agent can check its change took |
//! | `GET /_genesis/*` | the renderer's bundle (`trunk build` output) |
//! | `GET /api/config` | server-sent events: `config` with the loaded app as JSON, or `problems` with each one's `app.toml:LINE:COLUMN`; and `push`, `{"name", "payload"}`, for each push |
//! | `POST /api/call` | makes a call to one of the app's operations: `{"op", "params", "headers"}` in, `{"status", "body"}` or `{"error"}` out |
//! | `POST /api/call/files` | the same, as `multipart/form-data`: the call as JSON in a `call` part, and each file a part named by its reference's ID |
//! | `GET /api/log` | the log, as plain text, for agents with the agent token; `?since=TIME` and `?tail=N` narrow it |
//! | `POST /api/push/{name}` | fires `push.{name}` in every open client, with the JSON body as its payload, for agents with the agent token; answers `{"delivered": N}` |
//! | `POST /api/mirror` | a client's state and trail entries, written to the state folder |
//! | `/fs/*` | the files service (dufs), for agents with `Authorization: Bearer GENESIS_AGENT_TOKEN` |
//! | `GET /health` | `200` when the server is up |
//!
//! Every failure goes into one log, `genesis.log` in the logs folder ([spec 110](../../../specs/110_agents.md)):
//! each load's problems (and `load ok` once it loads clean again), each failed call, and each new entry
//! in a client's mirrored `sys.errors`, one line each, with the time. It keeps the app's last `[app] log_lines` lines.
//!
//! The mirror ([spec 050](../../../specs/050_state.md), "The mirror") turns names from the outside
//! world into file paths, so client names must be plain (letters, digits, `_`, `-`), or the
//! request is refused before anything touches the disk. Each client's state is written whole
//! (to a temporary file, then renamed), and its events are appended as JSON lines, keeping the last
//! [`EVENTS_KEPT`].

use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures_util::stream::{self, StreamExt};
use genesis_core::config::{App, Method, Operation};
use genesis_core::template;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;

mod log;
pub use log::FILE as LOG_FILE;

/// How many events a client's `.events.ndjson` keeps, newest last.
pub const EVENTS_KEPT: usize = 10_000;

/// Where serve finds things, and where it listens.
#[derive(Clone, PartialEq)]
pub struct Settings {
    /// The configs folder: the app, with `app.toml` at its top.
    pub configs: PathBuf,
    /// The state folder, where the mirror is written.
    pub state: PathBuf,
    /// The logs folder, where `genesis.log` is written ([spec 110](../../../specs/110_agents.md)).
    pub logs: PathBuf,
    /// The renderer's bundle.
    pub dist: PathBuf,
    /// The address to listen on.
    pub listen: String,
    /// How long the configs folder must be quiet after a change before the app reloads.
    pub debounce: Duration,
    /// The credentials connections may use, by name ([spec 070](../../../specs/070_secrets.md)).
    pub creds: Creds,
    /// Where the files service (dufs) answers, for `/fs/`. Without one, there's no `/fs/`.
    pub files_url: Option<String>,
    /// The token agents send for `/fs/`, and the password serve gives dufs. Without one, `/fs/` stays shut.
    pub agent_token: Option<String>,
}

// By hand, so the agent token never prints: only whether there is one.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("configs", &self.configs)
            .field("state", &self.state)
            .field("logs", &self.logs)
            .field("dist", &self.dist)
            .field("listen", &self.listen)
            .field("debounce", &self.debounce)
            .field("creds", &self.creds)
            .field("files_url", &self.files_url)
            .field("agent_token", &self.agent_token.as_ref().map(|_| "[set]"))
            .finish()
    }
}

/// Credentials, by name: `GENESIS_CREDS_PF_TOKEN` in serve's environment is `{{ creds.PF_TOKEN }}`.
/// Printing them shows their names, never their values.
#[derive(Clone, Default, PartialEq)]
pub struct Creds(BTreeMap<String, String>);

impl std::fmt::Debug for Creds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Creds({:?})", self.names())
    }
}

impl From<BTreeMap<String, String>> for Creds {
    fn from(creds: BTreeMap<String, String>) -> Creds {
        Creds(creds)
    }
}

impl Creds {
    /// The `GENESIS_CREDS_*` variables among `vars` (as `std::env::vars()` gives them).
    pub fn from_vars(vars: impl IntoIterator<Item = (String, String)>) -> Creds {
        let creds = vars.into_iter().filter_map(|(key, value)| {
            let name = key.strip_prefix("GENESIS_CREDS_")?;
            (!name.is_empty()).then(|| (name.to_string(), value))
        });
        Creds(creds.collect())
    }

    /// The credentials' names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    /// `text` with every credential's value replaced by `[redacted]`.
    fn redact(&self, text: &str) -> String {
        self.0
            .values()
            .filter(|v| !v.is_empty())
            .fold(text.to_string(), |text, value| {
                text.replace(value.as_str(), "[redacted]")
            })
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            configs: "configs".into(),
            state: "state".into(),
            logs: "logs".into(),
            dist: "apps/web/dist".into(),
            listen: "127.0.0.1:8080".into(),
            debounce: Duration::from_millis(150),
            creds: Creds::default(),
            files_url: None,
            agent_token: None,
        }
    }
}

impl Settings {
    /// These settings with what serve's environment says (`std::env::vars()`): the credentials
    /// (`GENESIS_CREDS_*`), the files service (`GENESIS_FILES_URL`), and the agent token
    /// (`GENESIS_AGENT_TOKEN`).
    pub fn with_env(self, vars: impl IntoIterator<Item = (String, String)>) -> Settings {
        let vars: Vec<(String, String)> = vars.into_iter().collect();
        let var = |name: &str| {
            vars.iter()
                .find(|(k, v)| k == name && !v.is_empty())
                .map(|(_, v)| v.clone())
        };
        Settings {
            files_url: var("GENESIS_FILES_URL"),
            agent_token: var("GENESIS_AGENT_TOKEN"),
            creds: Creds::from_vars(vars.clone()),
            ..self
        }
    }

    /// Reads `--configs DIR`, `--state DIR`, `--logs DIR`, `--dist DIR`, `--listen ADDRESS`, and `--debounce MS`,
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
                "--logs" => settings.logs = value()?.into(),
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
                        "unknown option {flag}; serve takes --configs, --state, --logs, --dist, --listen, and --debounce"
                    ));
                }
            }
        }
        Ok(settings)
    }
}

/// What serve's routes share: the settings, and the news that the app's files changed.
struct Server {
    settings: Settings,
    /// Makes every call, reusing connections.
    http: reqwest::Client,
    /// Sent once each time the configs folder changes and goes quiet again.
    changes: broadcast::Sender<()>,
    /// Every failure, one line each.
    log: log::Log,
    /// Each push, as the `push` event's data, for every open config stream.
    pushes: broadcast::Sender<String>,
    /// Held while a mirror is compared and written, so no error is logged twice.
    mirroring: tokio::sync::Mutex<()>,
}

impl Server {
    fn new(settings: Settings, changes: usize) -> Server {
        Server {
            log: log::Log::new(&settings.logs),
            mirroring: tokio::sync::Mutex::new(()),
            pushes: broadcast::channel(64).0,
            settings,
            http: reqwest::Client::new(),
            changes: broadcast::channel(changes).0,
        }
    }

    /// Logs each of a client's `sys.errors` that the mirror it last wrote didn't hold, oldest
    /// first, as a `run` line.
    async fn log_new_errors(&self, dir: &Path, client: &str, state: &Value) {
        let errors = |state: &Value| -> Vec<Value> {
            state["sys"]["errors"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        };
        let old = tokio::fs::read_to_string(dir.join(format!("{client}.json")))
            .await
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|old| errors(&old))
            .unwrap_or_default();
        // sys.errors is newest first.
        for error in errors(state).iter().rev().filter(|e| !old.contains(e)) {
            let line = format!(
                "client {client}: {}: {}",
                as_text(&error["name"]),
                as_text(&error["message"])
            );
            self.log
                .write("run", &self.settings.creds.redact(&line))
                .await;
        }
    }

    /// Logs a failed call, with every credential's value redacted.
    async fn log_call(&self, line: &str) {
        self.log
            .write("call", &self.settings.creds.redact(line))
            .await;
    }

    /// Loads the app and logs how it went.
    async fn load_and_log(&self) {
        match load(&self.settings.configs).await {
            Ok(app) => {
                self.log.keep(app.app.log_lines);
                self.log.load(None).await;
            }
            Err(problems) => self.log.load(Some(&problems)).await,
        }
    }
}

/// Keeps the configs folder watched for as long as it's held. Dropping it stops hot reload.
pub struct Watching(#[allow(dead_code)] notify::RecommendedWatcher);

/// serve's routes, with hot reload ([spec 060](../../../specs/060_hot_reload.md)): the configs
/// folder is watched, and once it's been quiet for `settings.debounce` after a change, every open
/// config stream gets the app's new config, or its problems.
///
/// Must be called inside a Tokio runtime. Fails if the configs folder can't be watched.
pub fn live_router(mut settings: Settings) -> Result<(Router, Watching), String> {
    settings.configs = settings
        .configs
        .canonicalize()
        .map_err(|e| format!("configs folder {}: {e}", settings.configs.display()))?;
    let (configs, debounce) = (settings.configs.clone(), settings.debounce);
    let server = Arc::new(Server::new(settings, 64));
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
    let loader = server.clone();
    tokio::spawn(async move {
        // The log says how the app loaded at start, and after each change.
        loader.load_and_log().await;
        while changed.recv().await.is_some() {
            // Editors and file servers save in bursts: wait for the folder to go quiet.
            loop {
                match tokio::time::timeout(debounce, changed.recv()).await {
                    Ok(Some(_)) => {}
                    Ok(None) => return,
                    Err(_quiet) => break,
                }
            }
            loader.load_and_log().await;
            let _ = loader.changes.send(());
        }
    });
    Ok((routes(server), Watching(watcher)))
}

/// serve's routes, without hot reload: each config stream sends the app once.
pub fn router(settings: Settings) -> Router {
    routes(Arc::new(Server::new(settings, 1)))
}

fn routes(server: Arc<Server>) -> Router {
    let bundle = ServeDir::new(&server.settings.dist);
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/", get(page))
        .route("/status", get(status))
        .route("/agents", get(agents))
        .route("/api/config", get(config))
        .route("/api/mirror", post(mirror))
        .route("/api/call", post(call))
        .route("/api/log", get(read_log))
        .route("/api/push/{name}", post(push))
        // Files can be big; how big is the app's call (each `file` control's `max_size`).
        .route(
            "/api/call/files",
            post(call_with_files).layer(DefaultBodyLimit::disable()),
        )
        .route("/fs", any(files))
        // `/fs/{*rest}` needs something after the slash, and the folder root is what agents list first.
        .route("/fs/", any(files))
        .route("/fs/{*rest}", any(files))
        .nest_service("/_genesis", bundle)
        .with_state(server)
}

type Shared = State<Arc<Server>>;

fn refuse(status: StatusCode, why: String) -> Response {
    (status, why).into_response()
}

/// Letters, digits, `_`, and `-` only, so a name can never leave the folder it's joined to.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The guide for agents: `AGENTS.md` from the top of the configs folder, with each `BASE` as a
/// word of its own replaced by the address the request came to. Open, since an agent reads it
/// before it has a token.
async fn agents(State(server): Shared, headers: axum::http::HeaderMap) -> Response {
    let guide = match tokio::fs::read_to_string(server.settings.configs.join("AGENTS.md")).await {
        Ok(guide) => guide,
        Err(_) => {
            return refuse(
                StatusCode::NOT_FOUND,
                "No agent instructions here yet: add AGENTS.md to the top of the configs folder (a copy of docs/authoring.md).".into(),
            );
        }
    };
    let base = base(&headers, &server.settings.listen);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/markdown; charset=utf-8",
        )],
        fill_base(&guide, &base),
    )
        .into_response()
}

/// The address a request came to: the scheme and host a proxy passes on, else `http` and `Host`.
/// A host that isn't a host name (with an optional port) falls back to `listen`.
fn base(headers: &axum::http::HeaderMap, listen: &str) -> String {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let scheme = match header("x-forwarded-proto") {
        Some(p) if p.eq_ignore_ascii_case("https") => "https",
        _ => "http",
    };
    let host = header("x-forwarded-host")
        .or_else(|| header("host"))
        .filter(|h| host_name(h))
        .unwrap_or(listen);
    format!("{scheme}://{host}")
}

/// A host name or IPv4 address, with an optional port: nothing that could carry anything else.
fn host_name(host: &str) -> bool {
    let (name, port) = match host.rsplit_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (host, None),
    };
    let name_ok = !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    let port_ok = port.is_none_or(|p| !p.is_empty() && p.len() <= 5 && p.parse::<u16>().is_ok());
    name_ok && port_ok
}

/// `text` with each `BASE` that stands as a word of its own replaced by `base`.
fn fill_base(text: &str, base: &str) -> String {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut before: Option<char> = None;
    while let Some(at) = rest.find("BASE") {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let prev = head
            .chars()
            .next_back()
            .or(if head.is_empty() { before } else { None });
        let next = tail["BASE".len()..].chars().next();
        if word(prev) || word(next) {
            out.push_str("BASE");
        } else {
            out.push_str(base);
        }
        before = Some('E');
        rest = &tail["BASE".len()..];
    }
    out.push_str(rest);
    out
}

async fn status(State(server): Shared) -> Json<Value> {
    Json(match load(&server.settings.configs).await {
        Ok(_) => serde_json::json!({ "status": "loaded" }),
        Err(problems) => {
            serde_json::json!({ "status": "failed", "problems": problems.lines().collect::<Vec<_>>() })
        }
    })
}

async fn page(State(server): Shared) -> Response {
    let settings = &server.settings;
    match tokio::fs::read_to_string(settings.dist.join("index.html")).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the renderer isn't built (run trunk build in apps/web): {e}"),
        ),
    }
}

async fn config(State(server): Shared) -> Response {
    let configs = server.settings.configs.clone();
    // Listen before the first load, so a change or a push during it isn't missed.
    let news = BroadcastStream::new(server.changes.subscribe());
    let pushes = BroadcastStream::new(server.pushes.subscribe())
        .filter_map(|push| async move { push.ok() })
        .map(|push| Event::default().event("push").data(push));
    let first = {
        let configs = configs.clone();
        stream::once(async move { app_event(&configs).await })
    };
    // After the first event the stream stays open: a closed stream would make the browser
    // reconnect and ask again every few seconds. Each change sends it again; so does a lag,
    // which may have dropped a change.
    let later = news.then(move |_| {
        let configs = configs.clone();
        async move { app_event(&configs).await }
    });
    let events = first.chain(stream::select(later, pushes));
    // A comment now and then finds a closed tab, so it stops counting as a client.
    Sse::new(events.map(Ok::<_, Infallible>))
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}

/// The event the configs folder makes now: `config` with the app, or `problems`.
async fn app_event(configs: &Path) -> Event {
    match load(configs).await {
        Ok(app) => Event::default()
            .event("config")
            .data(serde_json::to_string(&app).unwrap_or_default()),
        Err(problems) => Event::default().event("problems").data(problems),
    }
}

/// Loads the app from every `.toml` file in the configs folder, in any folder, or says what's
/// wrong with it, one problem per line as `FILE:LINE:COLUMN: ...` ([spec 020](../../../specs/020_config.md)).
async fn load(configs: &Path) -> Result<App, String> {
    let configs = configs.to_path_buf();
    let files = tokio::task::spawn_blocking(move || read_configs(&configs))
        .await
        .map_err(|e| format!("reading the configs folder stopped: {e}"))??;
    App::from_files(files).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        lines.join("\n")
    })
}

/// Every `.toml` file under `dir`, as its path from `dir` (with `/` between folders) and its text.
/// Hidden files and folders (a name starting with `.`, like `.git`) aren't part of the app.
fn read_configs(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries = std::fs::read_dir(&folder)
            .map_err(|e| format!("the configs folder {}: {e}", folder.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("{}: {e}", folder.display()))?
                .path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(".");
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                folders.push(path);
            } else if name.ends_with(".toml") {
                let rel = path.strip_prefix(dir).unwrap_or(&path);
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
                files.push((rel, text));
            }
        }
    }
    Ok(files)
}

async fn mirror(State(server): Shared, Json(body): Json<Value>) -> Response {
    let settings = &server.settings;
    let client = body["client"].as_str().unwrap_or_default();
    if !plain(client) {
        return refuse(
            StatusCode::BAD_REQUEST,
            format!("{client:?} isn't a plain client name"),
        );
    }
    let dir = &settings.state;
    let events: Vec<String> = body["events"]
        .as_array()
        .map(|e| e.iter().map(Value::to_string).collect())
        .unwrap_or_default();
    let _held = server.mirroring.lock().await;
    server.log_new_errors(dir, client, &body["state"]).await;
    match write_mirror(dir, client, &body["state"], &events).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the mirror couldn't be written: {e}"),
        ),
    }
}

async fn call(State(server): Shared, Json(body): Json<Value>) -> Response {
    run_call(&server, body, &BTreeMap::new()).await
}

/// One file the browser sent with a call: its name, its type, and its bytes.
struct Upload {
    name: String,
    kind: String,
    bytes: Vec<u8>,
}

/// A call whose params hold file references: the call in a `call` part, and each file's bytes in
/// a part named by its reference's ID.
async fn call_with_files(State(server): Shared, mut parts: Multipart) -> Response {
    let mut body = Value::Null;
    let mut files = BTreeMap::new();
    loop {
        let part = match parts.next_field().await {
            Ok(Some(part)) => part,
            Ok(None) => break,
            Err(e) => {
                return refuse(
                    StatusCode::BAD_REQUEST,
                    format!("the call's files broke off: {e}"),
                );
            }
        };
        let id = part.name().unwrap_or_default().to_string();
        let name = part.file_name().unwrap_or(&id).to_string();
        let kind = part
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = match part.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
                return refuse(
                    StatusCode::BAD_REQUEST,
                    format!("the call's files broke off: {e}"),
                );
            }
        };
        if id == "call" {
            body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        } else {
            files.insert(id, Upload { name, kind, bytes });
        }
    }
    run_call(&server, body, &files).await
}

/// Makes the call `body` names, with `files` for any file references in its params.
async fn run_call(server: &Server, body: Value, files: &BTreeMap<String, Upload>) -> Response {
    let app = match load(&server.settings.configs).await {
        Ok(app) => app,
        Err(problems) => {
            return refuse(
                StatusCode::CONFLICT,
                format!("the app doesn't load:\n{problems}"),
            );
        }
    };
    let op_name = body["op"].as_str().unwrap_or_default();
    // Who made the call, and from what, for the log.
    let whose = [("client", &body["client"]), ("from", &body["from"])]
        .iter()
        .filter_map(|(what, v)| v.as_str().map(|v| format!("{what} {v}")))
        .collect::<Vec<_>>()
        .join(", ");
    let whose = match whose.is_empty() {
        true => String::new(),
        false => format!(" ({whose})"),
    };
    // Only an operation the app's own config declares can be called: the client names it, and
    // serve decides where it goes.
    let Some(op) = app.operation(op_name) else {
        let why = format!("the app declares no operation named {op_name:?}");
        server.log_call(&format!("{op_name}: {why}{whose}")).await;
        return refuse(StatusCode::NOT_FOUND, why);
    };
    let params = body["params"].as_object().cloned().unwrap_or_default();
    let sent = body["headers"].as_object().cloned().unwrap_or_default();
    let reply = match make_call(server, op_name, &op, params, &sent, files).await {
        Ok((status, answer)) => {
            if status >= 400 {
                server
                    .log_call(&format!("{op_name} {status} {answer}{whose}"))
                    .await;
            }
            serde_json::json!({ "status": status, "body": answer })
        }
        Err(why) => {
            server.log_call(&format!("{op_name}: {why}{whose}")).await;
            serde_json::json!({ "error": server.settings.creds.redact(&why) })
        }
    };
    Json(reply).into_response()
}

/// Makes one call. Fails with why, in words a person can act on (redacted by the caller).
async fn make_call(
    server: &Server,
    op_name: &str,
    op: &Operation,
    mut params: Map<String, Value>,
    sent: &Map<String, Value>,
    files: &BTreeMap<String, Upload>,
) -> Result<(u16, Value), String> {
    let url = fill_path(op_name, &op.url, &mut params)?;
    let method =
        reqwest::Method::from_bytes(op.method.as_str().as_bytes()).map_err(|e| e.to_string())?;
    let mut request = server.http.request(method, &url);
    for (header, template) in &op.headers {
        if let Some(value) = header_value(
            &server.settings.creds,
            template,
            sent.get(header).and_then(Value::as_str),
        )? {
            request = request.header(header, value);
        }
    }
    let with_files = !genesis_core::file::references(&params).is_empty();
    request = match op.method {
        // Params holding files go out as multipart, whatever the method would otherwise send.
        _ if with_files => request.multipart(multipart(op_name, params, files)?),
        Method::Get | Method::Head | Method::Delete => {
            let pairs: Vec<(String, String)> = params
                .iter()
                .map(|(k, v)| (k.clone(), as_text(v)))
                .collect();
            request.query(&pairs)
        }
        _ => request
            .header("content-type", "application/json")
            .body(Value::Object(params).to_string()),
    };
    if op.timeout > 0 {
        request = request.timeout(Duration::from_secs(op.timeout));
    }
    let response = request.send().await.map_err(|e| match e.is_timeout() {
        true => format!(
            "the call to {op_name} timed out after {} seconds",
            op.timeout
        ),
        false => format!("the call to {op_name} failed: {e}"),
    })?;
    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("the reply to {op_name} broke off: {e}"))?;
    if bytes.is_empty() {
        return Ok((status, Value::Null));
    }
    match serde_json::from_slice(&bytes) {
        Ok(body) => Ok((status, body)),
        Err(_) if (200..300).contains(&status) => Err(format!("the reply to {op_name} isn't JSON")),
        Err(_) => Ok((
            status,
            Value::String(String::from_utf8_lossy(&bytes).into_owned()),
        )),
    }
}

/// Params as `multipart/form-data`: each file reference as a part with its bytes, name, and type
/// (a list of them as one part each, under the same name), and every other param as a text field.
fn multipart(
    op_name: &str,
    params: Map<String, Value>,
    files: &BTreeMap<String, Upload>,
) -> Result<reqwest::multipart::Form, String> {
    let part = |value: &Value| -> Result<Option<reqwest::multipart::Part>, String> {
        let Some(id) = genesis_core::file::reference(value) else {
            return Ok(None);
        };
        let upload = files.get(id).ok_or_else(|| {
            format!(
                "{op_name} sends {}, but its file is gone (a file is held only while its page is open); pick it again",
                as_text(&value["name"])
            )
        })?;
        let part = reqwest::multipart::Part::bytes(upload.bytes.clone())
            .file_name(upload.name.clone())
            .mime_str(&upload.kind)
            .map_err(|e| format!("{op_name}: {} has a bad type: {e}", upload.name))?;
        Ok(Some(part))
    };
    let mut form = reqwest::multipart::Form::new();
    for (key, value) in params {
        let items = match &value {
            Value::Array(items)
                if items
                    .iter()
                    .any(|v| genesis_core::file::reference(v).is_some()) =>
            {
                items.clone()
            }
            _ => vec![value.clone()],
        };
        for item in &items {
            form = match part(item)? {
                Some(file) => form.part(key.clone(), file),
                None => form.text(key.clone(), as_text(item)),
            };
        }
    }
    Ok(form)
}

/// The URL with each `{name}` replaced by the param `name`, URL-encoded, and taken out of the params.
fn fill_path(op_name: &str, url: &str, params: &mut Map<String, Value>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = url;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + 1..start + len];
        let value = params
            .remove(name)
            .ok_or_else(|| format!("{op_name} needs the param {name}"))?;
        out.push_str(&rest[..start]);
        out.extend(percent_encoding::utf8_percent_encode(
            &as_text(&value),
            percent_encoding::NON_ALPHANUMERIC,
        ));
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// What to send for one declared header. A header whose template names `app.*` or `sys.*` takes
/// the client's filled-in value, and isn't sent if the client sent none. Credentials are filled
/// only where the config's own template names them; any other `{{ creds.* }}` stays as text.
fn header_value(
    creds: &Creds,
    template: &str,
    sent: Option<&str>,
) -> Result<Option<String>, String> {
    let named = template::paths(template);
    let allowed: Vec<&str> = named
        .iter()
        .filter_map(|p| p.strip_prefix("creds."))
        .collect();
    let from_client = named.iter().any(|p| !p.starts_with("creds."));
    let base = match (from_client, sent) {
        (false, _) => template,
        (true, Some(sent)) => sent,
        (true, None) => return Ok(None),
    };
    for name in &allowed {
        if !creds.0.contains_key(*name) {
            return Err(format!(
                "credential {name} isn't set (GENESIS_CREDS_{name})"
            ));
        }
    }
    let fill = |path: &str| match path.strip_prefix("creds.") {
        Some(name) if allowed.contains(&name) => {
            creds.0.get(name).map(|v| Value::String(v.clone()))
        }
        _ => Some(Value::String(format!("{{{{ {path} }}}}"))),
    };
    Ok(Some(template::text(base, fill)))
}

/// A param as text: a string as it is, anything else as JSON.
fn as_text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), String::from)
}

/// The headers serve never passes between a client and the files service: its own login, and the
/// connection's own.
const NOT_PASSED: [&str; 5] = [
    "authorization",
    "host",
    "content-length",
    "connection",
    "transfer-encoding",
];

/// Passes a request through to the files service, for an agent holding the agent token, with
/// dufs's own login added (as PurpleFlow does, PurpleFlow spec 090).
async fn files(State(server): Shared, request: axum::extract::Request) -> Response {
    let settings = &server.settings;
    let Some(files_url) = &settings.files_url else {
        return refuse(
            StatusCode::NOT_FOUND,
            "this serve has no files service".into(),
        );
    };
    let Some(token) = agent(settings, request.headers()) else {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "/fs/ needs Authorization: Bearer GENESIS_AGENT_TOKEN".into(),
        );
    };
    let (parts, body) = request.into_parts();
    let path = parts.uri.path_and_query().map_or("/fs", |p| p.as_str());
    let url = format!("{}{path}", files_url.trim_end_matches('/'));
    let body = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(body) => body,
        Err(e) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                format!("couldn't read the request: {e}"),
            );
        }
    };
    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut onward = server
        .http
        .request(method, &url)
        .basic_auth("genesis", Some(token))
        .timeout(Duration::from_secs(60))
        .body(body);
    for (name, value) in parts
        .headers
        .iter()
        .filter(|(n, _)| !NOT_PASSED.contains(&n.as_str()))
    {
        onward = onward.header(name.as_str(), value.as_bytes());
    }
    let answer = match onward.send().await {
        Ok(answer) => answer,
        Err(_) => {
            return refuse(
                StatusCode::BAD_GATEWAY,
                "the files service didn't answer".into(),
            );
        }
    };
    let mut response = Response::builder().status(answer.status().as_u16());
    for (name, value) in answer
        .headers()
        .iter()
        .filter(|(n, _)| !NOT_PASSED.contains(&n.as_str()))
    {
        response = response.header(name.as_str(), value.as_bytes());
    }
    let body = answer.bytes().await.unwrap_or_default();
    response
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| {
            refuse(
                StatusCode::BAD_GATEWAY,
                "the files service's answer didn't read".into(),
            )
        })
}

/// The agent token, when `headers` carry it as `Authorization: Bearer TOKEN`. `None` when they
/// don't, or serve has none.
fn agent<'a>(settings: &'a Settings, headers: &axum::http::HeaderMap) -> Option<&'a str> {
    let token = settings.agent_token.as_deref().filter(|t| !t.is_empty());
    let sent = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    token.filter(|t| sent.is_some_and(|s| same(s, t)))
}

/// The log, for an agent holding the agent token: from `since` on, and the last `tail` lines.
async fn read_log(
    State(server): Shared,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<BTreeMap<String, String>>,
) -> Response {
    if agent(&server.settings, &headers).is_none() {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "/api/log needs Authorization: Bearer GENESIS_AGENT_TOKEN".into(),
        );
    }
    let since = query.get("since");
    if let Some(since) = since.filter(|s| !log::is_time(s)) {
        return refuse(
            StatusCode::BAD_REQUEST,
            format!("since takes a time like 2026-11-24T10:00:00Z, not {since:?}"),
        );
    }
    let tail = match query.get("tail").map(|t| t.parse::<usize>()) {
        None => None,
        Some(Ok(tail)) => Some(tail),
        Some(Err(_)) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                format!(
                    "tail takes a whole number of lines, not {:?}",
                    query["tail"]
                ),
            );
        }
    };
    let text = match server.log.read().await {
        Ok(text) => text,
        Err(e) => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("the log couldn't be read: {e}"),
            );
        }
    };
    // Each line starts with its time, in a form that sorts as text.
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| {
            since.is_none_or(|since| line.get(..20).is_some_and(|t| t >= since.as_str()))
        })
        .collect();
    let lines = &lines[lines.len().saturating_sub(tail.unwrap_or(usize::MAX))..];
    let text = lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        text,
    )
        .into_response()
}

/// Fires `push.{name}` in every open client, with the JSON body as its payload.
async fn push(
    State(server): Shared,
    axum::extract::Path(name): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let refused = |status: StatusCode, why: String| {
        let server = server.clone();
        let name = name.clone();
        async move {
            server
                .log
                .write("push", &format!("{name}: {} {why}", status.as_u16()))
                .await;
            refuse(status, why)
        }
    };
    if agent(&server.settings, &headers).is_none() {
        return refused(
            StatusCode::UNAUTHORIZED,
            "/api/push needs Authorization: Bearer GENESIS_AGENT_TOKEN".into(),
        )
        .await;
    }
    let named = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !named {
        return refused(
            StatusCode::BAD_REQUEST,
            "a push's name is lowercase letters, digits, and _".into(),
        )
        .await;
    }
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload @ Value::Object(_)) => payload,
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "the body isn't a JSON object".into(),
            )
            .await;
        }
    };
    let push = serde_json::json!({ "name": name, "payload": payload }).to_string();
    // Sending fails only when no one is listening: no screen is open, which isn't an error.
    let delivered = server.pushes.send(push).unwrap_or(0);
    Json(serde_json::json!({ "delivered": delivered })).into_response()
}

/// Whether two tokens are the same, taking as long whatever their first difference, so timing
/// doesn't give a token away a character at a time.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (x, y)| diff | (x ^ y))
            == 0
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
