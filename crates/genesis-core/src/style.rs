//! The closed style keys ([spec 040](../../../specs/040_catalog.md), "Style keys").
//!
//! Each key takes a word or number from a closed list, never a CSS class. The lists are here, so a
//! value outside one is a config error at load, naming the key and every value it takes. genesis-ui
//! turns the values into Tailwind classes.
//!
//! Each key is its own small type in [`keys`] (`keys::Gap(Space)`), so it knows its name for errors,
//! and the node types take them in groups: [`Layout`], [`Boxed`], and [`Typed`].
//!
//! ```
//! use genesis_core::style::{Space, Vocab, keys::Gap};
//!
//! let gap: Gap = serde_json::from_str("4").unwrap();
//! assert_eq!(gap, Gap(Space(4)));
//! let bad = serde_json::from_str::<Gap>("7").unwrap_err().to_string();
//! assert!(bad.starts_with("`gap` is 0, 1, 2, 3, 4, 5, 6, 8, 10, 12, or 16, not 7"));
//! ```

use serde_json::Value;

/// A closed list of values: what one or more style keys take.
pub trait Vocab: Sized {
    /// Every value, in the order an error lists them.
    fn all() -> Vec<Self>;
    /// How it's written in TOML (and JSON).
    fn to_value(&self) -> Value;
}

/// A style key: its name, and the list its value comes from.
pub trait StyleKey: Sized {
    /// The key as the TOML writes it.
    const NAME: &'static str;
    /// The list it takes a value from.
    type Value: Vocab;
    /// The key holding `value`.
    fn new(value: Self::Value) -> Self;
}

/// Reads `value` for the key `key`: one from `T`'s list, or a message naming the key and the list.
pub fn read<T: Vocab>(key: &str, value: &Value) -> Result<T, String> {
    T::all()
        .into_iter()
        .find(|v| &v.to_value() == value)
        .ok_or_else(|| format!("`{key}` is {}, not {value}", takes::<T>()))
}

/// `T`'s list as an error writes it: `"column" or "row"`, `1, 2, or 3`.
fn takes<T: Vocab>() -> String {
    let all: Vec<String> = T::all().iter().map(|v| v.to_value().to_string()).collect();
    match all.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} or {b}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

impl Vocab for bool {
    fn all() -> Vec<Self> {
        vec![true, false]
    }
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}

/// An enum of words, each written as a string.
macro_rules! words {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $word:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $(#[doc = concat!("`\"", $word, "\"`")] $variant),*
        }

        impl $name {
            /// The word as the TOML writes it.
            pub fn word(self) -> &'static str {
                match self {
                    $(Self::$variant => $word),*
                }
            }
        }

        impl Vocab for $name {
            fn all() -> Vec<Self> {
                vec![$(Self::$variant),*]
            }
            fn to_value(&self) -> Value {
                Value::from(self.word())
            }
        }
    };
}

/// A step of Tailwind's spacing scale (each step is 0.25rem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space(pub u8);

impl Vocab for Space {
    fn all() -> Vec<Self> {
        [0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 16].map(Space).to_vec()
    }
    fn to_value(&self) -> Value {
        Value::from(self.0)
    }
}

words! {
    /// One of daisyUI's theme colors, so every theme recolors every screen.
    Color {
        Base100 = "base-100",
        Base200 = "base-200",
        Base300 = "base-300",
        BaseContent = "base-content",
        Neutral = "neutral",
        NeutralContent = "neutral-content",
        Primary = "primary",
        PrimaryContent = "primary-content",
        Secondary = "secondary",
        SecondaryContent = "secondary-content",
        Accent = "accent",
        AccentContent = "accent-content",
        Info = "info",
        InfoContent = "info-content",
        Success = "success",
        SuccessContent = "success-content",
        Warning = "warning",
        WarningContent = "warning-content",
        Error = "error",
        ErrorContent = "error-content",
    }
}

words! {
    /// Which way a cell lays out its children.
    Direction { Column = "column", Row = "row" }
}

/// A grid's columns: a count, or as many as fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
    /// `1`–`12` equal columns.
    Count(u8),
    /// `"fit"`: as many columns of at least 6rem as there's room for.
    Fit,
}

impl Vocab for Columns {
    fn all() -> Vec<Self> {
        (1..=12).map(Columns::Count).chain([Columns::Fit]).collect()
    }
    fn to_value(&self) -> Value {
        match self {
            Columns::Count(n) => Value::from(*n),
            Columns::Fit => Value::from("fit"),
        }
    }
}

