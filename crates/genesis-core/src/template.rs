//! Templates: `{{ name.path }}` lookups, and nothing more.
//!
//! A template names a value by a dotted path (`app.contact.name`, `event.row.id`) and is replaced
//! by it ([spec 020](../../../specs/020_config.md), "Templates"). It's a lookup, never an
//! expression: no filters, no operators. Where to look is up to the caller, which passes a
//! `look` function from a path to a value.
//!
//! - [`text`] fills templates into display text. A missing value shows as empty.
//! - [`value`] fills templates into a value that will be used (params, a `set`, a payload). A
//!   string that's only one template keeps the value's type, and a missing value is an error.
//! - [`check`] says whether every template in a string is a plain lookup, for loading checks.
//!
//! ```
//! use genesis_core::template;
//! use serde_json::{json, Value};
//!
//! let look = |path: &str| (path == "app.n").then(|| json!(3));
//! assert_eq!(template::text("n is {{ app.n }}", look), "n is 3");
//! assert_eq!(template::value(&json!("{{ app.n }}"), look), Ok(json!(3)));
//! ```

use serde_json::Value;

/// One piece of a string: plain text, or a template's path.
enum Piece<'a> {
    Text(&'a str),
    Lookup(&'a str),
}

/// Fills every template in `text` for display: values as plain text, a missing value as empty.
pub fn text(text: &str, look: impl Fn(&str) -> Option<Value>) -> String {
    pieces(text)
        .map(|piece| match piece {
            Piece::Text(t) => t.to_string(),
            Piece::Lookup(path) => look(path).map(|v| as_text(&v)).unwrap_or_default(),
        })
        .collect()
}

/// Fills every template inside `template`, through tables and lists.
///
/// A string that's exactly one template becomes the value itself, type and all; a string with
/// templates among other text becomes text. Fails on the first missing value, naming it.
pub fn value(
    template: &Value,
    look: impl Fn(&str) -> Option<Value> + Copy,
) -> Result<Value, String> {
    match template {
        Value::String(s) => fill_string(s, look),
        Value::Array(items) => items
            .iter()
            .map(|item| value(item, look))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        Value::Object(fields) => fields
            .iter()
            .map(|(k, v)| value(v, look).map(|v| (k.clone(), v)))
            .collect::<Result<_, _>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

/// The path of every template in `text`, in order: `"{{ app.page }} of {{ app.pages }}"` has
/// `app.page` and `app.pages`.
pub fn paths(text: &str) -> Vec<&str> {
    pieces(text)
        .filter_map(|piece| match piece {
            Piece::Lookup(path) => Some(path),
            Piece::Text(_) => None,
        })
        .collect()
}

/// Whether every `{{ ... }}` in `text` is a plain lookup: dot-separated names of letters, digits,
/// and `_`. Fails naming the first one that isn't.
pub fn check(text: &str) -> Result<(), String> {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err(format!("`{}` has no closing `}}}}`", &rest[start..]));
        };
        let inner = after[..end].trim();
        if !is_path(inner) {
            return Err(format!(
                "`{{{{ {inner} }}}}` isn't a plain lookup like `{{{{ app.name }}}}`"
            ));
        }
        rest = &after[end + 2..];
    }
    Ok(())
}

fn fill_string(s: &str, look: impl Fn(&str) -> Option<Value>) -> Result<Value, String> {
    let found = |path: &str| look(path).ok_or_else(|| format!("{path} isn't set"));
    let parts: Vec<Piece> = pieces(s).collect();
    if let [Piece::Lookup(path)] = parts.as_slice() {
        return found(path);
    }
    let mut out = String::new();
    for piece in parts {
        match piece {
            Piece::Text(t) => out.push_str(t),
            Piece::Lookup(path) => out.push_str(&as_text(&found(path)?)),
        }
    }
    Ok(Value::String(out))
}

/// Splits a string into text and templates. A `{{` with no `}}`, or whose inside isn't a plain
/// path, stays as text: [`check`] reports those when the app loads.
fn pieces(s: &str) -> impl Iterator<Item = Piece<'_>> {
    let mut rest = s;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        if let Some(start) = rest.find("{{")
            && let Some(len) = rest[start + 2..].find("}}")
            && is_path(rest[start + 2..start + 2 + len].trim())
        {
            if start > 0 {
                let text = &rest[..start];
                rest = &rest[start..];
                return Some(Piece::Text(text));
            }
            let path = rest[2..2 + len].trim();
            rest = &rest[2 + len + 2..];
            return Some(Piece::Lookup(path));
        }
        let text = rest;
        rest = "";
        Some(Piece::Text(text))
    })
}

fn is_path(s: &str) -> bool {
    !s.is_empty()
        && s.split('.')
            .all(|name| !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// A value as text: strings as they are, `null` as empty, everything else as compact JSON.
pub fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
