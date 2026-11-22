//! /fs/: the files service, behind the agent token (spec 010; PurpleFlow spec 090).

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode, Uri};
use axum::routing::any;
use genesis_serve::{Settings, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// A mock dufs on a random port that records what reaches it and answers with it.
async fn mock_files(seen: Arc<Mutex<Vec<Value>>>) -> String {
    let api = axum::Router::new().fallback(any(move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
        let seen = seen.clone();
        async move {
            let header = |n: &str| headers.get(n).and_then(|v| v.to_str().ok()).map(String::from);
            let logins = headers.get_all("authorization").iter().count();
            let request = json!({ "method": method.as_str(), "uri": uri.to_string(), "authorization": header("authorization"), "logins": logins, "x_custom": header("x-custom"), "body": String::from_utf8_lossy(&body) });
            seen.lock().unwrap().push(request.clone());
            (StatusCode::CREATED, [("content-type", "application/json"), ("x-dufs", "yes")], request.to_string())
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    format!("http://{address}")
}

async fn settings(seen: Arc<Mutex<Vec<Value>>>) -> Settings {
    Settings {
        files_url: Some(mock_files(seen).await),
        agent_token: Some("agent-secret".into()),
        ..Settings::default()
    }
}

async fn send(settings: Settings, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = router(settings).oneshot(request).await.unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn without_the_agent_token_nothing_gets_through() {
    let seen = Arc::default();
    for auth in [
        None,
        Some("Bearer wrong"),
        Some("agent-secret"),
        Some("Basic genesis:agent-secret"),
    ] {
        let mut request = Request::get("/fs/crm/app.toml");
        if let Some(auth) = auth {
            request = request.header("authorization", auth);
        }
        let (status, _, _) = send(
            settings(Arc::clone(&seen)).await,
            request.body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{auth:?}");
    }
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn with_it_the_request_reaches_dufs_with_dufs_own_login_instead() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let request = Request::put("/fs/crm/app.toml?x=1")
        .header("authorization", "Bearer agent-secret")
        .header("x-custom", "kept")
        .body(Body::from("[app]"))
        .unwrap();
    let (status, headers, _) = send(settings(seen.clone()).await, request).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        headers.get("x-dufs").map(|v| v.to_str().unwrap()),
        Some("yes")
    );
    let reached = seen.lock().unwrap()[0].clone();
    assert_eq!(
        (&reached["method"], &reached["uri"], &reached["body"]),
        (
            &json!("PUT"),
            &json!("/fs/crm/app.toml?x=1"),
            &json!("[app]")
        )
    );
    // Basic base64("genesis:agent-secret"), and only that: the agent's Bearer token stays with serve.
    assert_eq!(
        reached["authorization"],
        "Basic Z2VuZXNpczphZ2VudC1zZWNyZXQ="
    );
    assert_eq!(reached["logins"], 1);
    assert_eq!(
        reached["x_custom"], "kept",
        "the request's other headers pass through"
    );
}

#[tokio::test]
async fn the_bare_fs_path_reaches_dufs_too() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let request = Request::get("/fs")
        .header("authorization", "Bearer agent-secret")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(settings(seen.clone()).await, request).await.0,
        StatusCode::CREATED
    );
    assert_eq!(seen.lock().unwrap()[0]["uri"], "/fs");
}

#[tokio::test]
async fn without_a_files_service_there_is_no_fs() {
    let request = Request::get("/fs/x")
        .header("authorization", "Bearer agent-secret")
        .body(Body::empty())
        .unwrap();
    let settings = Settings {
        agent_token: Some("agent-secret".into()),
        ..Settings::default()
    };
    assert_eq!(send(settings, request).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn without_an_agent_token_set_fs_stays_shut() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let settings = Settings {
        agent_token: None,
        ..settings(seen.clone()).await
    };
    let request = Request::get("/fs/x")
        .header("authorization", "Bearer ")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(settings, request).await.0, StatusCode::UNAUTHORIZED);
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_files_service_that_cant_be_reached_is_a_bad_gateway() {
    let settings = Settings {
        files_url: Some("http://127.0.0.1:1".into()),
        agent_token: Some("agent-secret".into()),
        ..Settings::default()
    };
    let request = Request::get("/fs/x")
        .header("authorization", "Bearer agent-secret")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(settings, request).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!body.contains("agent-secret"), "{body}");
}

#[test]
fn the_files_service_and_agent_token_come_from_the_environment() {
    let vars = [
        ("GENESIS_FILES_URL", "http://files:5000"),
        ("GENESIS_AGENT_TOKEN", "t"),
        ("GENESIS_CREDS_PF", "x"),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()));
    let settings = Settings::default().with_env(vars);
    assert_eq!(settings.creds.names(), ["PF"]);
    assert_eq!(
        (
            settings.files_url.as_deref(),
            settings.agent_token.as_deref()
        ),
        (Some("http://files:5000"), Some("t"))
    );
    assert!(
        !format!("{settings:?}").contains("agent_token: Some(\"t\")"),
        "the token never prints"
    );
}

#[tokio::test]
async fn a_token_that_only_starts_or_nearly_matches_like_the_real_one_doesnt_get_through() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    // "ceent-secret" differs from "agent-secret" in two places whose differences cancel out under XOR.
    for wrong in ["agent-secret-and-more", "agent-secreX", "ceent-secret"] {
        let request = Request::get("/fs/x")
            .header("authorization", format!("Bearer {wrong}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(settings(seen.clone()).await, request).await.0,
            StatusCode::UNAUTHORIZED,
            "{wrong}"
        );
    }
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_empty_agent_token_keeps_fs_shut() {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let settings = Settings {
        agent_token: Some(String::new()),
        ..settings(seen.clone()).await
    };
    let request = Request::get("/fs/x")
        .header("authorization", "Bearer ")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(settings, request).await.0, StatusCode::UNAUTHORIZED);
}
