//! Style keys as Tailwind and daisyUI classes ([spec 040](../../../specs/040_catalog.md), "Style keys").
//!
//! genesis-core holds each key's closed list; this turns a value into its classes. The CSS is built
//! ahead of time, so every class a key can give is listed in [`vocabulary`], which is written to
//! `crates/genesis-ui/classes.txt` for Tailwind to read (a test keeps the two the same).

use genesis_core::style::{
    Boxed, BySize, Color, Columns, Direction, Height, Layout, Line, Margin, Placed, SCREENS, Span,
    StyleKey, TextColor, Typed, Vocab, keys,
};

/// The classes a key gives for each of its values, space-separated, before any screen prefix.
pub trait Classes: StyleKey {
    /// The classes for `value`.
    fn of(value: Self::Value) -> String;
}

/// `classes` for `screen`: each one with the screen's prefix (`md:gap-4`), or as it is for `base`.
fn at_screen(screen: &str, classes: &str) -> Vec<String> {
    classes
        .split_whitespace()
        .map(|class| match screen {
            "base" => class.to_string(),
            _ => format!("{screen}:{class}"),
        })
        .collect()
}

/// Every class `key` gives, at every screen size it names.
pub fn classes<K: Classes>(key: &K) -> String {
    key.sizes()
        .each()
        .flat_map(|(screen, value)| at_screen(screen, &K::of(value)))
        .collect::<Vec<_>>()
        .join(" ")
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
    fn of(value: Self::Value) -> String {
        match value {
            Direction::Column => "flex flex-col",
            Direction::Row => "flex flex-row",
        }
        .into()
    }
}

impl Classes for keys::Grid {
    fn of(value: Self::Value) -> String {
        match value {
            Columns::Count(n) => format!("grid grid-cols-{n}"),
            Columns::Fit => "grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))]".into(),
        }
    }
}

impl Classes for keys::Gap {
    fn of(value: Self::Value) -> String {
        format!("gap-{}", value.0)
    }
}

impl Classes for keys::Align {
    fn of(value: Self::Value) -> String {
        format!("items-{}", value.word())
    }
}

impl Classes for keys::Justify {
    fn of(value: Self::Value) -> String {
        format!("justify-{}", value.word())
    }
}

impl Classes for keys::Wrap {
    fn of(value: Self::Value) -> String {
        if value { "flex-wrap" } else { "flex-nowrap" }.into()
    }
}

impl Classes for keys::Pad {
    fn of(value: Self::Value) -> String {
        format!("p-{}", value.0)
    }
}

impl Classes for keys::PadX {
    fn of(value: Self::Value) -> String {
        format!("px-{}", value.0)
    }
}

impl Classes for keys::PadY {
    fn of(value: Self::Value) -> String {
        format!("py-{}", value.0)
    }
}

impl Classes for keys::Margin {
    fn of(value: Self::Value) -> String {
        format!("m-{}", margin(value))
    }
}

impl Classes for keys::MarginX {
    fn of(value: Self::Value) -> String {
        format!("mx-{}", margin(value))
    }
}

impl Classes for keys::MarginY {
    fn of(value: Self::Value) -> String {
        format!("my-{}", margin(value))
    }
}

impl Classes for keys::Bg {
    fn of(value: Self::Value) -> String {
        format!("bg-{}", color(value))
    }
}

impl Classes for keys::Border {
    fn of(value: Self::Value) -> String {
        match value {
            Line::Plain => "border border-base-300".into(),
            Line::Color(c) => format!("border border-{}", color(c)),
        }
    }
}

impl Classes for keys::Radius {
    fn of(value: Self::Value) -> String {
        format!("rounded-{}", value.word())
    }
}

impl Classes for keys::Shadow {
    fn of(value: Self::Value) -> String {
        format!("shadow-{}", value.word())
    }
}

impl Classes for keys::Height {
    fn of(value: Self::Value) -> String {
        match value {
            Height::Full => "h-full".into(),
            Height::Screen => "h-screen".into(),
            Height::Rem(n) => format!("h-[{n}rem]"),
        }
    }
}

impl Classes for keys::Scroll {
    fn of(value: Self::Value) -> String {
        if value {
            "overflow-auto"
        } else {
            "overflow-visible"
        }
        .into()
    }
}

