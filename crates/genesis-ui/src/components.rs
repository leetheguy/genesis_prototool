//! The catalog's components ([spec 040](../../../specs/040_catalog.md)), one [`Component`] each.
//!
//! Each is small on purpose: it draws its daisyUI markup, reads values through its [`Wired`],
//! and fires its events through it. That's all a component ever does.

use crate::style::ClassList;
use crate::{Component, Wired, nothing};
use genesis_core::config::{Button, Cell, Image, Link, Text};
use genesis_core::style::{BySize, TextSize, Variant, Weight, keys};
use leptos::prelude::*;

impl Component for Cell {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let children: Vec<AnyView> = self
            .children
            .iter()
            .enumerate()
            .map(|(i, child)| wired.child(i, child))
            .collect();
        let class = wired.class(
            ClassList::default()
                .layout(self.layout())
                .boxed(self.boxed())
                .typed(self.typed()),
        );
        view! { <div class=class>{children}</div> }.into_any()
    }
}

impl Component for Text {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let text = wired.text(&self.text);
        // A heading has a heading's size and weight, unless its own keys say otherwise.
        let (size, weight) = match self.level {
            None => (None, None),
            Some(1) => (Some(TextSize::Xl3), Some(Weight::Bold)),
            Some(2) => (Some(TextSize::Xl2), Some(Weight::Bold)),
            Some(_) => (Some(TextSize::Xl), Some(Weight::Semibold)),
        };
        // A size or weight key that names only larger screens keeps the heading's own below them.
        let mut typed = self.typed();
        if let Some(size) = size {
            typed.size = Some(keys::Size(
                typed.size.map_or(BySize::base(size), |k| k.0.or_base(size)),
            ));
        }
        if let Some(weight) = weight {
            typed.weight = Some(keys::Weight(
                typed
                    .weight
                    .map_or(BySize::base(weight), |k| k.0.or_base(weight)),
            ));
        }
        let class = wired.class(ClassList::default().boxed(self.boxed()).typed(typed));
        match self.level {
            None => view! { <p class=class>{text}</p> }.into_any(),
            Some(1) => view! { <h1 class=class>{text}</h1> }.into_any(),
            Some(2) => view! { <h2 class=class>{text}</h2> }.into_any(),
            Some(_) => view! { <h3 class=class>{text}</h3> }.into_any(),
        }
    }
}

impl Component for Button {
    const EVENTS: &'static [&'static str] = &["click"];

    fn view(&self, wired: Wired) -> AnyView {
        let label = wired.text(&self.label);
        let variant = self
            .variant
            .unwrap_or(keys::Variant(BySize::base(Variant::Primary)));
        let class = wired.class(
            ClassList::default()
                .raw("btn")
                .add(Some(variant))
                .add(self.size),
        );
        view! { <button class=class on:click=move |_| wired.fire("click", nothing())>{label}</button> }.into_any()
    }
}

impl Component for Image {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let (src, alt) = (wired.text(&self.src), self.alt.clone());
        let class = wired.class(ClassList::default().boxed(self.boxed()));
        view! { <img src=src alt=alt class=class /> }.into_any()
    }
}

/// Whether `href` leaves the app: a full URL, or one that starts with `//`.
fn outside(href: &str) -> bool {
    ["http://", "https://", "//"]
        .iter()
        .any(|start| href.starts_with(start))
}

impl Component for Link {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let (text, href) = (wired.text(&self.text), wired.text(&self.href));
        let leaves = {
            let href = wired.text(&self.href);
            Signal::derive(move || outside(&href()))
        };
        let target = move || leaves.get().then_some("_blank");
        let rel = move || leaves.get().then_some("noopener noreferrer");
        let class = wired.class(ClassList::default().raw("link"));
        view! { <a href=href target=target rel=rel class=class>{text}</a> }.into_any()
    }
}
