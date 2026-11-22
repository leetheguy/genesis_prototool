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
        for dir in ["configs/hello", "state", "dist"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(
            root.join("configs/hello/app.toml"),
            "[app]\nname = \"Hello\"\n",
        )
        .unwrap();
        std::fs::write(root.join("dist/index.html"), "<html>the renderer</html>").unwrap();
        std::fs::write(root.join("dist/genesis.js"), "// bundle").unwrap();
        Folders(root)
    }

    fn settings(&self) -> Settings {
        Settings {
            configs: self.0.join("configs"),
            state: self.0.join("state"),
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
async fn an_apps_page_is_the_renderer() {
    let folders = Folders::new("page");
    assert_eq!(
        get(&folders, "/hello/").await,
        (StatusCode::OK, "<html>the renderer</html>".into())
    );
    assert_eq!(get(&folders, "/hello").await.0, StatusCode::OK);
}

#[tokio::test]
async fn an_app_that_doesnt_exist_is_not_found() {
    let folders = Folders::new("nope");
    assert_eq!(get(&folders, "/nope/").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&folders, "/api/nope/config").await.0,
        StatusCode::NOT_FOUND
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
    let (status, event) = first_event(&folders, "/api/hello/config").await;
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
        folders.path("configs/hello/app.toml"),
        "[app]\nname = \"Hello\"\ncolour = \"red\"\n",
    )
    .unwrap();
    let (_, event) = first_event(&folders, "/api/hello/config").await;
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
        "/api/hello/mirror",
        json!({ "client": "abc123", "state": { "app": { "n": 1 } }, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let state: Value =
        serde_json::from_str(&read(&folders.path("state/hello/abc123.json"))).unwrap();
    assert_eq!(state, json!({ "app": { "n": 1 } }));
}

#[tokio::test]
async fn the_mirror_appends_each_event_as_a_line() {
    let folders = Folders::new("events");
    post(
        &folders,
        "/api/hello/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "a" }] }),
    )
    .await;
    post(
        &folders,
        "/api/hello/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "b" }, { "log": "x" }] }),
    )
    .await;
    let lines: Vec<Value> = read(&folders.path("state/hello/c1.events.ndjson"))
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
        "/api/hello/mirror",
        json!({ "client": "c1", "state": {}, "events": events }),
    )
    .await;
    let text = read(&folders.path("state/hello/c1.events.ndjson"));
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
            "/api/hello/mirror",
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

#[tokio::test]
async fn an_app_name_that_isnt_plain_is_refused() {
    let folders = Folders::new("appname");
    assert_eq!(
        get(&folders, "/api/..%2Fhello/config").await.0,
        StatusCode::BAD_REQUEST
    );
    let status = post(
        &folders,
        "/api/%2E%2E/mirror",
        json!({ "client": "c1", "state": {}, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_mirror_for_an_app_that_doesnt_exist_is_not_found() {
    let folders = Folders::new("mirror404");
    let status = post(
        &folders,
        "/api/nope/mirror",
        json!({ "client": "c1", "state": {}, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
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
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(
        (s.listen.as_str(), s.configs, s.state, s.dist),
        ("0.0.0.0:9000", "/c".into(), "/s".into(), "/d".into())
    );
    assert!(Settings::from_args(["--bogus".to_string()]).is_err());
}

#[tokio::test]
async fn names_may_hold_underscores_and_dashes() {
    let folders = Folders::new("plain");
    let status = post(
        &folders,
        "/api/hello/mirror",
        json!({ "client": "my_client-1", "state": {}, "events": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_event_file_that_cant_be_read_fails_loud_and_is_left_alone() {
    let folders = Folders::new("unreadable");
    std::fs::create_dir_all(folders.path("state/hello")).unwrap();
    std::fs::write(
        folders.path("state/hello/c1.events.ndjson"),
        [0xff, 0xfe, 0x00],
    )
    .unwrap();
    let status = post(
        &folders,
        "/api/hello/mirror",
        json!({ "client": "c1", "state": {}, "events": [{ "name": "a" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let untouched = std::fs::read(folders.path("state/hello/c1.events.ndjson")).unwrap();
    assert_eq!(untouched, [0xff, 0xfe, 0x00]);
}