words! {
    /// How children line up across the flow.
    Alignment { Start = "start", Center = "center", End = "end", Stretch = "stretch" }
}

words! {
    /// How children spread along the flow.
    Justification {
        Start = "start",
        Center = "center",
        End = "end",
        Between = "between",
        Around = "around",
    }
}

/// A margin: a step of spacing, or `"auto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Margin {
    /// A step of spacing.
    Space(Space),
    /// `"auto"`: takes what's left (so `margin_x = "auto"` centers).
    Auto,
}

impl Vocab for Margin {
    fn all() -> Vec<Self> {
        Space::all()
            .into_iter()
            .map(Margin::Space)
            .chain([Margin::Auto])
            .collect()
    }
    fn to_value(&self) -> Value {
        match self {
            Margin::Space(space) => space.to_value(),
            Margin::Auto => Value::from("auto"),
        }
    }
}

/// A border: the theme's plain one, or one in a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// `true`: the theme's border color.
    Plain,
    /// A color.
    Color(Color),
}

impl Vocab for Line {
    fn all() -> Vec<Self> {
        [Line::Plain]
            .into_iter()
            .chain(Color::all().into_iter().map(Line::Color))
            .collect()
    }
    fn to_value(&self) -> Value {
        match self {
            Line::Plain => Value::Bool(true),
            Line::Color(color) => color.to_value(),
        }
    }
}

words! {
    /// Rounded corners; `"box"` is the theme's own.
    Radius {
        None = "none",
        Sm = "sm",
        Md = "md",
        Lg = "lg",
        Xl = "xl",
        Full = "full",
        Box = "box",
    }
}

words! {
    /// A shadow's depth.
    Shadow { None = "none", Sm = "sm", Md = "md", Lg = "lg" }
}

words! {
    /// A height.
    Height { Full = "full", Screen = "screen" }
}

words! {
    /// A text size.
    TextSize {
        Xs = "xs",
        Sm = "sm",
        Base = "base",
        Lg = "lg",
        Xl = "xl",
        Xl2 = "2xl",
        Xl3 = "3xl",
        Xl4 = "4xl",
    }
}

words! {
    /// A font weight.
    Weight { Normal = "normal", Medium = "medium", Semibold = "semibold", Bold = "bold" }
}

/// A text color: muted, or a theme color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextColor {
    /// `"muted"`: the base text, faded.
    Muted,
    /// A theme color.
    Color(Color),
}

impl Vocab for TextColor {
    fn all() -> Vec<Self> {
        [TextColor::Muted]
            .into_iter()
            .chain(Color::all().into_iter().map(TextColor::Color))
            .collect()
    }
    fn to_value(&self) -> Value {
        match self {
            TextColor::Muted => Value::from("muted"),
            TextColor::Color(color) => color.to_value(),
        }
    }
}

words! {
    /// Which side text lines up on.
    TextAlign { Left = "left", Center = "center", Right = "right" }
}

words! {
    /// A font family.
    Font { Sans = "sans", Mono = "mono" }
}

/// One type per style key, each holding its value and knowing its own name.
pub mod keys {
    use super::{StyleKey, Vocab, read};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
    use serde_json::Value;

    macro_rules! keys {
        ($($(#[$m:meta])* $key:ident($value:ty) = $name:literal;)*) => {$(
            $(#[$m])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $key(pub $value);

            impl StyleKey for $key {
                const NAME: &'static str = $name;
                type Value = $value;
                fn new(value: $value) -> Self {
                    $key(value)
                }
            }

            impl Serialize for $key {
                fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    self.0.to_value().serialize(serializer)
                }
            }

            impl<'de> Deserialize<'de> for $key {
                fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                    let value = Value::deserialize(deserializer)?;
                    read($name, &value).map($key).map_err(de::Error::custom)
                }
            }
        )*};
    }

