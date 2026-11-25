//! serve makes the calls (spec 030, "Calls"; spec 070): only declared operations, credentials
//! filled only where the config names them, and every failure loud and redacted.

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Query};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::routing::{any, get};
use genesis_serve::{Settings, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tower::ServiceExt;

/// A mock API on a random local port: `/echo/*` answers with what it was sent.
async fn mock_api() -> String {
    async fn echo(
        method: Method,
        UrlPath(rest): UrlPath<String>,
        Query(query): Query<BTreeMap<String, String>>,
        headers: HeaderMap,
        body: Bytes,
    ) -> axum::Json<Value> {
        let headers: BTreeMap<String, String> = headers
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        axum::Json(
            json!({ "method": method.as_str(), "path": rest, "query": query, "headers": headers, "body": body }),
        )
    }
    /// What reached the API as multipart: each part's name, file name, type, and text.
    async fn parts(method: Method, mut form: axum::extract::Multipart) -> axum::Json<Value> {
        let mut parts = Vec::new();
        while let Some(part) = form.next_field().await.unwrap() {
            let (name, file) = (
                part.name().map(String::from),
                part.file_name().map(String::from),
            );
            let kind = part.content_type().map(String::from);
            let text = part.text().await.unwrap();
            parts.push(json!({ "name": name, "file": file, "type": kind, "text": text }));
        }
        axum::Json(json!({ "method": method.as_str(), "parts": parts }))
    }
    let api = axum::Router::new()
        .route("/echo/{*rest}", any(echo))
        .route("/upload", any(parts))
        .route(
            "/status/{code}",
            get(|UrlPath(code): UrlPath<u16>| async move {
                (
                    StatusCode::from_u16(code).unwrap(),
                    axum::Json(json!({ "error": "nope" })),
                )
            }),
        )
        .route("/text", get(|| async { "plain words" }))
        .route("/empty", get(|| async { StatusCode::NO_CONTENT }))
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                "late"
            }),
        )
        .route(
            "/slowish",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                axum::Json(json!({ "waited": true }))
            }),
        )
        .route(
            "/boom",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    format!("http://{address}")
}

struct Folders(PathBuf);

