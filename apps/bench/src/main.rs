//! `genesis-bench`: runs a Genesis app's event bus with no screen, for tests and agents.
//!
//! ```text
//! genesis-bench APP_DIR_OR_APP_TOML [--mock MOCKS_TOML] < script.ndjson
//! ```
//!
//! It loads the app (every `.toml` file in the configs folder, [spec 020](../../../specs/020_config.md)), opens it (firing `app.opened`), then reads one JSON object per line from
//! stdin ([spec 100](../../../specs/100_events.md), "The workbench"):
//!
//! - `{"fire": "hello_btn.click", "payload": {...}}` fires an event (`payload` is optional).
//! - `{"advance": 250}` moves the clock on 250 milliseconds, running any timers that fall due.
//! - `{"answer": true}` (or `false`) answers the first `confirm` waiting.
//!
//! After each line it prints, one JSON object per line, every trail entry recorded (events with
//! their handlers, and `log` lines), then any toasts as `{"toast": "...", "variant": "..."}` and
//! questions as `{"confirm": "...", "id": N}`. At the end it prints
//! `{"state": ...}`, the final state.
//!
//! A call the app makes is printed as `{"call": {...}}`. With `--mock`, each call is answered from
//! the mocks file at once, and what follows is printed too:
//!
//! ```toml
//! [[mock]]
//! op = "crm.contacts.list"
//! status = 200                  # default 200
//! body = { rows = [] }          # default null
//!
//! [[mock]]
//! op = "crm.contacts.create"
//! error = "refused"             # a call that got no response
//! ```
//!
//! A call with no mock fails, saying so. Without `--mock`, calls are left waiting. Keep the mocks
//! file outside the configs folder: every `.toml` file inside it is part of the app.
//!
//! It exits with `2` when the app doesn't load (each problem printed as `FILE:LINE:COLUMN: ...`),
//! and `1` when a line of the script can't be read.

use genesis_core::bus::{Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

/// One `[[mock]]`: the answer to every call to `op`.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Mock {
    op: String,
    #[serde(default = "ok")]
    status: u16,
    #[serde(default)]
    body: Value,
    error: Option<String>,
}

fn ok() -> u16 {
    200
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Mocks {
    #[serde(default)]
    mock: Vec<Mock>,
}

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let (target, mocks) = match args.as_slice() {
        [target] => (target, None),
        [target, flag, file] if flag == "--mock" => match load_mocks(Path::new(file)) {
            Ok(mocks) => (target, Some(mocks)),
            Err(why) => {
                eprintln!("{why}");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!(
                "usage: genesis-bench APP_DIR_OR_APP_TOML [--mock MOCKS_TOML] < script.ndjson"
            );
            return ExitCode::from(2);
        }
    };
    let mut machine = match load(Path::new(&target)) {
        Ok(machine) => machine,
        Err(problems) => {
            eprintln!("{problems}");
            return ExitCode::from(2);
        }
    };
    let mut out = std::io::stdout().lock();
    machine.tap();
    let effects = machine.open();
    report(&mut out, &mut machine, effects, mocks.as_deref());
    for (number, line) in std::io::stdin().lock().lines().enumerate() {
        let line = line.unwrap_or_default();
        if line.trim().is_empty() {
            continue;
        }
        let effects = match step(&mut machine, &line) {
            Ok(effects) => effects,
            Err(why) => {
                eprintln!("line {}: {why}", number + 1);
                return ExitCode::from(1);
            }
        };
        report(&mut out, &mut machine, effects, mocks.as_deref());
    }
    print_line(&mut out, &json!({ "state": machine.snapshot() }));
    ExitCode::SUCCESS
}

/// Loads the app at `target` into a machine: its configs folder, or the `app.toml` at the folder's
/// top, which stands for the folder. Every `.toml` file in it, in any folder, is part of the app;
/// hidden ones (a name starting with `.`) aren't. Fails with every problem, one per line, each as
/// `FILE:LINE:COLUMN: ...` with the file's path in the folder.
fn load(target: &Path) -> Result<Machine, String> {
    let dir: &Path = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(Path::new("."))
    };
    let app = App::from_files(read_configs(dir)?).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        lines.join("\n")
    })?;
    Ok(Machine::new(app))
}

/// Every `.toml` file under `dir`, as its path from `dir` (with `/` between folders) and its text,
/// leaving out hidden files and folders. The same walk as `genesis-serve`'s.
fn read_configs(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries =
            std::fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
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

/// Runs one line of the script.
fn step(machine: &mut Machine, line: &str) -> Result<Vec<Effect>, String> {
    let input: Value = serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))?;
    if let Some(name) = input.get("fire").and_then(Value::as_str) {
        let payload = input.get("payload").cloned().unwrap_or_else(|| json!({}));
        return Ok(machine.fire(name, payload));
    }
    if let Some(ms) = input.get("advance").and_then(Value::as_u64) {
        return Ok(machine.advance(ms));
    }
    if let Some(yes) = input.get("answer").and_then(Value::as_bool) {
        let id = machine
            .get("sys.asking.0.id")
            .and_then(|id| id.as_u64())
            .ok_or_else(|| format!("no question is waiting for an answer, at {line}"))?;
        return Ok(machine.answer(id, yes));
    }
    Err(format!(
        r#"expected {{"fire": "NAME"}}, {{"advance": MILLISECONDS}}, or {{"answer": true}}, got {line}"#
    ))
}

/// Reads a mocks file.
fn load_mocks(file: &Path) -> Result<Vec<Mock>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mocks: Mocks =
        toml::from_str(&text).map_err(|e| format!("{}: {}", file.display(), e.message()))?;
    Ok(mocks.mock)
}

/// The answer to a call to `op`, from the first mock for it.
fn answer(mocks: &[Mock], op: &str) -> Reply {
    match mocks.iter().find(|m| m.op == op) {
        Some(Mock {
            error: Some(error), ..
        }) => Reply::Failed(error.clone()),
        Some(mock) => Reply::Response {
            status: mock.status,
            body: mock.body.clone(),
        },
        None => Reply::Failed(format!("the bench has no mock for {op}")),
    }
}

/// Prints the trail entries recorded since the last report, then the toasts, questions, and calls. With
/// mocks, each call is answered at once, and whatever follows is printed in turn.
fn report(
    out: &mut impl Write,
    machine: &mut Machine,
    mut effects: Vec<Effect>,
    mocks: Option<&[Mock]>,
) {
    loop {
        for entry in machine.take_tapped() {
            print_line(out, &entry);
        }
        if effects.is_empty() {
            return;
        }
        let mut next = Vec::new();
        for effect in effects {
            match effect {
                Effect::Toast { text, variant } => {
                    print_line(out, &json!({ "toast": text, "variant": variant.name() }));
                }
                Effect::Confirm { id, text } => {
                    print_line(out, &json!({ "confirm": text, "id": id }));
                }
                Effect::Call(call) => {
                    print_line(
                        out,
                        &json!({ "call": { "id": call.id, "op": call.op, "params": call.params } }),
                    );
                    if let Some(mocks) = mocks {
                        next.extend(machine.reply(call.id, answer(mocks, &call.op)));
                    }
                }
            }
        }
        effects = next;
    }
}

fn print_line(out: &mut impl Write, value: &Value) {
    // A closed stdout (the reader went away) isn't worth a panic; the run just ends quietly.
    let _ = writeln!(out, "{value}");
}
