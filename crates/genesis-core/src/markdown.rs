//! Markdown, drawn safely ([spec 040](../../../specs/040_catalog.md), "markdown"). CommonMark with
//! tables, strikethrough, and task lists becomes HTML, but nothing in it can run or reach outside
//! the page on its own: raw HTML is shown as text, and a link or image whose address isn't
//! `http`, `https`, `mailto`, `tel`, or relative is dropped, leaving its words.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

/// The HTML for `text`, as Markdown.
pub fn html(text: &str) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    // Whether the link or image now open was dropped, so its end is dropped too. Neither nests.
    let (mut link_dropped, mut image_dropped) = (false, false);
    let events = Parser::new_ext(text, options).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Link { ref dest_url, .. }) => {
            link_dropped = !safe(dest_url);
            (!link_dropped).then_some(event)
        }
        Event::End(TagEnd::Link) => (!std::mem::take(&mut link_dropped)).then_some(event),
        Event::Start(Tag::Image { ref dest_url, .. }) => {
            image_dropped = !safe(dest_url);
            (!image_dropped).then_some(event)
        }
        Event::End(TagEnd::Image) => (!std::mem::take(&mut image_dropped)).then_some(event),
        other => Some(other),
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// Whether a link or image may go to `url`: one of a few schemes, or none (a relative address).
pub fn safe(url: &str) -> bool {
    // Browsers skip whitespace and control characters in a scheme, so `java\tscript:` counts.
    let url: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    let scheme_end = url.find(':');
    let path_start = url.find(['/', '?', '#']);
    match (scheme_end, path_start) {
        (Some(colon), Some(path)) if path < colon => true,
        (Some(colon), _) => ["http", "https", "mailto", "tel"].contains(&&url[..colon]),
        (None, _) => true,
    }
}