    keys! {
        /// `flow`: lays children top to bottom (the default), or side by side.
        Flow(super::Direction) = "flow";
        /// `grid`: lays children in a grid of equal columns, filling rows left to right.
        Grid(super::Columns) = "grid";
        /// `gap`: space between children.
        Gap(super::Space) = "gap";
        /// `align`: lines children up across the flow.
        Align(super::Alignment) = "align";
        /// `justify`: spreads children along the flow.
        Justify(super::Justification) = "justify";
        /// `wrap`: lets a row wrap.
        Wrap(bool) = "wrap";
        /// `pad`: space inside.
        Pad(super::Space) = "pad";
        /// `pad_x`: space inside, left and right.
        PadX(super::Space) = "pad_x";
        /// `pad_y`: space inside, top and bottom.
        PadY(super::Space) = "pad_y";
        /// `margin`: space outside.
        Margin(super::Margin) = "margin";
        /// `margin_x`: space outside, left and right; `"auto"` centers.
        MarginX(super::Margin) = "margin_x";
        /// `margin_y`: space outside, top and bottom.
        MarginY(super::Margin) = "margin_y";
        /// `bg`: the background.
        Bg(super::Color) = "bg";
        /// `border`: a border.
        Border(super::Line) = "border";
        /// `radius`: rounded corners.
        Radius(super::Radius) = "radius";
        /// `shadow`: a shadow.
        Shadow(super::Shadow) = "shadow";
        /// `height`: its height.
        Height(super::Height) = "height";
        /// `scroll`: scrolls what doesn't fit.
        Scroll(bool) = "scroll";
        /// `size`: the text size.
        Size(super::TextSize) = "size";
        /// `weight`: the font weight.
        Weight(super::Weight) = "weight";
        /// `color`: the text color.
        Color(super::TextColor) = "color";
        /// `text_align`: which side text lines up on.
        TextAlign(super::TextAlign) = "text_align";
        /// `font`: the font family.
        Font(super::Font) = "font";
    }
}

/// A cell's layout keys, read together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Layout {
    pub flow: Option<keys::Flow>,
    pub grid: Option<keys::Grid>,
    pub gap: Option<keys::Gap>,
    pub align: Option<keys::Align>,
    pub justify: Option<keys::Justify>,
    pub wrap: Option<keys::Wrap>,
}

/// The box keys, read together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Boxed {
    pub pad: Option<keys::Pad>,
    pub pad_x: Option<keys::PadX>,
    pub pad_y: Option<keys::PadY>,
    pub margin: Option<keys::Margin>,
    pub margin_x: Option<keys::MarginX>,
    pub margin_y: Option<keys::MarginY>,
    pub bg: Option<keys::Bg>,
    pub border: Option<keys::Border>,
    pub radius: Option<keys::Radius>,
    pub shadow: Option<keys::Shadow>,
    pub height: Option<keys::Height>,
    pub scroll: Option<keys::Scroll>,
}

/// The type keys, read together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Typed {
    pub size: Option<keys::Size>,
    pub weight: Option<keys::Weight>,
    pub color: Option<keys::Color>,
    pub text_align: Option<keys::TextAlign>,
    pub font: Option<keys::Font>,
}

