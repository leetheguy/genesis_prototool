//! serve's log ([spec 110](../../../specs/110_agents.md), "The log"): every failure, one line each,
//! in order, in `genesis.log` in the logs folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};

/// The log's file name in the logs folder.
pub const FILE: &str = "genesis.log";

/// The log: where it is, how many lines it keeps, and whether the last load failed.
pub(crate) struct Log {
    file: PathBuf,
    /// `[app] log_lines` as the app last loaded clean.
    keep: AtomicUsize,
    /// Whether the last logged load failed, so the next clean one logs `load ok`. Held while
    /// writing, so lines never interleave.
    failed: Mutex<bool>,
}

impl Log {
    pub(crate) fn new(folder: &Path) -> Log {
        Log {
            file: folder.join(FILE),
            keep: AtomicUsize::new(genesis_core::config::LOG_LINES),
            failed: Mutex::new(false),
        }
    }

    /// Keeps the last `lines` lines from now on.
    pub(crate) fn keep(&self, lines: usize) {
        self.keep.store(lines, Ordering::Relaxed);
    }

    /// The whole log, or nothing before anything is logged.
    pub(crate) async fn read(&self) -> std::io::Result<String> {
        let _held = self.failed.lock().await;
        match tokio::fs::read_to_string(&self.file).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            read => read,
        }
    }

    /// Logs a load: each of its problems, or `load ok` if it's clean after one that wasn't.
    pub(crate) async fn load(&self, problems: Option<&str>) {
        let mut failed = self.failed.lock().await;
        let lines: Vec<String> = match problems {
            Some(problems) => problems.lines().map(str::to_string).collect(),
            None if *failed => vec!["ok".into()],
            None => vec![],
        };
        *failed = problems.is_some();
        self.append(&failed, "load", &lines).await;
    }

    /// Logs one entry of `kind` (`call`, `run`, `push`).
    pub(crate) async fn write(&self, kind: &str, text: &str) {
        let held = self.failed.lock().await;
        self.append(&held, kind, &[text.to_string()]).await;
    }

    /// Adds `lines` to the end of the log, keeping its last `keep` lines. Takes the lock's guard,
    /// so it's only called while holding it.
    async fn append(&self, _held: &MutexGuard<'_, bool>, kind: &str, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let now = now();
        let new = lines
            .iter()
            .map(|line| format!("{now} {kind:<4} {}", one_line(line)));
        if let Err(e) = self.add(new).await {
            // Nowhere else to say it: serve's own output.
            eprintln!("genesis: can't write the log {}: {e}", self.file.display());
        }
    }

    async fn add(&self, new: impl Iterator<Item = String>) -> std::io::Result<()> {
        if let Some(folder) = self.file.parent() {
            tokio::fs::create_dir_all(folder).await?;
        }
        let old = match tokio::fs::read_to_string(&self.file).await {
            Ok(old) => old,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let lines: Vec<String> = old.lines().map(str::to_string).chain(new).collect();
        let keep = self.keep.load(Ordering::Relaxed);
        let kept = &lines[lines.len().saturating_sub(keep)..];
        let text = match kept.is_empty() {
            true => String::new(),
            false => kept.join("\n") + "\n",
        };
        crate::write_whole(&self.file, &text).await
    }
}

/// `text` on one line: each line break a space.
fn one_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether `text` is a time in the log's own form, `2026-11-24T10:00:00Z`.
pub(crate) fn is_time(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b'T',
            13 | 16 => c == b':',
            19 => c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// Now, in UTC, to the second: `2026-11-24T10:02:11Z`. Lines in this form sort by time as text.
pub(crate) fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    stamp(secs)
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ` (Howard Hinnant's days-to-civil).
fn stamp(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_read_as_utc() {
        assert_eq!(stamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(stamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(stamp(1_795_515_731), "2026-11-24T10:22:11Z");
    }

    #[test]
    fn a_time_is_in_the_logs_own_form() {
        assert!(is_time("2026-11-24T10:00:00Z"));
        for bad in [
            "2026-11-24",
            "2026-11-24T10:00:00",
            "2026-11-24 10:00:00Z",
            "2026-11-24T10:00:0xZ",
        ] {
            assert!(!is_time(bad), "{bad}");
        }
    }

    #[test]
    fn an_entry_is_one_line() {
        assert_eq!(one_line("a\nb\r\nc"), "a b c");
    }
}
