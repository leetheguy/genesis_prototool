//! Checking an `svg` node's markup ([spec 040](../../../specs/040_catalog.md), "Media and embeds").
//!
//! An svg is drawn inline, so it could carry script. Its markup is read as XML at load and checked
//! against what an svg may hold: shapes, text, and paint, with no event attributes and no links out
//! of itself. Anything else is a config error, never silently dropped. What's drawn is Genesis's own
//! copy, written out from the checked tree (never the markup as written), so a difference between
//! how XML and HTML read it can't let anything through.
//!
//! ```
//! use genesis_core::svg::clean;
//!
//! assert_eq!(
//!     clean(r#"<svg viewBox="0 0 2 2"><circle r="1"/></svg>"#).unwrap(),
//!     r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 2" width="1em" height="1em"><circle r="1"></circle></svg>"#
//! );
//! assert!(clean("<svg onload=\"alert(1)\"/>").is_err());
//! ```

use roxmltree::{Document, Node};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const SVG: &str = "http://www.w3.org/2000/svg";
const XLINK: &str = "http://www.w3.org/1999/xlink";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

/// The elements an svg may hold: shapes, text, paint, and what groups them.
const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "symbol",
    "use",
    "title",
    "desc",
    "path",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "rect",
    "text",
    "tspan",
    "linearGradient",
    "radialGradient",
    "stop",
    "pattern",
    "clipPath",
    "mask",
    "marker",
];

/// Checks `markup` and writes out Genesis's own copy of it, or says what's wrong.
///
/// An svg with neither `width` nor `height` of its own is drawn 1em square, so the type keys' `size`
/// sizes it.
pub fn clean(markup: &str) -> Result<String, String> {
    let doc = Document::parse(markup).map_err(|e| format!("`svg` isn't well-formed XML: {e}"))?;
    let root = doc.root_element();
    let name = element_name(root);
    if name != "svg" {
        return Err(format!("`svg` is one <svg> element, not <{name}>"));
    }
    let mut out = String::new();
    write(root, &mut out, true)?;
    Ok(out)
}

/// An element's name as written: with its prefix (or namespace) if it isn't SVG's, so it can't pass
/// for one of [`ELEMENTS`].
fn element_name(node: Node) -> String {
    let tag = node.tag_name();
    match tag.namespace().filter(|ns| *ns != SVG) {
        Some(ns) => format!("{}:{}", node.lookup_prefix(ns).unwrap_or(ns), tag.name()),
        None => tag.name().to_string(),
    }
}

/// Writes `node` (an element) and everything in it, checking as it goes.
fn write(node: Node, out: &mut String, root: bool) -> Result<(), String> {
    let name = element_name(node);
    if !ELEMENTS.contains(&name.as_str()) {
        return Err(format!(
            "`svg` can't hold <{name}>; it draws shapes, text, and paint only"
        ));
    }
    out.push('<');
    out.push_str(&name);
    if root {
        out.push_str(&format!(" xmlns=\"{SVG}\""));
        let xlink = node
            .descendants()
            .flat_map(|n| n.attributes())
            .any(|a| a.namespace() == Some(XLINK));
        if xlink {
            out.push_str(&format!(" xmlns:xlink=\"{XLINK}\""));
        }
    }
    for attribute in node.attributes() {
        let prefix = match attribute.namespace() {
            None => "",
            Some(XLINK) => "xlink:",
            Some(XML) => "xml:",
            Some(ns) => {
                return Err(format!(
                    "`svg` can't hold {}:{} on <{name}>: only SVG's own attributes",
                    node.lookup_prefix(ns).unwrap_or(ns),
                    attribute.name()
                ));
            }
        };
        let (key, value) = (attribute.name(), attribute.value());
        check(&name, key, value)?;
        out.push_str(&format!(" {prefix}{key}=\"{}\"", escape(value, true)));
    }
    if root && !node.has_attribute("width") && !node.has_attribute("height") {
        out.push_str(" width=\"1em\" height=\"1em\"");
    }
    out.push('>');
    for child in node.children() {
        if child.is_element() {
            write(child, out, false)?;
        } else if child.is_text() {
            out.push_str(&escape(child.text().unwrap_or_default(), false));
        }
        // Comments and processing instructions aren't drawn.
    }
    out.push_str(&format!("</{name}>"));
    Ok(())
}

/// Refuses an attribute that could run something or reach outside the svg.
fn check(element: &str, key: &str, value: &str) -> Result<(), String> {
    if key.to_ascii_lowercase().starts_with("on") {
        return Err(format!(
            "`svg` can't hold {key} on <{element}>: no event attributes"
        ));
    }
    let squeezed: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    if squeezed.contains("javascript:") {
        return Err(format!(
            "`svg` can't hold a javascript: URL ({key} on <{element}>)"
        ));
    }
    let outside = match key {
        "href" => !value.starts_with('#'),
        "style" => squeezed
            .split("url(")
            .skip(1)
            .any(|rest| !rest.trim_start_matches(['"', '\'']).starts_with('#')),
        _ => false,
    };
    if outside {
        return Err(format!(
            "`svg` can't link outside itself: {key}=\"{value}\" on <{element}>"
        ));
    }
    Ok(())
}

/// `text` with what XML would read as markup escaped; quotes too, inside an attribute.
fn escape(text: &str, attribute: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// An `svg` node's markup, checked and cleaned as it's read ([`clean`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgMarkup(String);

impl SvgMarkup {
    /// The cleaned markup, ready to draw.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for SvgMarkup {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SvgMarkup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let markup = String::deserialize(deserializer)?;
        clean(&markup).map(SvgMarkup).map_err(de::Error::custom)
    }
}