impl Folders {
    async fn new(name: &str) -> Folders {
        let base = mock_api().await;
        let root =
            std::env::temp_dir().join(format!("genesis-calls-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("configs")).unwrap();
        let toml = format!(
            r#"
[app]
name = "CRM"

[connections.api]
base_url = "{base}"
headers = {{ Authorization = "Bearer {{{{ creds.API_TOKEN }}}}", "X-User" = "{{{{ app.user }}}}" }}

[connections.api.ops.echo_get]
method = "GET"
path = "/echo/contacts/{{id}}"

[connections.api.ops.echo_post]
method = "POST"
path = "/echo/contacts"

[connections.api.ops.upload]
method = "POST"
path = "/upload"

[connections.api.ops.upload_get]
method = "GET"
path = "/upload"

[connections.api.ops.missing]
method = "GET"
path = "/status/404"

[connections.api.ops.text]
method = "GET"
path = "/text"

[connections.api.ops.empty]
method = "GET"
path = "/empty"

[connections.api.ops.slow]
method = "GET"
path = "/slow"
timeout = 1

[connections.api.ops.patient]
method = "GET"
path = "/slowish"
timeout = 0

[connections.api.ops.boom]
method = "GET"
path = "/boom"

[connections.nocreds]
base_url = "{base}"
headers = {{ Authorization = "Bearer {{{{ creds.UNSET }}}}" }}

[connections.nocreds.ops.echo]
method = "GET"
path = "/echo/x"
"#
        );
        std::fs::write(root.join("configs/app.toml"), toml).unwrap();
        Folders(root)
    }

    fn settings(&self) -> Settings {
        let creds = BTreeMap::from([
            ("API_TOKEN".to_string(), "s3cret-value".to_string()),
            ("OTHER".to_string(), "other-secret".to_string()),
        ]);
        Settings {
            configs: self.0.join("configs"),
            state: self.0.join("state"),
            dist: self.0.join("dist"),
            creds: creds.into(),
            ..Settings::default()
        }
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn call(folders: &Folders, body: Value) -> (StatusCode, Value) {
    let request = Request::post("/api/call")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router(folders.settings()).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        // A refusal is plain text: keep it as a string, so tests can read why.
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())),
    )
}

/// A call with files, as the browser sends it: the call as JSON in a `call` part, and each file
/// (`(id, name, type, text)`) in a part named by its reference's ID.
async fn call_with_files(
    folders: &Folders,
    body: Value,
    files: &[(&str, &str, &str, &str)],
) -> (StatusCode, Value) {
    let boundary = "genesis-test-boundary";
    let mut form =
        format!("--{boundary}\r\ncontent-disposition: form-data; name=\"call\"\r\n\r\n{body}\r\n");
    for (id, name, kind, text) in files {
        form.push_str(&format!(
            "--{boundary}\r\ncontent-disposition: form-data; name=\"{id}\"; filename=\"{name}\"\r\ncontent-type: {kind}\r\n\r\n{text}\r\n"
        ));
    }
    form.push_str(&format!("--{boundary}--\r\n"));
    let request = Request::post("/api/call/files")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(form))
        .unwrap();
    let response = router(folders.settings()).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A file reference, as a `file` control gives it.
fn reference(id: &str, name: &str) -> Value {
    json!({ "file": id, "name": name, "type": "text/plain", "size": 5 })
}

#[tokio::test]
async fn params_holding_a_file_go_out_as_multipart_with_the_file_as_a_part() {
    let folders = Folders::new("multipart").await;
    let (status, reply) = call_with_files(
        &folders,
        json!({ "op": "api.upload", "params": { "title": "Q3", "doc": reference("file-1", "notes.txt") }, "headers": {} }),
        &[("file-1", "notes.txt", "text/plain", "hello")],
    )
    .await;
    assert_eq!(
        (status, &reply["status"]),
        (StatusCode::OK, &json!(200)),
        "{reply}"
    );
    assert_eq!(
        reply["body"]["parts"],
        json!([
            { "name": "title", "file": null, "type": null, "text": "Q3" },
            { "name": "doc", "file": "notes.txt", "type": "text/plain", "text": "hello" },
        ])
    );
}

#[tokio::test]
async fn a_list_of_files_is_one_part_each_whatever_the_method() {
    let folders = Folders::new("multipart-list").await;
    let docs = json!([reference("file-1", "a.txt"), reference("file-2", "b.txt")]);
    let (_, reply) = call_with_files(
        &folders,
        json!({ "op": "api.upload_get", "params": { "docs": docs }, "headers": {} }),
        &[
            ("file-1", "a.txt", "text/plain", "one"),
            ("file-2", "b.txt", "text/plain", "two"),
        ],
    )
    .await;
    assert_eq!(reply["body"]["method"], "GET");
    let parts = reply["body"]["parts"].as_array().unwrap();
    let texts: Vec<_> = parts.iter().map(|p| (&p["name"], &p["text"])).collect();
    assert_eq!(
        texts,
        [
            (&json!("docs"), &json!("one")),
            (&json!("docs"), &json!("two"))
        ]
    );
}

#[tokio::test]
async fn a_file_reference_whose_file_is_gone_fails_the_call() {
    let folders = Folders::new("multipart-gone").await;
    let call_body = json!({ "op": "api.upload", "params": { "doc": reference("file-9", "notes.txt") }, "headers": {} });
    for (_, reply) in [
        call_with_files(&folders, call_body.clone(), &[]).await,
        call(&folders, call_body).await,
    ] {
        let error = reply["error"].as_str().unwrap_or_default();
        assert!(
            error.contains("notes.txt") && error.contains("gone"),
            "{reply}"
        );
    }
}

#[tokio::test]
async fn a_get_puts_params_in_the_path_and_query() {
    let folders = Folders::new("get").await;
    let (status, reply) = call(
        &folders,
        json!({ "op": "api.echo_get", "params": { "id": "a b/c", "page": 2 }, "headers": {} }),
    )
    .await;
    assert_eq!((status, &reply["status"]), (StatusCode::OK, &json!(200)));
    assert_eq!(reply["body"]["method"], "GET");
    assert_eq!(
        reply["body"]["path"], "contacts/a b/c",
        "the id is URL-encoded on the way and decoded by the API"
    );
    assert_eq!(
        reply["body"]["query"],
        json!({ "page": "2" }),
        "the path's param isn't sent again"
    );
}

#[tokio::test]
async fn a_post_sends_its_params_as_a_json_body() {
    let folders = Folders::new("post").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.echo_post", "params": { "name": "Ada", "tags": [1] }, "headers": {} }),
    )
    .await;
    assert_eq!(reply["body"]["body"], json!({ "name": "Ada", "tags": [1] }));
}

#[tokio::test]
async fn credentials_are_filled_in_on_the_server() {
    let folders = Folders::new("creds").await;
    let headers = json!({ "Authorization": "Bearer {{ creds.API_TOKEN }}", "X-User": "ada" });
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.echo_post", "params": {}, "headers": headers }),
    )
    .await;
    assert_eq!(
        reply["body"]["headers"]["authorization"],
        "Bearer s3cret-value"
    );
    assert_eq!(reply["body"]["headers"]["x-user"], "ada");
}