/// Defines a node type with groups of style keys (`layout`, `boxed`, `typed`) as its own fields,
/// each optional, and a method per group reading them together ([`Layout`], [`Boxed`], [`Typed`]).
///
/// The keys are the node's own fields rather than a nested table, so the TOML writes them flat
/// (`gap = 4`) and an unknown key is still refused with its line.
macro_rules! styled {
    // Every group's fields are gathered: write the struct, and a method per group.
    (@fields [$($fields:tt)*] [] [$($group:ident)*]
        $(#[$m:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$m])*
        pub struct $name {
            $($body)*
            $($fields)*
        }
        $($crate::style::styled!(@method $group $name);)*
    };
    (@fields [$($fields:tt)*] [layout $($rest:ident)*] $groups:tt $($item:tt)*) => {
        $crate::style::styled!(@fields [$($fields)*
            #[doc = concat!("`flow`: see [`$crate::style::keys::Flow`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub flow: Option<$crate::style::keys::Flow>,
            #[doc = concat!("`grid`: see [`$crate::style::keys::Grid`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub grid: Option<$crate::style::keys::Grid>,
            #[doc = concat!("`gap`: see [`$crate::style::keys::Gap`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub gap: Option<$crate::style::keys::Gap>,
            #[doc = concat!("`align`: see [`$crate::style::keys::Align`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub align: Option<$crate::style::keys::Align>,
            #[doc = concat!("`justify`: see [`$crate::style::keys::Justify`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub justify: Option<$crate::style::keys::Justify>,
            #[doc = concat!("`wrap`: see [`$crate::style::keys::Wrap`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub wrap: Option<$crate::style::keys::Wrap>,
        ] [$($rest)*] $groups $($item)*);
    };
    (@fields [$($fields:tt)*] [boxed $($rest:ident)*] $groups:tt $($item:tt)*) => {
        $crate::style::styled!(@fields [$($fields)*
            #[doc = concat!("`pad`: see [`$crate::style::keys::Pad`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub pad: Option<$crate::style::keys::Pad>,
            #[doc = concat!("`pad_x`: see [`$crate::style::keys::PadX`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub pad_x: Option<$crate::style::keys::PadX>,
            #[doc = concat!("`pad_y`: see [`$crate::style::keys::PadY`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub pad_y: Option<$crate::style::keys::PadY>,
            #[doc = concat!("`margin`: see [`$crate::style::keys::Margin`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub margin: Option<$crate::style::keys::Margin>,
            #[doc = concat!("`margin_x`: see [`$crate::style::keys::MarginX`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub margin_x: Option<$crate::style::keys::MarginX>,
            #[doc = concat!("`margin_y`: see [`$crate::style::keys::MarginY`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub margin_y: Option<$crate::style::keys::MarginY>,
            #[doc = concat!("`bg`: see [`$crate::style::keys::Bg`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub bg: Option<$crate::style::keys::Bg>,
            #[doc = concat!("`border`: see [`$crate::style::keys::Border`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub border: Option<$crate::style::keys::Border>,
            #[doc = concat!("`radius`: see [`$crate::style::keys::Radius`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub radius: Option<$crate::style::keys::Radius>,
            #[doc = concat!("`shadow`: see [`$crate::style::keys::Shadow`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub shadow: Option<$crate::style::keys::Shadow>,
            #[doc = concat!("`height`: see [`$crate::style::keys::Height`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub height: Option<$crate::style::keys::Height>,
            #[doc = concat!("`scroll`: see [`$crate::style::keys::Scroll`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub scroll: Option<$crate::style::keys::Scroll>,
        ] [$($rest)*] $groups $($item)*);
    };
    (@fields [$($fields:tt)*] [typed $($rest:ident)*] $groups:tt $($item:tt)*) => {
        $crate::style::styled!(@fields [$($fields)*
            #[doc = concat!("`size`: see [`$crate::style::keys::Size`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub size: Option<$crate::style::keys::Size>,
            #[doc = concat!("`weight`: see [`$crate::style::keys::Weight`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub weight: Option<$crate::style::keys::Weight>,
            #[doc = concat!("`color`: see [`$crate::style::keys::Color`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub color: Option<$crate::style::keys::Color>,
            #[doc = concat!("`text_align`: see [`$crate::style::keys::TextAlign`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub text_align: Option<$crate::style::keys::TextAlign>,
            #[doc = concat!("`font`: see [`$crate::style::keys::Font`].")]
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub font: Option<$crate::style::keys::Font>,
        ] [$($rest)*] $groups $($item)*);
    };
    (@method layout $name:ident) => {
        impl $name {
            /// Its layout keys, read together.
            pub fn layout(&self) -> $crate::style::Layout {
                $crate::style::Layout { flow: self.flow, grid: self.grid, gap: self.gap, align: self.align, justify: self.justify, wrap: self.wrap }
            }
        }
    };
    (@method boxed $name:ident) => {
        impl $name {
            /// Its box keys, read together.
            pub fn boxed(&self) -> $crate::style::Boxed {
                $crate::style::Boxed { pad: self.pad, pad_x: self.pad_x, pad_y: self.pad_y, margin: self.margin, margin_x: self.margin_x, margin_y: self.margin_y, bg: self.bg, border: self.border, radius: self.radius, shadow: self.shadow, height: self.height, scroll: self.scroll }
            }
        }
    };
    (@method typed $name:ident) => {
        impl $name {
            /// Its type keys, read together.
            pub fn typed(&self) -> $crate::style::Typed {
                $crate::style::Typed { size: self.size, weight: self.weight, color: self.color, text_align: self.text_align, font: self.font }
            }
        }
    };
    ([$($group:ident),*] $($item:tt)*) => {
        $crate::style::styled!(@fields [] [$($group)*] [$($group)*] $($item)*);
    };
}
pub(crate) use styled;
