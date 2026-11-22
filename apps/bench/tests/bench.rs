//! genesis-bench, end to end: an app folder in, events on stdin, the trail and state out (spec 100, "The workbench").

use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const HELLO: &str = r#"
[app]
name = "Hello"

[state]
clicks = 0

[[body]]
type = "button"
id = "hello_btn"
label = "Click me"

[on]
"hello_btn.click" = [{ step = "app.clicks" }, { log = "clicked {{ app.clicks }} times" }]
"#;

/// A fresh folder holding `app.toml`, removed when dropped.
struct AppDir(PathBuf);

impl AppDir {
    fn new(name: &str, toml: &str) -> AppDir {
        let dir = std::env::temp_dir().join(format!("genesis-bench-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.toml"), toml).unwrap();
        AppDir(dir)
    }
}

impl Drop for AppDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn bench(target: &std::path::Path, stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_genesis-bench"))
        .arg(target)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn lines(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is JSON"))
        .collect()
}

#[test]
fn it_prints_each_event_as_its_handled_then_the_final_state() {
    let app = AppDir::new("hello", HELLO);
    let out = bench(
        &app.0,
        "{\"fire\": \"hello_btn.click\"}\n{\"fire\": \"hello_btn.click\"}\n",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = lines(&out);
    let names: Vec<&Value> = lines.iter().filter_map(|l| l.get("name")).collect();
    assert_eq!(
        names,
        [
            "app.opened",
            "hello_btn.click",
            "app.clicks.changed",
            "hello_btn.click",
            "app.clicks.changed"
        ]
    );
    let logs: Vec<&Value> = lines.iter().filter_map(|l| l.get("log")).collect();
    assert_eq!(logs, ["clicked 1 times", "clicked 2 times"]);
    assert_eq!(
        lines.last().unwrap()["state"]["app"],
        json!({ "clicks": 2 })
    );
}

#[test]
fn a_fired_event_can_carry_a_payload() {
    let app = AppDir::new(
        "payload",
        "[app]\nname = \"x\"\n[on]\n\"hi\" = [{ log = \"hi {{ event.who }}\" }]\n",
    );
    let out = bench(
        &app.0,
        "{\"fire\": \"hi\", \"payload\": {\"who\": \"Ada\"}}\n",
    );
    assert!(lines(&out).iter().any(|l| l["log"] == "hi Ada"));
}

#[test]
fn advance_moves_the_clock_and_runs_timers() {
    let app = AppDir::new(
        "timers",
        "[app]\nname = \"x\"\n[on]\n\"app.opened\" = [{ every = 100, fire = \"tick\" }]\n",
    );
    let out = bench(&app.0, "{\"advance\": 250}\n");
    let ticks = lines(&out).iter().filter(|l| l["name"] == "tick").count();
    assert_eq!(ticks, 2);
}

#[test]
fn toasts_are_printed_as_effects() {
    let app = AppDir::new(
        "toast",
        "[app]\nname = \"x\"\n[state]\nname = \"Ada\"\n[on]\n\"go\" = [{ toggle = \"app.name\" }]\n",
    );
    let out = bench(&app.0, "{\"fire\": \"go\"}\n");
    assert!(
        lines(&out)
            .iter()
            .any(|l| l["toast"] == "go: app.name isn't true or false")
    );
}

#[test]
fn it_takes_the_app_toml_file_as_well_as_its_folder() {
    let app = AppDir::new("file", HELLO);
    let out = bench(&app.0.join("app.toml"), "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn an_app_that_doesnt_load_fails_with_where_the_problem_is() {
    let app = AppDir::new("bad", "[app]\nname = \"x\"\ncolour = \"red\"\n");
    let out = bench(&app.0, "");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("app.toml:3:1") && stderr.contains("colour"),
        "{stderr}"
    );
}

#[test]
fn a_line_it_cant_read_fails_naming_the_line() {
    let app = AppDir::new("badline", HELLO);
    let out = bench(
        &app.0,
        "{\"fire\": \"hello_btn.click\"}\n{\"explode\": 1}\n",
    );
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("line 2"), "{stderr}");
}

const CALLS: &str = r#"
[app]
name = "Calls"

[connections.api]
base_url = "http://api"

[connections.api.ops.hi]
method = "GET"
path = "/hi"

[sources.greeting]
call = "api.hi"
select = "/text"
"#;

fn bench_with_mocks(app: &std::path::Path, mocks: &str, stdin: &str) -> Output {
    let file = app.join("mocks.toml");
    std::fs::write(&file, mocks).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_genesis-bench"))
        .arg(app)
        .arg("--mock")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn a_mock_answers_a_call() {
    let app = AppDir::new("mock", CALLS);
    let out = bench_with_mocks(
        &app.0,
        "[[mock]]\nop = \"api.hi\"\nbody = { text = \"hello\" }\n",
        "",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = lines(&out);
    assert!(lines.iter().any(|l| l["name"] == "api.hi.ok"));
    assert_eq!(lines.last().unwrap()["state"]["app"]["greeting"], "hello");
}

#[test]
fn a_mock_can_answer_with_a_status_or_a_failure() {
    let app = AppDir::new("mockfail", CALLS);
    let out = bench_with_mocks(
        &app.0,
        "[[mock]]\nop = \"api.hi\"\nstatus = 503\nbody = { error = \"down\" }\n",
        "",
    );
    assert!(lines(&out).iter().any(|l| l["name"] == "api.hi.503"));
    let out = bench_with_mocks(
        &app.0,
        "[[mock]]\nop = \"api.hi\"\nerror = \"refused\"\n",
        "",
    );
    assert!(
        lines(&out)
            .iter()
            .any(|l| l["name"] == "api.hi.failed" && l["payload"]["error"] == "refused")
    );
}

#[test]
fn a_call_with_no_mock_fails_saying_so() {
    let app = AppDir::new("nomock", CALLS);
    let out = bench_with_mocks(&app.0, "", "");
    assert!(
        lines(&out)
            .iter()
            .any(|l| l["payload"]["error"] == "the bench has no mock for api.hi")
    );
}

#[test]
fn without_mocks_calls_are_printed_and_left_waiting() {
    let app = AppDir::new("nomocks", CALLS);
    let out = bench(&app.0, "");
    assert!(lines(&out).iter().any(|l| l["call"]["op"] == "api.hi"));
}

#[test]
fn an_option_it_doesnt_know_shows_how_to_use_it() {
    let app = AppDir::new("badflag", CALLS);
    let out = Command::new(env!("CARGO_BIN_EXE_genesis-bench"))
        .arg(&app.0)
        .arg("--mocks")
        .arg("x.toml")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage:"));
}
