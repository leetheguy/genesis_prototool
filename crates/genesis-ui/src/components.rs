//! The catalog's components ([spec 040](../../../specs/040_catalog.md)), one [`Component`] each.
//!
//! Each is small on purpose: it draws its daisyUI markup, reads values through its [`Wired`],
//! and fires its events through it. That's all a component ever does.

use crate::style::ClassList;
use crate::{Component, Wired, nothing};
use genesis_core::config::{Button, Cell, Text};
use genesis_core::style::{BySize, TextSize, Weight, keys};
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
        let class = ClassList::default()
            .layout(self.layout())
            .boxed(self.boxed())
            .typed(self.typed())
            .attr();
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
        let class = ClassList::default().boxed(self.boxed()).typed(typed).attr();
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
        view! { <button class="btn btn-primary" on:click=move |_| wired.fire("click", nothing())>{label}</button> }.into_any()
    }
}
