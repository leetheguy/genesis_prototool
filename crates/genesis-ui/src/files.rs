//! The files a `file` control picked ([spec 040](../../../specs/040_catalog.md), "file"). The
//! browser keeps each one here, under the ID its reference carries, for as long as the page is
//! open. State only ever holds the reference; whatever sends a call reads the bytes from here.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HELD: RefCell<HashMap<String, web_sys::File>> = RefCell::new(HashMap::new());
}

/// Keeps `file` and gives back its reference: `{ file, name, type, size }`, the same shape as
/// PurpleFlow's. Its ID is a fresh UUID, so a reference kept from before a reload never finds
/// another file.
pub fn hold(file: web_sys::File) -> serde_json::Value {
    let id = web_sys::window()
        .and_then(|w| w.crypto().ok())
        .map(|c| c.random_uuid())
        .unwrap_or_default();
    let kind = match file.type_() {
        kind if kind.is_empty() => "application/octet-stream".to_string(),
        kind => kind,
    };
    let reference = serde_json::json!({
        "file": id, "name": file.name(), "type": kind, "size": file.size() as u64,
    });
    HELD.with(|held| held.borrow_mut().insert(id, file));
    reference
}

/// The file held under `id`, while its page is open.
pub fn get(id: &str) -> Option<web_sys::File> {
    HELD.with(|held| held.borrow().get(id).cloned())
}
