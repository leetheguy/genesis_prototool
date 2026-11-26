//! The controls with `var` ([spec 040](../../../specs/040_catalog.md), "Controls"): `input`,
//! `textarea`, `select`, `checkbox`, `toggle`, and `radio`.
//!
//! Each shows its variable's current value and, when the person types or picks, fires
//! `NAME.change` with the new `value`, keeping its JSON type. The bus turns that into a `set` of
//! the variable, so a control never changes state itself. Each is drawn as the real HTML element
//! with daisyUI's class, so keyboards, screen readers, and autofill work. A label wraps its control,
//! which is what names the control for screen readers.

use crate::style::ClassList;
use crate::{Component, Wired, failed_face};
use genesis_core::config::{Checkbox, Input, InputKind, Options, Radio, Select, Textarea, Toggle};
use genesis_core::template::as_text;
use leptos::prelude::*;
use serde_json::{Value, json};

/// Draws `control` with its label above it, or alone when it has none. Either way the outermost
/// element takes the node's placement.
fn labelled(
    wired: &Wired,
    label: &Option<String>,
    own: &'static str,
    control: impl Fn(String) -> AnyView,
) -> AnyView {
    match label {
        Some(label) => {
            let text = wired.text(label);
            let class = wired.class(ClassList::default().raw("fieldset"));
            let control = control(own.to_string());
            view! { <label class=class><span class="fieldset-legend">{text}</span>{control}</label> }
                .into_any()
        }
        None => control(wired.class(ClassList::default().raw(own))),
    }
}

/// The same for a checkbox, toggle, or radio: the box first, then its words.
fn boxed_label(
    wired: &Wired,
    label: &Option<String>,
    own: &'static str,
    control: impl Fn(String) -> AnyView,
) -> AnyView {
    match label {
        Some(label) => {
            let text = wired.text(label);
            let class = wired.class(ClassList::default().raw("label"));
            let control = control(own.to_string());
            view! { <label class=class>{control}<span>{text}</span></label> }.into_any()
        }
        None => control(wired.class(ClassList::default().raw(own))),
    }
}

/// A variable's value as the text a box shows.
fn shown(read: impl Fn() -> Option<Value>) -> impl Fn() -> String {
    move || read().map(|v| as_text(&v)).unwrap_or_default()
}

/// What a number box holds, as JSON: `null` when it's empty, a whole number as an integer.
fn number(text: &str) -> Value {
    match text.trim().parse::<f64>() {
        Ok(n) if n.fract() == 0.0 && n.abs() < 9e15 => json!(n as i64),
        Ok(n) => json!(n),
        Err(_) => Value::Null,
    }
}

/// What a box with `kind` means by `text`: a number for a number box, the text itself otherwise.
fn meaning(kind: InputKind, text: &str) -> Value {
    match kind {
        InputKind::Number => number(text),
        _ => json!(text),
    }
}

impl Component for Input {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        let (kind, read) = (self.kind, wired.read(&self.var));
        let placeholder = self.placeholder.as_ref().map(|p| wired.text(p));
        let placeholder = move || placeholder.as_ref().map(|p| p());
        // What the person last typed. While it still means the variable's value, the box keeps it,
        // so "1." or "-" on the way to a number isn't wiped out by the variable's own text.
        let typed = RwSignal::new(None::<String>);
        // A memo tells the box only when its text really changes: setting a box's value, even to
        // what it already means, wipes a half-typed number the browser holds as "".
        let showing = Memo::new(move |_| {
            let now = read().unwrap_or(Value::Null);
            match typed.get() {
                Some(text) if meaning(kind, &text) == now => text,
                _ => as_text(&now),
            }
        });
        let fire = wired.clone();
        let on_input = move |ev| {
            let input = event_target::<leptos::web_sys::HtmlInputElement>(&ev);
            let text = input.value();
            // A number box half typed ("-", "2.") reads as "" and fails its own check; it means
            // nothing yet, so the variable keeps its value until the number is whole.
            if kind == InputKind::Number && text.is_empty() && !input.check_validity() {
                return;
            }
            let value = meaning(kind, &text);
            typed.set(Some(text));
            fire.fire("change", json!({ "value": value }));
        };
        labelled(&wired, &self.label, "input", move |class| {
            let (value, prop) = (move || showing.get(), move || showing.get());
            let (placeholder, on_input) = (placeholder.clone(), on_input.clone());
            view! {
                <input type=kind.as_str() class=class placeholder=placeholder value=value prop:value=prop
                    on:input=on_input />
            }
            .into_any()
        })
    }
}

impl Component for Textarea {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        let (rows, read) = (self.rows, wired.read(&self.var));
        let placeholder = self.placeholder.as_ref().map(|p| wired.text(p));
        let placeholder = move || placeholder.as_ref().map(|p| p());
        let fire = wired.clone();
        let on_input = move |ev| {
            fire.fire("change", json!({ "value": event_target_value(&ev) }));
        };
        labelled(&wired, &self.label, "textarea", move |class| {
            let (text, prop) = (shown(read.clone()), shown(read.clone()));
            let (placeholder, on_input) = (placeholder.clone(), on_input.clone());
            view! {
                <textarea class=class placeholder=placeholder rows=rows prop:value=prop on:input=on_input>
                    {text}
                </textarea>
            }
            .into_any()
        })
    }
}

