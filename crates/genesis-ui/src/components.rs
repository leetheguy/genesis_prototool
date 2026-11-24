//! The catalog's components ([spec 040](../../../specs/040_catalog.md)), one [`Component`] each.
//!
//! Each is small on purpose: it draws its daisyUI markup, reads values through its [`Wired`],
//! and fires its events through it. That's all a component ever does.

use crate::{Component, Wired, nothing};
use genesis_core::config::{Button, Cell, Text};
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
        view! { <div class="flex flex-col gap-4">{children}</div> }.into_any()
    }
}

impl Component for Text {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let text = wired.text(&self.text);
        match self.level {
            None => view! { <p>{text}</p> }.into_any(),
            Some(1) => view! { <h1 class="text-3xl font-bold">{text}</h1> }.into_any(),
            Some(2) => view! { <h2 class="text-2xl font-bold">{text}</h2> }.into_any(),
            Some(_) => view! { <h3 class="text-xl font-semibold">{text}</h3> }.into_any(),
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
