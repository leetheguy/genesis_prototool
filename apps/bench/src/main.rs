//! `genesis-bench`: runs a Genesis app's event bus with no screen, for tests and agents.
//!
//! ```text
//! genesis-bench APP_DIR_OR_APP_TOML < script.ndjson
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
//! It exits with `2` when the app doesn't load (each problem printed as `FILE:LINE:COLUMN: ...`),
//! and `1` when a line of the script can't be read.

use genesis_core::bus::{Effect, Machine};
use genesis_core::config::App;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(target) = std::env::args_os().nth(1) else {
        eprintln!("usage: genesis-bench APP_DIR_OR_APP_TOML < script.ndjson");
        return ExitCode::from(2);
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
    report(&mut out, &mut machine, effects);
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
        report(&mut out, &mut machine, effects);
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

/// Prints the trail entries recorded since the last report, then the toasts.
fn report(out: &mut impl Write, machine: &mut Machine, effects: Vec<Effect>) {
    for entry in machine.take_tapped() {
        print_line(out, &entry);
    }
    for Effect::Toast { text } in effects {
        print_line(out, &json!({ "toast": text }));
    }
}

fn print_line(out: &mut impl Write, value: &Value) {
    // A closed stdout (the reader went away) isn't worth a panic; the run just ends quietly.
    let _ = writeln!(out, "{value}");
}
