//! Hot reload (spec 060): a saved change reaches every open config stream, debounced.

use axum::body::Body;
use axum::http::Request;
use genesis_serve::{Settings, live_router};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::time::Duration;
use tower::ServiceExt;

struct Folders(PathBuf);

impl Folders {
    fn new(name: &str) -> Folders {
        let root =
            std::env::temp_dir().join(format!("genesis-reload-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["configs/views", "state", "dist"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("configs/app.toml"), "[app]\nname = \"First\"\n").unwrap();
        Folders(root)
    }

    fn settings(&self) -> Settings {
        Settings {
            configs: self.0.join("configs"),
            state: self.0.join("state"),
            logs: self.0.join("logs"),
            dist: self.0.join("dist"),
            debounce: Duration::from_millis(100),
            ..Settings::default()
        }
    }

    fn write(&self, rel: &str, text: &str) {
        std::fs::write(self.0.join(rel), text).unwrap();
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An open config stream, read an event at a time.
struct Stream(Body);

impl Stream {
    async fn open(router: &axum::Router) -> Stream {
        let request = Request::get("/api/config").body(Body::empty()).unwrap();
        Stream(router.clone().oneshot(request).await.unwrap().into_body())
    }

    /// The next event, or `None` if none comes within `wait`.
    async fn next(&mut self, wait: Duration) -> Option<String> {
        let frame = tokio::time::timeout(wait, self.0.frame())
            .await
            .ok()??
            .ok()?;
        Some(String::from_utf8_lossy(&frame.into_data().ok()?).into_owned())
    }
}

const SOON: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(600);

#[tokio::test]
async fn a_saved_change_reaches_an_open_stream() {
    let folders = Folders::new("change");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    assert!(stream.next(SOON).await.unwrap().contains("First"));
    folders.write("configs/app.toml", "[app]\nname = \"Second\"\n");
    let event = stream.next(SOON).await.expect("a new config");
    assert!(
        event.starts_with("event: config\n") && event.contains("Second"),
        "{event}"
    );
}

#[tokio::test]
async fn a_bad_edit_sends_its_problems_and_the_next_good_one_a_config() {
    let folders = Folders::new("bad");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    folders.write("configs/app.toml", "[app]\nname = \"x\"\ncolour = 1\n");
    let event = stream.next(SOON).await.expect("problems");
    assert!(
        event.starts_with("event: problems\n") && event.contains("app.toml:3:1"),
        "{event}"
    );
    folders.write("configs/app.toml", "[app]\nname = \"Fixed\"\n");
    assert!(stream.next(SOON).await.expect("a config").contains("Fixed"));
}

#[tokio::test]
async fn a_burst_of_writes_makes_one_reload() {
    let folders = Folders::new("burst");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    for name in ["A", "B", "C"] {
        folders.write("configs/app.toml", &format!("[app]\nname = \"{name}\"\n"));
    }
    assert!(
        stream
            .next(SOON)
            .await
            .expect("one reload")
            .contains("\"C\"")
    );
    assert_eq!(stream.next(QUIET).await, None);
}

#[tokio::test]
async fn a_deleted_app_toml_sends_problems_and_recreating_it_reloads() {
    let folders = Folders::new("deleted");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    std::fs::remove_file(folders.0.join("configs/app.toml")).unwrap();
    assert!(
        stream
            .next(SOON)
            .await
            .expect("problems")
            .contains("app.toml not found")
    );
    folders.write("configs/app.toml", "[app]\nname = \"Back\"\n");
    assert!(stream.next(SOON).await.expect("a config").contains("Back"));
}

#[test]
fn the_debounce_is_set_in_milliseconds_and_defaults_to_150() {
    assert_eq!(Settings::default().debounce, Duration::from_millis(150));
    let s = Settings::from_args(["--debounce", "40"].map(String::from)).unwrap();
    assert_eq!(s.debounce, Duration::from_millis(40));
}

#[tokio::test]
async fn a_change_to_any_file_in_any_folder_reloads_the_app() {
    let folders = Folders::new("anyfile");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    folders.write("configs/views/notes.txt", "hi");
    assert!(stream.next(SOON).await.expect("a reload").contains("First"));
}

impl Folders {
    /// The log's lines, without their times.
    fn log(&self) -> Vec<String> {
        std::fs::read_to_string(self.0.join("logs").join(genesis_serve::LOG_FILE))
            .unwrap_or_default()
            .lines()
            .map(|line| line.split_once(' ').unwrap().1.to_string())
            .collect()
    }

    /// The log once it has `n` lines.
    async fn log_of(&self, n: usize) -> Vec<String> {
        let deadline = tokio::time::Instant::now() + SOON;
        while self.log().len() < n && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.log()
    }
}

#[tokio::test]
async fn a_load_problem_is_logged_with_where_it_is_and_a_clean_load_after_it_logs_load_ok() {
    let folders = Folders::new("log-load");
    folders.write("configs/app.toml", "[app]\nname = \"x\"\ncolour = 1\n");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let lines = folders.log_of(1).await;
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("load app.toml:3:1: unknown field `colour`"),
        "{lines:?}"
    );
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    folders.write("configs/app.toml", "[app]\nname = \"Fixed\"\n");
    stream.next(SOON).await.expect("a config");
    assert_eq!(folders.log_of(2).await[1], "load ok");
    // Clean again: nothing more to say.
    folders.write("configs/app.toml", "[app]\nname = \"Again\"\n");
    stream.next(SOON).await.expect("a config");
    assert_eq!(folders.log().len(), 2);
}

#[tokio::test]
async fn a_clean_start_logs_nothing() {
    let folders = Folders::new("log-clean");
    let (router, _watching) = live_router(folders.settings()).unwrap();
    Stream::open(&router).await.next(SOON).await.unwrap();
    tokio::time::sleep(QUIET).await;
    assert!(folders.log().is_empty());
}

#[tokio::test]
async fn the_log_keeps_only_its_last_log_lines_lines() {
    let folders = Folders::new("log-lines");
    let app = "[app]\nname = \"x\"\nlog_lines = 3\n";
    folders.write("configs/app.toml", app);
    let (router, _watching) = live_router(folders.settings()).unwrap();
    let mut stream = Stream::open(&router).await;
    stream.next(SOON).await.unwrap();
    // A bad load, another, a clean one, and a third bad one: four lines, of which three are kept.
    for (blank, bad) in [(0, true), (1, true), (9, false), (2, true)] {
        let text = match blank {
            9 => app.to_string(),
            _ => format!("{app}{}colour = 1\n", "\n".repeat(blank)),
        };
        folders.write("configs/app.toml", &text);
        let event = stream.next(SOON).await.expect("a reload");
        assert_eq!(event.contains("problems"), bad, "{event}");
    }
    let lines = folders.log();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("load app.toml:5:1:"), "{lines:?}");
    assert_eq!(lines[1], "load ok");
    assert!(lines[2].starts_with("load app.toml:6:1:"), "{lines:?}");
}