/// Fires a checkbox or toggle's `.change` with whether it's now checked.
fn checked_view(wired: &Wired, var: &str, label: &Option<String>, own: &'static str) -> AnyView {
    let read = wired.read(var);
    let fire = wired.clone();
    let on_change = move |ev| {
        fire.fire("change", json!({ "value": event_target_checked(&ev) }));
    };
    boxed_label(wired, label, own, move |class| {
        let (on, prop) = (read.clone(), read.clone());
        let checked = move || on() == Some(Value::Bool(true));
        let prop = move || prop() == Some(Value::Bool(true));
        let on_change = on_change.clone();
        view! { <input type="checkbox" class=class checked=checked prop:checked=prop on:change=on_change /> }
            .into_any()
    })
}

impl Component for Checkbox {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        checked_view(&wired, &self.var, &self.label, "checkbox")
    }
}

impl Component for Toggle {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        checked_view(&wired, &self.var, &self.label, "toggle")
    }
}

impl Component for Radio {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        let (read, value) = (wired.read(&self.var), self.value.clone());
        // The browser groups radios by `name`, and moves through a group with the arrow keys.
        let group = self.var.clone();
        let fire = wired.clone();
        let picked = value.clone();
        let on_change = move |_| fire.fire("change", json!({ "value": picked.clone() }));
        boxed_label(&wired, &self.label, "radio", move |class| {
            let (on, prop) = (read.clone(), read.clone());
            let (mine, also) = (value.clone(), value.clone());
            let checked = move || on().as_ref() == Some(&mine);
            let prop = move || prop().as_ref() == Some(&also);
            let on_change = on_change.clone();
            view! {
                <input type="radio" name=group.clone() class=class checked=checked prop:checked=prop
                    on:change=on_change />
            }
            .into_any()
        })
    }
}

/// A select's options right now, as `(label, value)`: written out, or read from their variable by
/// the label and value pointers. `Err` says why a variable's options aren't a list.
fn choices(
    select: &Select,
    wired: &Wired,
) -> impl Fn() -> Result<Vec<(String, Value)>, String> + Clone + Send + Sync + 'static {
    let read = match &select.options {
        Options::From(path) => Some(wired.read(path)),
        Options::List(_) => None,
    };
    let options = select.options.clone();
    let label = select.option_label.clone().unwrap_or("/label".into());
    let value = select.option_value.clone().unwrap_or("/value".into());
    move || match (&options, &read) {
        (Options::List(list), _) => Ok(list
            .iter()
            .map(|c| (c.label.clone(), c.value.clone()))
            .collect()),
        (Options::From(path), Some(read)) => match read() {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(rows)) => Ok(rows
                .iter()
                .map(|row| {
                    let label = row.pointer(&label).map(as_text).unwrap_or_default();
                    (label, row.pointer(&value).cloned().unwrap_or(Value::Null))
                })
                .collect()),
            Some(_) => Err(format!(
                "a select's options need a list, and {path} isn't one"
            )),
        },
        (Options::From(_), None) => Ok(Vec::new()),
    }
}

impl Component for Select {
    const EVENTS: &'static [&'static str] = &["change"];

    fn view(&self, wired: Wired) -> AnyView {
        let (read, choices) = (wired.read(&self.var), choices(self, &wired));
        let placeholder = self.placeholder.as_ref().map(|p| wired.text(p));
        let fire = wired.clone();
        let pick = {
            let choices = choices.clone();
            move |ev| {
                // Each option's `value` is its place in the list; the variable gets its JSON value.
                let picked = event_target_value(&ev).parse::<usize>().ok();
                let list = choices().unwrap_or_default();
                if let Some((_, value)) = picked.and_then(|i| list.get(i)) {
                    fire.fire("change", json!({ "value": value }));
                }
            }
        };
        // A variable's options that aren't a list are a loud failure in the select's place. The
        // select itself is drawn again only when that changes, so it keeps focus while in use.
        let failure = {
            let choices = choices.clone();
            Memo::new(move |_| choices().err())
        };
        let (label, wired) = (self.label.clone(), wired.clone());
        let select = move || {
            if let Some(why) = failure.get() {
                return failed_face(None, &why);
            }
            labelled(&wired, &label, "select", |class| {
                let (read, choices, pick) = (read.clone(), choices.clone(), pick.clone());
                let placeholder = placeholder.clone();
                let options = move || {
                    let now = read();
                    let list = choices().unwrap_or_default();
                    let matched = list.iter().any(|(_, v)| Some(v) == now.as_ref());
                    let first = placeholder.as_ref().map(|p| {
                        let text = p();
                        view! { <option value="" disabled selected=!matched>{text}</option> }
                    });
                    let rest = list
                        .into_iter()
                        .enumerate()
                        .map(|(i, (label, value))| {
                            let on = Some(&value) == now.as_ref();
                            view! { <option value=i.to_string() selected=on>{label}</option> }
                        })
                        .collect_view();
                    view! { {first}{rest} }
                };
                view! { <select class=class on:change=pick>{options}</select> }.into_any()
            })
        };
        select.into_any()
    }
}
