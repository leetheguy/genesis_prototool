//! serve's routes (spec 010, "Routes on serve"): the renderer, the config stream, and the mirror.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use genesis_serve::{Settings, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

/// A fresh configs, state, and dist folder, removed when dropped.
struct Folders(PathBuf);

impl Folders {
    fn new(name: &str) -> Folders {
        let root =
            std::env::temp_dir().join(format!("genesis-serve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["configs", "state", "dist"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("configs/app.toml"), "[app]\nname = \"Hello\"\n").unwrap();
        std::fs::write(root.join("dist/index.html"), "<html>the renderer</html>").unwrap();
        std::fs::write(root.join("dist/genesis.js"), "// bundle").unwrap();
        Folders(root)
    }

    fn settings(&self) -> Settings {
        Settings {
            configs: self.0.join("configs"),
            state: self.0.join("state"),
            logs: self.0.join("logs"),
            dist: self.0.join("dist"),
            ..Settings::default()
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn get(folders: &Folders, uri: &str) -> (StatusCode, String) {
    let response = router(folders.settings())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// The first chunk of a stream that stays open.
async fn first_event(folders: &Folders, uri: &str) -> (StatusCode, String) {
    let response = router(folders.settings())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    (status, String::from_utf8_lossy(&frame).into_owned())
}

async fn post(folders: &Folders, uri: &str, body: Value) -> StatusCode {
    let request = Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    router(folders.settings())
        .oneshot(request)
        .await
        .unwrap()
        .status()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[tokio::test]
async fn health_answers_ok() {
    let folders = Folders::new("health");
    assert_eq!(get(&folders, "/health").await.0, StatusCode::OK);
}

#[tokio::test]
async fn the_root_is_the_renderer() {
    let folders = Folders::new("page");
    assert_eq!(
        get(&folders, "/").await,
        (StatusCode::OK, "<html>the renderer</html>".into())
    );
}

#[tokio::test]
async fn with_no_app_toml_the_stream_says_so() {
    let folders = Folders::new("noapp");
    std::fs::remove_file(folders.path("configs/app.toml")).unwrap();
    let (status, event) = first_event(&folders, "/api/config").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        event.starts_with("event: problems\n") && event.contains("app.toml not found"),
        "{event}"
    );
}

#[tokio::test]
async fn the_bundle_is_served_under_genesis() {
    let folders = Folders::new("bundle");
    assert_eq!(
        get(&folders, "/_genesis/genesis.js").await,
        (StatusCode::OK, "// bundle".into())
    );
}

#[tokio::test]
async fn the_config_stream_starts_with_the_loaded_app() {
    let folders = Folders::new("config");
    let (status, event) = first_event(&folders, "/api/config").await;
    assert_eq!(status, StatusCode::OK);
    assert!(event.starts_with("event: config\n"), "{event}");
    let data: Value = serde_json::from_str(
        event
            .lines()
            .nth(1)
            .unwrap()
            .strip_prefix("data: ")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(data["app"]["name"], "Hello");
}

#[tokio::test]
async fn an_app_that_doesnt_load_streams_its_problems_with_where_they_are() {
    let folders = Folders::new("problems");
    std::fs::write(
        folders.path("configs/app.toml"),
        "[app]\nname = \"Hello\"\ncolour = \"red\"\n",
    )
    .unwrap();
    let (_, event) = first_event(&folders, "/api/config").await;
    assert!(event.starts_with("event: problems\n"), "{event}");
    assert!(
        event.contains("app.toml:3:1") && event.contains("colour"),
        "{event}"
    );
}

#[tokio::test]
async fn the_mirror_writes_the_clients_state_whole() {
    let folders = Folders::new("mirror");
    let status = post(
        &folders,
        "/api/mirror",
        json!({ "client": "abc123", "state": { "app": { "n": 1 } }, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let state: Value = serde_json::from_str(&read(&folders.path("state/abc123.json"))).unwrap();
    assert_eq!(state, json!({ "app": { "n": 1 } }));
}

#[tokio::test]
async fn the_mirror_appends_each_event_as_a_line() {
    let folders = Folders::new("events");
    post(
        &folders,
        "/api/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "a" }] }),
    )
    .await;
    post(
        &folders,
        "/api/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "b" }, { "log": "x" }] }),
    )
    .await;
    let lines: Vec<Value> = read(&folders.path("state/c1.events.ndjson"))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        [
            json!({ "name": "a" }),
            json!({ "name": "b" }),
            json!({ "log": "x" })
        ]
    );
}

#[tokio::test]
async fn the_mirrors_event_file_keeps_the_last_ten_thousand() {
    let folders = Folders::new("trim");
    let events: Vec<Value> = (0..10_005).map(|i| json!({ "n": i })).collect();
    post(
        &folders,
        "/api/mirror",
        json!({ "client": "c1", "state": {}, "events": events }),
    )
    .await;
    let text = read(&folders.path("state/c1.events.ndjson"));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 10_000);
    assert_eq!(lines[0], r#"{"n":5}"#);
}

#[tokio::test]
async fn a_client_name_that_isnt_plain_is_refused_and_nothing_is_written() {
    let folders = Folders::new("traversal");
    for client in ["../escape", "a/b", "", "dot.dot"] {
        let status = post(
            &folders,
            "/api/mirror",
            json!({ "client": client, "state": {}, "events": [] }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{client:?}");
    }
    assert!(
        std::fs::read_dir(folders.path("state"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn settings_listen_on_localhost_unless_told_otherwise() {
    assert_eq!(
        Settings::from_args(Vec::<String>::new()).unwrap().listen,
        "127.0.0.1:8080"
    );
    let s = Settings::from_args(
        [
            "--listen",
            "0.0.0.0:9000",
            "--configs",
            "/c",
            "--state",
            "/s",
            "--dist",
            "/d",
            "--logs",
            "/l",
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(
        (s.listen.as_str(), s.configs, s.state, s.dist, s.logs),
        (
            "0.0.0.0:9000",
            "/c".into(),
            "/s".into(),
            "/d".into(),
            "/l".into()
        )
    );
    assert_eq!(Settings::default().logs, std::path::PathBuf::from("logs"));
    assert!(Settings::from_args(["--bogus".to_string()]).is_err());
}

#[tokio::test]
async fn names_may_hold_underscores_and_dashes() {
    let folders = Folders::new("plain");
    let status = post(
        &folders,
        "/api/mirror",
        json!({ "client": "my_client-1", "state": {}, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_event_file_that_cant_be_read_fails_loud_and_is_left_alone() {
    let folders = Folders::new("unreadable");
    std::fs::write(folders.path("state/c1.events.ndjson"), [0xff, 0xfe, 0x00]).unwrap();
    let status = post(
        &folders,
        "/api/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "a" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let untouched = std::fs::read(folders.path("state/c1.events.ndjson")).unwrap();
    assert_eq!(untouched, [0xff, 0xfe, 0x00]);
}

#[tokio::test]
async fn status_says_the_app_loaded() {
    let folders = Folders::new("status");
    let (status, body) = get(&folders, "/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({ "status": "loaded" })
    );
}

#[tokio::test]
async fn status_says_the_app_failed_with_where_each_problem_is() {
    let folders = Folders::new("statusbad");
    std::fs::write(
        folders.path("configs/app.toml"),
        "[app]\nname = \"x\"\ncolour = 1\n",
    )
    .unwrap();
    let (_, body) = get(&folders, "/status").await;
    let report: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(report["status"], "failed");
    assert!(
        report["problems"][0]
            .as_str()
            .unwrap()
            .starts_with("app.toml:3:1:"),
        "{report}"
    );
}

#[tokio::test]
async fn the_app_is_every_toml_file_in_every_folder() {
    let folders = Folders::new("manyfiles");
    std::fs::create_dir_all(folders.path("configs/crm/deep")).unwrap();
    std::fs::write(
        folders.path("configs/crm/deep/more.toml"),
        "[state]\npage = 3\n",
    )
    .unwrap();
    std::fs::write(folders.path("configs/notes.md"), "not toml, not read").unwrap();
    let (_, event) = first_event(&folders, "/api/config").await;
    let data: Value = serde_json::from_str(
        event
            .lines()
            .nth(1)
            .unwrap()
            .strip_prefix("data: ")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(data["state"]["page"], 3);
}

#[tokio::test]
async fn a_problem_in_a_folder_names_the_files_path() {
    let folders = Folders::new("deepproblem");
    std::fs::create_dir_all(folders.path("configs/crm")).unwrap();
    std::fs::write(
        folders.path("configs/crm/api.toml"),
        "[state]\nx = 1\ny = \n",
    )
    .unwrap();
    let (_, body) = get(&folders, "/status").await;
    let report: Value = serde_json::from_str(&body).unwrap();
    assert!(
        report["problems"][0]
            .as_str()
            .unwrap()
            .starts_with("crm/api.toml:3:"),
        "{report}"
    );
}

#[tokio::test]
async fn hidden_files_and_folders_arent_part_of_the_app() {
    let folders = Folders::new("hidden");
    std::fs::create_dir_all(folders.path("configs/.git")).unwrap();
    std::fs::write(folders.path("configs/.git/config.toml"), "not = [valid").unwrap();
    std::fs::write(folders.path("configs/.draft.toml"), "not = [valid").unwrap();
    let (_, body) = get(&folders, "/status").await;
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({ "status": "loaded" })
    );
}

/// `GET /agents` with the headers given, as (status, content type, body).
async fn agents(folders: &Folders, headers: &[(&str, &str)]) -> (StatusCode, String, String) {
    let mut request = Request::get("/agents");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router(Settings {
        listen: "127.0.0.1:8080".into(),
        ..folders.settings()
    })
    .oneshot(request.body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let kind = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, kind, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn agents_answers_the_guide_with_base_filled_in_and_needs_no_token() {
    let folders = Folders::new("agents-guide");
    std::fs::write(
        folders.path("configs/AGENTS.md"),
        "Read BASE/status, then BASE/fs/.\nBASE_URL, DATABASE and BASEline stay. (BASE)\n",
    )
    .unwrap();
    let (status, kind, body) = agents(&folders, &[("host", "my-domain.com")]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kind, "text/markdown; charset=utf-8");
    assert_eq!(
        body,
        "Read http://my-domain.com/status, then http://my-domain.com/fs/.\n\
         BASE_URL, DATABASE and BASEline stay. (http://my-domain.com)\n"
    );
}

#[tokio::test]
async fn agents_without_agents_md_is_a_404_saying_to_add_one() {
    let folders = Folders::new("agents-missing");
    let (status, _, body) = agents(&folders, &[("host", "my-domain.com")]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("add AGENTS.md"), "{body}");
}

#[tokio::test]
async fn base_takes_the_scheme_and_host_a_proxy_passes_on_and_only_a_host_name() {
    let folders = Folders::new("agents-base");
    std::fs::write(folders.path("configs/AGENTS.md"), "BASE").unwrap();
    let cases: &[(&[(&str, &str)], &str)] = &[
        (
            &[
                ("host", "serve:8080"),
                ("x-forwarded-proto", "https"),
                ("x-forwarded-host", "my-domain.com"),
            ],
            "https://my-domain.com",
        ),
        (
            &[
                ("host", "my-domain.com:8443"),
                ("x-forwarded-proto", "https, http"),
            ],
            "https://my-domain.com:8443",
        ),
        (
            &[("host", "localhost:3000"), ("x-forwarded-proto", "gopher")],
            "http://localhost:3000",
        ),
        (&[("host", "evil.com/<script>")], "http://127.0.0.1:8080"),
        (&[("host", "a b")], "http://127.0.0.1:8080"),
        (&[("host", "my-domain.com:99999")], "http://127.0.0.1:8080"),
        (&[], "http://127.0.0.1:8080"),
    ];
    for (headers, want) in cases {
        let (_, _, body) = agents(&folders, headers).await;
        assert_eq!(&body, want, "{headers:?}");
    }
}

#[tokio::test]
async fn a_run_failure_a_client_reports_through_the_mirror_is_logged_once() {
    let folders = Folders::new("log-run");
    let first = json!({ "name": "save_btn.click", "message": "app.nope isn't declared", "at": 10 });
    let second = json!({ "name": "chain.too_deep", "message": "a chain of events went past max_chain", "at": 20 });
    let mirror = |errors: Value| json!({ "client": "c7f3a", "state": { "app": {}, "sys": { "errors": errors } }, "events": [] });
    let log = || {
        std::fs::read_to_string(folders.path("logs/genesis.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| line.split_once(' ').unwrap().1.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        post(&folders, "/api/mirror", mirror(json!([first]))).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        log(),
        ["run  client c7f3a: save_btn.click: app.nope isn't declared"]
    );
    // The mirror sends sys.errors whole, newest first, each time: only what's new is logged.
    post(&folders, "/api/mirror", mirror(json!([second, first]))).await;
    post(&folders, "/api/mirror", mirror(json!([second, first]))).await;
    assert_eq!(
        log(),
        [
            "run  client c7f3a: save_btn.click: app.nope isn't declared",
            "run  client c7f3a: chain.too_deep: a chain of events went past max_chain",
        ]
    );
}

/// `GET uri` with `token` as the bearer token, from a serve whose agent token is `agent-check-token`.
async fn get_with(folders: &Folders, uri: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut request = Request::get(uri);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let settings = Settings {
        agent_token: Some("agent-check-token".into()),
        ..folders.settings()
    };
    let response = router(settings)
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

const LOG: &str = "\
2026-11-24T09:59:59Z load app.toml:3:1: unknown field `colour`
2026-11-24T10:00:00Z load ok
2026-11-24T10:05:03Z call crm.contacts.update 422 {\"error\":\"email taken\"}
2026-11-24T10:07:30Z run  client c1: save_btn.click: app.nope isn't declared
";

#[tokio::test]
async fn api_log_needs_the_agent_token() {
    let folders = Folders::new("api-log-token");
    std::fs::create_dir_all(folders.path("logs")).unwrap();
    std::fs::write(folders.path("logs/genesis.log"), LOG).unwrap();
    for token in [None, Some("wrong-token")] {
        let (status, body) = get_with(&folders, "/api/log", token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
        assert!(!body.contains("colour"), "{body}");
    }
    // A serve with no agent token keeps it shut.
    assert_eq!(get(&folders, "/api/log").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        get_with(&folders, "/api/log", Some("agent-check-token")).await,
        (StatusCode::OK, LOG.to_string())
    );
}

#[tokio::test]
async fn api_log_narrows_by_since_and_tail() {
    let folders = Folders::new("api-log-narrow");
    std::fs::create_dir_all(folders.path("logs")).unwrap();
    std::fs::write(folders.path("logs/genesis.log"), LOG).unwrap();
    let lines = |from: usize, to: usize| -> String {
        LOG.lines()
            .skip(from)
            .take(to - from)
            .map(|l| format!("{l}\n"))
            .collect()
    };
    let read = |query: &'static str| {
        let folders = &folders;
        async move {
            get_with(
                folders,
                &format!("/api/log{query}"),
                Some("agent-check-token"),
            )
            .await
        }
    };
    assert_eq!(
        read("?since=2026-11-24T10:00:00Z").await,
        (StatusCode::OK, lines(1, 4))
    );
    assert_eq!(read("?tail=2").await, (StatusCode::OK, lines(2, 4)));
    assert_eq!(read("?tail=0").await, (StatusCode::OK, String::new()));
    assert_eq!(read("?tail=100").await, (StatusCode::OK, LOG.to_string()));
    assert_eq!(
        read("?since=2026-11-24T10:00:00Z&tail=1").await,
        (StatusCode::OK, lines(3, 4))
    );
    assert_eq!(
        read("?since=2027-01-01T00:00:00Z").await,
        (StatusCode::OK, String::new())
    );
    for bad in [
        "?since=yesterday",
        "?since=2026-11-24",
        "?tail=-1",
        "?tail=lots",
    ] {
        let (status, body) = read(bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
    }
}

#[tokio::test]
async fn api_log_answers_nothing_before_anything_is_logged() {
    let folders = Folders::new("api-log-empty");
    assert_eq!(
        get_with(&folders, "/api/log", Some("agent-check-token")).await,
        (StatusCode::OK, String::new())
    );
}

/// A serve with the agent token `agent-check-token`, to push through.
fn pushing(folders: &Folders) -> axum::Router {
    router(Settings {
        agent_token: Some("agent-check-token".into()),
        ..folders.settings()
    })
}

/// Pushes `body` to `/api/push/{name}`, with `token`: the status and the answer.
async fn push(
    serve: &axum::Router,
    name: &str,
    token: Option<&str>,
    body: &str,
) -> (StatusCode, String) {
    let mut request = Request::post(format!("/api/push/{name}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = serve
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn a_push_reaches_every_open_config_stream_and_answers_how_many() {
    let folders = Folders::new("push");
    let serve = pushing(&folders);
    let body = r#"{"contact": {"id": 42, "name": "Ada"}, "note": "Ready for review"}"#;
    // No screen open: delivered to none, which isn't an error.
    assert_eq!(
        push(&serve, "review_ready", Some("agent-check-token"), body).await,
        (StatusCode::OK, r#"{"delivered":0}"#.to_string())
    );
    let mut streams = Vec::new();
    for _ in 0..2 {
        let response = serve
            .clone()
            .oneshot(Request::get("/api/config").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let mut body = response.into_body();
        let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert!(String::from_utf8_lossy(&first).starts_with("event: config\n"));
        streams.push(body);
    }
    assert_eq!(
        push(&serve, "review_ready", Some("agent-check-token"), body).await,
        (StatusCode::OK, r#"{"delivered":2}"#.to_string())
    );
    for mut stream in streams {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.frame())
            .await
            .expect("a push")
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();
        let frame = String::from_utf8_lossy(&frame).into_owned();
        let data = frame
            .strip_prefix("event: push\ndata: ")
            .unwrap_or_else(|| panic!("{frame}"));
        let push: Value = serde_json::from_str(data.trim_end()).unwrap();
        assert_eq!(
            push,
            json!({ "name": "review_ready", "payload": { "contact": { "id": 42, "name": "Ada" }, "note": "Ready for review" } })
        );
    }
}

#[tokio::test]
async fn a_refused_push_says_why_and_is_logged() {
    let folders = Folders::new("push-refused");
    let serve = pushing(&folders);
    let cases = [
        ("review_ready", None, "{}", StatusCode::UNAUTHORIZED),
        (
            "review_ready",
            Some("wrong-token"),
            "{}",
            StatusCode::UNAUTHORIZED,
        ),
        (
            "Review-Ready",
            Some("agent-check-token"),
            "{}",
            StatusCode::BAD_REQUEST,
        ),
        (
            "review_ready",
            Some("agent-check-token"),
            "[1, 2]",
            StatusCode::BAD_REQUEST,
        ),
        (
            "review_ready",
            Some("agent-check-token"),
            "not json",
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (name, token, body, want) in cases {
        let (status, why) = push(&serve, name, token, body).await;
        assert_eq!(status, want, "{name} {token:?} {body}: {why}");
    }
    // A serve with no agent token keeps pushing shut.
    let shut = router(folders.settings());
    assert_eq!(
        push(&shut, "review_ready", Some("agent-check-token"), "{}")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let log: Vec<String> = std::fs::read_to_string(folders.path("logs/genesis.log"))
        .unwrap()
        .lines()
        .map(|line| line.split_once(' ').unwrap().1.to_string())
        .collect();
    assert_eq!(
        log,
        [
            "push review_ready: 401 /api/push needs Authorization: Bearer GENESIS_AGENT_TOKEN",
            "push review_ready: 401 /api/push needs Authorization: Bearer GENESIS_AGENT_TOKEN",
            "push Review-Ready: 400 a push's name is lowercase letters, digits, and _",
            "push review_ready: 400 the body isn't a JSON object",
            "push review_ready: 400 the body isn't a JSON object",
            "push review_ready: 401 /api/push needs Authorization: Bearer GENESIS_AGENT_TOKEN",
        ]
    );
}