impl Classes for keys::Size {
    fn of(value: Self::Value) -> String {
        format!("text-{}", value.word())
    }
}

impl Classes for keys::Weight {
    fn of(value: Self::Value) -> String {
        format!("font-{}", value.word())
    }
}

impl Classes for keys::Color {
    fn of(value: Self::Value) -> String {
        match value {
            TextColor::Muted => "text-base-content/60".into(),
            TextColor::Color(c) => format!("text-{}", color(c)),
        }
    }
}

impl Classes for keys::TextAlign {
    fn of(value: Self::Value) -> String {
        format!("text-{}", value.word())
    }
}

impl Classes for keys::Font {
    fn of(value: Self::Value) -> String {
        format!("font-{}", value.word())
    }
}

impl Classes for keys::Span {
    fn of(value: Self::Value) -> String {
        match value {
            Span::Count(n) => format!("col-span-{n}"),
            Span::Full => "col-span-full".into(),
        }
    }
}

impl Classes for keys::Offset {
    fn of(value: Self::Value) -> String {
        // Skipping n columns is starting at column n + 1.
        format!("col-start-{}", value.0 + 1)
    }
}

impl Classes for keys::Grow {
    fn of(value: Self::Value) -> String {
        if value { "grow" } else { "grow-0" }.into()
    }
}

impl Classes for keys::Width {
    fn of(value: Self::Value) -> String {
        format!("w-{}", value.word())
    }
}

impl Classes for keys::MaxWidth {
    fn of(value: Self::Value) -> String {
        format!("max-w-{}", value.word())
    }
}

impl Classes for keys::Variant {
    fn of(value: Self::Value) -> String {
        format!("btn-{}", value.word())
    }
}

impl Classes for keys::ButtonSize {
    fn of(value: Self::Value) -> String {
        format!("btn-{}", value.word())
    }
}

impl Classes for keys::LineColor {
    fn of(value: Self::Value) -> String {
        format!("border-{}", color(value))
    }
}

/// Collects classes, skipping keys that aren't set.
#[derive(Default)]
pub(crate) struct ClassList(Vec<String>);

impl ClassList {
    /// Adds `key`'s classes, if it's set.
    pub(crate) fn add<K: Classes>(&mut self, key: Option<K>) -> &mut Self {
        if let Some(key) = key {
            self.0.push(classes(&key));
        }
        self
    }

    /// Adds `classes` as they are: a component's own daisyUI classes.
    pub(crate) fn raw(&mut self, classes: &str) -> &mut Self {
        self.0.push(classes.into());
        self
    }

    /// The placement keys.
    pub(crate) fn placed(&mut self, p: Placed) -> &mut Self {
        self.add(p.span)
            .add(p.offset)
            .add(p.grow)
            .add(p.width)
            .add(p.max_width)
    }

    /// A cell's layout: a flex column or row, until a screen size where it's a grid.
    pub(crate) fn layout(&mut self, l: Layout) -> &mut Self {
        if l.grid.and_then(|g| g.0.at("base")).is_none() {
            let flow = l.flow.map_or(BySize::base(Direction::Column), |f| {
                f.0.or_base(Direction::Column)
            });
            self.add(Some(keys::Flow(flow)));
        }
        self.add(l.grid)
            .add(l.gap)
            .add(l.align)
            .add(l.justify)
            .add(l.wrap)
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

/// Every class key `K` can give, on every screen size.
fn every<K: Classes>() -> impl Iterator<Item = String> {
    SCREENS.into_iter().flat_map(|screen| {
        K::Value::all()
            .into_iter()
            .flat_map(move |v| at_screen(screen, &K::of(v)))
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
        every::<keys::Span>().collect(),
        every::<keys::Offset>().collect(),
        every::<keys::Grow>().collect(),
        every::<keys::Width>().collect(),
        every::<keys::MaxWidth>().collect(),
        every::<keys::Variant>().collect(),
        every::<keys::ButtonSize>().collect(),
        every::<keys::LineColor>().collect(),
    ];
    let mut seen = std::collections::BTreeSet::new();
    all.into_iter()
        .flatten()
        .filter(|class| seen.insert(class.clone()))
        .collect()
}