#[tokio::test]
async fn a_client_cant_have_a_credential_filled_where_the_config_doesnt_name_it() {
    let folders = Folders::new("smuggle").await;
    let headers =
        json!({ "Authorization": "Bearer {{ creds.OTHER }}", "X-User": "{{ creds.API_TOKEN }}" });
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.echo_post", "params": {}, "headers": headers }),
    )
    .await;
    let sent = &reply["body"]["headers"];
    assert!(!reply.to_string().contains("other-secret"), "{reply}");
    assert_eq!(
        sent["authorization"], "Bearer s3cret-value",
        "the config's own template decides, not the client"
    );
    assert_eq!(
        sent["x-user"], "{{ creds.API_TOKEN }}",
        "a credential the config doesn't name there stays text"
    );
}

#[tokio::test]
async fn headers_the_config_doesnt_declare_arent_sent() {
    let folders = Folders::new("undeclared").await;
    let headers = json!({ "Authorization": "Bearer {{ creds.API_TOKEN }}", "X-Evil": "1" });
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.echo_post", "params": {}, "headers": headers }),
    )
    .await;
    assert_eq!(reply["body"]["headers"]["x-evil"], Value::Null);
    assert_eq!(
        reply["body"]["headers"]["x-user"],
        Value::Null,
        "a declared header the client didn't fill isn't sent half-made"
    );
}

#[tokio::test]
async fn a_bad_status_comes_back_with_its_body() {
    let folders = Folders::new("status").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.missing", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(reply, json!({ "status": 404, "body": { "error": "nope" } }));
}

#[tokio::test]
async fn a_2xx_body_that_isnt_json_is_a_failure() {
    let folders = Folders::new("text").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.text", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(
        reply,
        json!({ "error": "the reply to api.text isn't JSON" })
    );
}

#[tokio::test]
async fn no_body_is_null() {
    let folders = Folders::new("empty").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.empty", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(reply, json!({ "status": 204, "body": null }));
}

#[tokio::test]
async fn a_slow_reply_times_out_loudly() {
    let folders = Folders::new("slow").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.slow", "params": {}, "headers": {} }),
    )
    .await;
    assert!(
        reply["error"].as_str().unwrap().contains("timed out"),
        "{reply}"
    );
}

#[tokio::test]
async fn a_timeout_of_zero_waits_as_long_as_it_takes() {
    let folders = Folders::new("patient").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.patient", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(reply, json!({ "status": 200, "body": { "waited": true } }));
}

#[tokio::test]
async fn a_bad_status_with_a_text_body_comes_back_as_text() {
    let folders = Folders::new("boom").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.boom", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(reply, json!({ "status": 500, "body": "boom" }));
}

#[tokio::test]
async fn an_unset_credential_fails_and_sends_nothing() {
    let folders = Folders::new("unset").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "nocreds.echo", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(
        reply,
        json!({ "error": "credential UNSET isn't set (GENESIS_CREDS_UNSET)" })
    );
}

#[tokio::test]
async fn a_missing_path_param_fails_naming_it() {
    let folders = Folders::new("pathparam").await;
    let (_, reply) = call(
        &folders,
        json!({ "op": "api.echo_get", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(reply, json!({ "error": "api.echo_get needs the param id" }));
}

#[tokio::test]
async fn an_operation_the_app_doesnt_declare_is_not_found() {
    let folders = Folders::new("undeclared-op").await;
    let (status, why) = call(
        &folders,
        json!({ "op": "api.nowhere", "params": {}, "headers": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(why, "the app declares no operation named \"api.nowhere\"");
}

#[tokio::test]
async fn a_failure_never_shows_a_credentials_value() {
    let folders = Folders::new("redact").await;
    std::fs::write(
        folders.0.join("configs/app.toml"),
        "[app]\nname = \"x\"\n[connections.bad]\nbase_url = \"http://s3cret-value.invalid\"\nheaders = { Authorization = \"{{ creds.API_TOKEN }}\" }\n[connections.bad.ops.go]\nmethod = \"GET\"\npath = \"/\"\n",
    )
    .unwrap();
    let (_, reply) = call(
        &folders,
        json!({ "op": "bad.go", "params": {}, "headers": {} }),
    )
    .await;
    let error = reply["error"].as_str().unwrap();
    assert!(
        !error.contains("s3cret-value") && error.contains("[redacted]"),
        "{error}"
    );
}

#[test]
fn settings_never_print_a_credentials_value() {
    let settings = Settings {
        creds: BTreeMap::from([("API_TOKEN".to_string(), "s3cret-value".to_string())]).into(),
        ..Settings::default()
    };
    let printed = format!("{settings:?}");
    assert!(
        printed.contains("API_TOKEN") && !printed.contains("s3cret-value"),
        "{printed}"
    );
}

#[test]
fn credentials_come_from_genesis_creds_environment_variables() {
    let vars = [
        ("GENESIS_CREDS_PF_TOKEN", "abc"),
        ("PATH", "/bin"),
        ("GENESIS_CREDS_", "x"),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()));
    let creds = genesis_serve::Creds::from_vars(vars);
    assert_eq!(creds.names(), ["PF_TOKEN"]);
}
