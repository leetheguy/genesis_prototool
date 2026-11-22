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
//! | `POST /api/{app}/call` | makes a call to one of the app's operations: `{"op", "params", "headers"}` in, `{"status", "body"}` or `{"error"}` out |
//! | `POST /api/{app}/mirror` | a client's state and trail entries, written to the state folder |
//! | `/fs/*` | the files service (dufs), for agents with `Authorization: Bearer GENESIS_AGENT_TOKEN` |
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
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures_util::stream::{self, StreamExt};
use genesis_core::config::{App, Method, Operation};
use genesis_core::template;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
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
#[derive(Clone, PartialEq)]
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
    /// Makes every call, reusing connections.
    http: reqwest::Client,
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
        http: reqwest::Client::new(),
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
        http: reqwest::Client::new(),
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
        .route("/api/{app}/call", post(call))
        .route("/fs", any(files))
        .route("/fs/{*rest}", any(files))
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

async fn call(
    State(server): Shared,
    UrlPath(name): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    let dir = match app_dir(&server.settings, &name) {
        Ok(dir) => dir,
        Err(refused) => return refused.into_response(),
    };
    let app = match load(&dir.join("app.toml")).await {
        Ok(app) => app,
        Err(problems) => {
            return refuse(
                StatusCode::CONFLICT,
                format!("the app doesn't load:\n{problems}"),
            );
        }
    };
    let op_name = body["op"].as_str().unwrap_or_default();
    // Only an operation the app's own config declares can be called: the client names it, and
    // serve decides where it goes.
    let Some(op) = app.operation(op_name) else {
        return refuse(
            StatusCode::NOT_FOUND,
            format!("{name} declares no operation named {op_name:?}"),
        );
    };
    let params = body["params"].as_object().cloned().unwrap_or_default();
    let sent = body["headers"].as_object().cloned().unwrap_or_default();
    let reply = match make_call(&server, op_name, &op, params, &sent).await {
        Ok((status, body)) => serde_json::json!({ "status": status, "body": body }),
        Err(why) => serde_json::json!({ "error": server.settings.creds.redact(&why) }),
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
    request = match op.method {
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
    let token = settings.agent_token.as_deref().filter(|t| !t.is_empty());
    let sent = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token.filter(|t| sent.is_some_and(|s| same(s, t))) else {
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
