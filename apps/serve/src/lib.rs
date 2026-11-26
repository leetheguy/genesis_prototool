//! `genesis-serve`: hands out the renderer and the app's config, makes its calls, and writes the mirror.
//!
//! The configs folder is the app, with `app.toml` at its top ([spec 120](../../../specs/120_one_app.md),
//! [spec 010](../../../specs/010_architecture.md)). The routes:
//!
//! | Route | What |
//! |---|---|
//! | `GET /` | the renderer's page, drawing the app |
//! | `GET /status` | `{"status": "loaded"}`, or `"failed"` with its `problems`, as JSON, so an agent can check its change took |
//! | `GET /_genesis/*` | the renderer's bundle (`trunk build` output) |
//! | `GET /api/config` | server-sent events: `config` with the loaded app as JSON, or `problems` with each one's `app.toml:LINE:COLUMN` |
//! | `POST /api/call` | makes a call to one of the app's operations: `{"op", "params", "headers"}` in, `{"status", "body"}` or `{"error"}` out |
//! | `POST /api/call/files` | the same, as `multipart/form-data`: the call as JSON in a `call` part, and each file a part named by its reference's ID |
//! | `POST /api/mirror` | a client's state and trail entries, written to the state folder |
//! | `/fs/*` | the files service (dufs), for agents with `Authorization: Bearer GENESIS_AGENT_TOKEN` |
//! | `GET /health` | `200` when the server is up |
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

/// How many events a client's `.events.ndjson` keeps, newest last.
pub const EVENTS_KEPT: usize = 10_000;

/// Where serve finds things, and where it listens.
#[derive(Clone, PartialEq)]
pub struct Settings {
    /// The configs folder: the app, with `app.toml` at its top.
    pub configs: PathBuf,
    /// The state folder, where the mirror is written.
    pub state: PathBuf,
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

/// What serve's routes share: the settings, and the news that the app's files changed.
struct Server {
    settings: Settings,
    /// Makes every call, reusing connections.
    http: reqwest::Client,
    /// Sent once each time the configs folder changes and goes quiet again.
    changes: broadcast::Sender<()>,
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
        while changed.recv().await.is_some() {
            // Editors and file servers save in bursts: wait for the folder to go quiet.
            loop {
                match tokio::time::timeout(debounce, changed.recv()).await {
                    Ok(Some(_)) => {}
                    Ok(None) => return,
                    Err(_quiet) => break,
                }
            }
            let _ = changes.send(());
        }
    });
    Ok((routes(server), Watching(watcher)))
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
        .route("/", get(page))
        .route("/status", get(status))
        .route("/api/config", get(config))
        .route("/api/mirror", post(mirror))
        .route("/api/call", post(call))
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
    // Listen before the first load, so a change during it isn't missed.
    let news = BroadcastStream::new(server.changes.subscribe());
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
    Sse::new(first.chain(later).map(Ok::<_, Infallible>)).into_response()
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
    // Only an operation the app's own config declares can be called: the client names it, and
    // serve decides where it goes.
    let Some(op) = app.operation(op_name) else {
        return refuse(
            StatusCode::NOT_FOUND,
            format!("the app declares no operation named {op_name:?}"),
        );
    };
    let params = body["params"].as_object().cloned().unwrap_or_default();
    let sent = body["headers"].as_object().cloned().unwrap_or_default();
    let reply = match make_call(server, op_name, &op, params, &sent, files).await {
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
