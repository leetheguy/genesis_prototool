//! `genesis-bench`: runs a Genesis app's event bus with no screen, for tests and agents.
//!
//! ```text
//! genesis-bench APP_DIR_OR_APP_TOML [--mock MOCKS_TOML] < script.ndjson
//! ```
//!
//! It loads the app, opens it (firing `app.opened`), then reads one JSON object per line from
//! stdin ([spec 100](../../../specs/100_events.md), "The workbench"):
//!
//! - `{"fire": "hello_btn.click", "payload": {...}}` fires an event (`payload` is optional).
//! - `{"advance": 250}` moves the clock on 250 milliseconds, running any timers that fall due.
//!
//! After each line it prints, one JSON object per line, every trail entry recorded (events with
//! their handlers, and `log` lines), then any toasts as `{"toast": "..."}`. At the end it prints
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
//! A call with no mock fails, saying so. Without `--mock`, calls are left waiting.
//!
//! It exits with `2` when the app doesn't load (each problem printed as `FILE:LINE:COLUMN: ...`),
//! and `1` when a line of the script can't be read.

use genesis_core::bus::{Effect, Machine, Reply};
use genesis_core::config::App;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
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

/// Loads the app at `target`, a folder holding `app.toml` or the file itself, into a machine.
/// Fails with every problem, one per line, each with its file and place.
fn load(target: &Path) -> Result<Machine, String> {
    let file: PathBuf = if target.is_dir() {
        target.join("app.toml")
    } else {
        target.to_path_buf()
    };
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let app = App::from_toml(&text).map_err(|problems| {
        let lines: Vec<String> = problems
            .iter()
            .map(|p| match (p.line, p.column) {
                (Some(line), Some(column)) => {
                    format!("{}:{line}:{column}: {}", file.display(), p.message)
                }
                _ => format!("{}: {}", file.display(), p.message),
            })
            .collect();
        lines.join("\n")
    })?;
    Ok(Machine::new(app))
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
    Err(format!(
        r#"expected {{"fire": "NAME"}} or {{"advance": MILLISECONDS}}, got {line}"#
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

/// Prints the trail entries recorded since the last report, then the toasts and calls. With
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
                Effect::Toast { text } => print_line(out, &json!({ "toast": text })),
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
