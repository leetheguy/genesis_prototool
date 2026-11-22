//! Event patterns: how a handler says which events it listens for.
//!
//! A pattern is an event name that may hold two wildcards, the way a shell matches file names
//! ([spec 100](../../../specs/100_events.md), "Handlers"):
//!
//! - `*` matches any run of characters, dots included, or none at all.
//! - `?` matches exactly one character.
//!
//! Everything else matches only itself, case and all.
//!
//! ```
//! use genesis_core::pattern::matches;
//!
//! assert!(matches("*.failed", "auth.logout.failed"));
//! assert!(matches("contacts.4??", "contacts.404"));
//! assert!(!matches("contacts.4??", "contacts.200"));
//! ```

/// Whether the event `name` matches `pattern`.
pub fn matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n) = (0, 0);
    // The last `*` seen: where the pattern carries on after it, and where in the name it last
    // stopped taking characters. On a mismatch the `*` takes one more and matching tries again.
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                p += 1;
                star = Some((p, n));
            }
            Some(&c) if c == '?' || c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((after, taken)) => {
                    let taken = taken + 1;
                    star = Some((after, taken));
                    p = after;
                    n = taken;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}
