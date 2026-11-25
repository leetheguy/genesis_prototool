//! The catalog's components ([spec 040](../../../specs/040_catalog.md)), one [`Component`] each.
//!
//! Each is small on purpose: it draws its daisyUI markup, reads values through its [`Wired`],
//! and fires its events through it. That's all a component ever does.

use crate::style::ClassList;
use crate::{Component, Wired, nothing};
use genesis_core::config::{Audio, Button, Cell, Divider, Iframe, Image, Link, Svg, Text, Video};
use genesis_core::style::{BySize, Color, Height, TextSize, Variant, Weight, keys};
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

impl Component for Svg {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        // With a label it's an image with that name; without one, decoration screen readers skip.
        let label = self.label.clone();
        let role = label.is_some().then_some("img");
        let hidden = label.is_none().then_some("true");
        let class = wired.class(ClassList::default().raw("inline-flex").typed(self.typed()));
        let markup = self.svg.as_str().to_string();
        view! { <span role=role aria-label=label aria-hidden=hidden class=class inner_html=markup></span> }
            .into_any()
    }
}

impl Component for Divider {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let color = self
            .color
            .unwrap_or(keys::LineColor(BySize::base(Color::Base300)));
        let class = wired.class(ClassList::default().add(Some(color)).boxed(self.boxed()));
        view! { <hr class=class /> }.into_any()
    }
}

/// Fires a player's `event` when it has an `id`; one without fires nothing.
fn player(
    wired: &Wired,
    id: &Option<String>,
) -> impl Fn(&'static str) + Clone + Send + Sync + 'static {
    let (wired, named) = (wired.clone(), id.is_some());
    move |event| {
        if named {
            wired.fire(event, nothing());
        }
    }
}

impl Component for Audio {
    const EVENTS: &'static [&'static str] = &["play", "pause", "ended"];

    fn view(&self, wired: Wired) -> AnyView {
        let src = wired.text(&self.src);
        let class = wired.class(&mut ClassList::default());
        let fire = player(&wired, &self.id);
        let (play, pause, ended) = (fire.clone(), fire.clone(), fire);
        view! {
            <audio src=src controls=self.controls autoplay=self.autoplay loop=self.r#loop muted=self.muted class=class
                on:play=move |_| play("play")
                on:pause=move |_| pause("pause")
                on:ended=move |_| ended("ended")
            ></audio>
        }
        .into_any()
    }
}

impl Component for Video {
    const EVENTS: &'static [&'static str] = &["play", "pause", "ended"];

    fn view(&self, wired: Wired) -> AnyView {
        let src = wired.text(&self.src);
        let poster = self.poster.as_ref().map(|p| wired.text(p));
        let poster = move || poster.as_ref().map(|p| p());
        let class = wired.class(ClassList::default().boxed(self.boxed()));
        let fire = player(&wired, &self.id);
        let (play, pause, ended) = (fire.clone(), fire.clone(), fire);
        view! {
            <video src=src poster=poster controls=self.controls autoplay=self.autoplay loop=self.r#loop muted=self.muted class=class
                on:play=move |_| play("play")
                on:pause=move |_| pause("pause")
                on:ended=move |_| ended("ended")
            ></video>
        }
        .into_any()
    }
}

impl Component for Iframe {
    const EVENTS: &'static [&'static str] = &[];

    fn view(&self, wired: Wired) -> AnyView {
        let src = wired.text(&self.src);
        let title = self.title.clone();
        // Scripts, forms, and popups, but not this app's origin, and no navigating it.
        let sandbox = self
            .sandbox
            .then_some("allow-scripts allow-forms allow-popups");
        let mut own = ClassList::default();
        let boxed = self.boxed();
        if boxed.height.is_none() {
            own.add(Some(keys::Height(BySize::base(Height::Rem(24)))));
        }
        if self.width.is_none() {
            own.raw("w-full");
        }
        let class = wired.class(own.boxed(boxed));
        view! { <iframe src=src title=title sandbox=sandbox class=class></iframe> }.into_any()
    }
}
