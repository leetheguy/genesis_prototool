//! File references ([spec 040](../../../specs/040_catalog.md), "A `file` works only inside a
//! `form`"). A `file` control's value never holds the bytes. It's a reference,
//! `{ file, name, type, size }`, where `file` is an ID for the bytes the browser holds. A call whose
//! params hold one goes out as `multipart/form-data`, with the bytes as a part.

use serde_json::{Map, Value};

/// The biggest file a `file` control takes unless its `max_size` says otherwise: 100 MB, as in
/// PurpleFlow.
pub const MAX_SIZE: u64 = 100_000_000;

/// The ID in a file reference: `Some` when `value` is `{ file, name, type, size }` with a text
/// `file`, and `None` for anything else.
pub fn reference(value: &Value) -> Option<&str> {
    let fields = value.as_object()?;
    let shaped = fields.len() == 4
        && ["name", "type", "size"]
            .iter()
            .all(|key| fields.contains_key(*key));
    shaped.then(|| fields["file"].as_str()).flatten()
}

/// The ID of every file reference in a call's params: a param that is one, or a list of them.
pub fn references(params: &Map<String, Value>) -> Vec<&str> {
    params
        .values()
        .flat_map(|value| match value {
            Value::Array(items) => items.iter().filter_map(reference).collect::<Vec<_>>(),
            other => reference(other).into_iter().collect(),
        })
        .collect()
}

/// Why a file of `size` bytes is too big for a control taking at most `max` (`0` for no limit),
/// naming both sizes; `None` when it fits.
pub fn too_big(name: &str, size: u64, max: u64) -> Option<String> {
    (max > 0 && size > max).then(|| {
        format!(
            "{name} is {}; this takes at most {}.",
            size_text(size),
            size_text(max)
        )
    })
}

/// A size in words: `512 bytes`, `800 KB`, `12.3 MB`.
pub fn size_text(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} bytes")
    } else if b < KB * KB {
        format!("{} KB", trim(b / KB))
    } else {
        format!("{} MB", trim(b / KB / KB))
    }
}

/// A number with at most one decimal, and none when it's whole.
fn trim(n: f64) -> String {
    let text = format!("{n:.1}");
    text.strip_suffix(".0").map(String::from).unwrap_or(text)
}
