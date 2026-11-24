//! Style keys as Tailwind and daisyUI classes ([spec 040](../../../specs/040_catalog.md), "Style keys").
//!
//! genesis-core holds each key's closed list; this turns a value into its classes. The CSS is built
//! ahead of time, so every class a key can give is listed in [`vocabulary`], which is written to
//! `crates/genesis-ui/classes.txt` for Tailwind to read (a test keeps the two the same).

use genesis_core::style::{
    Boxed, Color, Columns, Direction, Layout, Line, Margin, StyleKey, TextColor, Typed, Vocab, keys,
};

/// The classes one key's value gives, space-separated.
pub trait Classes {
    /// Its classes.
    fn classes(&self) -> String;
}

/// A theme color's name, as daisyUI's classes write it (`bg-base-200`).
fn color(c: Color) -> &'static str {
    c.word()
}

/// A margin's class suffix: a spacing step, or `auto`.
fn margin(m: Margin) -> String {
    match m {
        Margin::Space(space) => space.0.to_string(),
        Margin::Auto => "auto".into(),
    }
}

impl Classes for keys::Flow {
    fn classes(&self) -> String {
        match self.0 {
            Direction::Column => "flex flex-col",
            Direction::Row => "flex flex-row",
        }
        .into()
    }
}

impl Classes for keys::Grid {
    fn classes(&self) -> String {
        match self.0 {
            Columns::Count(n) => format!("grid grid-cols-{n}"),
            Columns::Fit => "grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))]".into(),
        }
    }
}

impl Classes for keys::Gap {
    fn classes(&self) -> String {
        format!("gap-{}", self.0.0)
    }
}

impl Classes for keys::Align {
    fn classes(&self) -> String {
        format!("items-{}", self.0.word())
    }
}

impl Classes for keys::Justify {
    fn classes(&self) -> String {
        format!("justify-{}", self.0.word())
    }
}

impl Classes for keys::Wrap {
    fn classes(&self) -> String {
        if self.0 { "flex-wrap" } else { "" }.into()
    }
}

impl Classes for keys::Pad {
    fn classes(&self) -> String {
        format!("p-{}", self.0.0)
    }
}

impl Classes for keys::PadX {
    fn classes(&self) -> String {
        format!("px-{}", self.0.0)
    }
}

impl Classes for keys::PadY {
    fn classes(&self) -> String {
        format!("py-{}", self.0.0)
    }
}

impl Classes for keys::Margin {
    fn classes(&self) -> String {
        format!("m-{}", margin(self.0))
    }
}

impl Classes for keys::MarginX {
    fn classes(&self) -> String {
        format!("mx-{}", margin(self.0))
    }
}

impl Classes for keys::MarginY {
    fn classes(&self) -> String {
        format!("my-{}", margin(self.0))
    }
}

impl Classes for keys::Bg {
    fn classes(&self) -> String {
        format!("bg-{}", color(self.0))
    }
}

impl Classes for keys::Border {
    fn classes(&self) -> String {
        match self.0 {
            Line::Plain => "border border-base-300".into(),
            Line::Color(c) => format!("border border-{}", color(c)),
        }
    }
}

impl Classes for keys::Radius {
    fn classes(&self) -> String {
        format!("rounded-{}", self.0.word())
    }
}

impl Classes for keys::Shadow {
    fn classes(&self) -> String {
        format!("shadow-{}", self.0.word())
    }
}

impl Classes for keys::Height {
    fn classes(&self) -> String {
        format!("h-{}", self.0.word())
    }
}

impl Classes for keys::Scroll {
    fn classes(&self) -> String {
        if self.0 { "overflow-auto" } else { "" }.into()
    }
}

impl Classes for keys::Size {
    fn classes(&self) -> String {
        format!("text-{}", self.0.word())
    }
}

impl Classes for keys::Weight {
    fn classes(&self) -> String {
        format!("font-{}", self.0.word())
    }
}

impl Classes for keys::Color {
    fn classes(&self) -> String {
        match self.0 {
            TextColor::Muted => "text-base-content/60".into(),
            TextColor::Color(c) => format!("text-{}", color(c)),
        }
    }
}

impl Classes for keys::TextAlign {
    fn classes(&self) -> String {
        format!("text-{}", self.0.word())
    }
}

impl Classes for keys::Font {
    fn classes(&self) -> String {
        format!("font-{}", self.0.word())
    }
}

/// Collects classes, skipping keys that aren't set or give none.
#[derive(Default)]
pub(crate) struct ClassList(Vec<String>);

impl ClassList {
    /// Adds `key`'s classes, if it's set.
    pub(crate) fn add<K: Classes>(&mut self, key: Option<K>) -> &mut Self {
        if let Some(classes) = key.map(|k| k.classes()).filter(|c| !c.is_empty()) {
            self.0.push(classes);
        }
        self
    }

    /// A cell's layout: a grid when it has one, or else a flex column or row.
    pub(crate) fn layout(&mut self, l: Layout) -> &mut Self {
        match l.grid {
            Some(grid) => self.add(Some(grid)),
            None => self.add(Some(l.flow.unwrap_or(keys::Flow(Direction::Column)))),
        };
        self.add(l.gap).add(l.align).add(l.justify).add(l.wrap)
    }

    /// The box keys.
    pub(crate) fn boxed(&mut self, b: Boxed) -> &mut Self {
        self.add(b.pad).add(b.pad_x).add(b.pad_y);
        self.add(b.margin).add(b.margin_x).add(b.margin_y);
        self.add(b.bg).add(b.border).add(b.radius).add(b.shadow);
        self.add(b.height).add(b.scroll)
    }

    /// The type keys.
    pub(crate) fn typed(&mut self, t: Typed) -> &mut Self {
        self.add(t.size)
            .add(t.weight)
            .add(t.color)
            .add(t.text_align)
            .add(t.font)
    }

    /// The `class` attribute.
    pub(crate) fn attr(&self) -> String {
        self.0.join(" ")
    }
}

/// Every class key `K` can give.
fn every<K: StyleKey + Classes>() -> impl Iterator<Item = String> {
    K::Value::all().into_iter().flat_map(|v| {
        K::new(v)
            .classes()
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
    })
}

/// Every class a style key can give, each once, in the order the keys are listed in spec 040.
///
/// The CSS is built from this list (by way of `classes.txt`), so a value's classes are always there.
pub fn vocabulary() -> Vec<String> {
    let all = [
        every::<keys::Flow>().collect::<Vec<_>>(),
        every::<keys::Grid>().collect(),
        every::<keys::Gap>().collect(),
        every::<keys::Align>().collect(),
        every::<keys::Justify>().collect(),
        every::<keys::Wrap>().collect(),
        every::<keys::Pad>().collect(),
        every::<keys::PadX>().collect(),
        every::<keys::PadY>().collect(),
        every::<keys::Margin>().collect(),
        every::<keys::MarginX>().collect(),
        every::<keys::MarginY>().collect(),
        every::<keys::Bg>().collect(),
        every::<keys::Border>().collect(),
        every::<keys::Radius>().collect(),
        every::<keys::Shadow>().collect(),
        every::<keys::Height>().collect(),
        every::<keys::Scroll>().collect(),
        every::<keys::Size>().collect(),
        every::<keys::Weight>().collect(),
        every::<keys::Color>().collect(),
        every::<keys::TextAlign>().collect(),
        every::<keys::Font>().collect(),
    ];
    let mut seen = std::collections::BTreeSet::new();
    all.into_iter()
        .flatten()
        .filter(|class| seen.insert(class.clone()))
        .collect()
}
