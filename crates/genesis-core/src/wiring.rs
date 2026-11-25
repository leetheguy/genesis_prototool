//! The load checks for wiring ([spec 020](../../../specs/020_config.md), "Checks"): once every file
//! reads, the whole app is read again for what would otherwise fail only when it runs: a step naming
//! an action, timer, operation, or source that isn't there; a step writing a variable that isn't
//! declared or can't be written; and a template that isn't a plain lookup, or reads a variable that
//! isn't there where it's written. Each problem names its file, line, and column.
//!
//! The files are read again, with their positions, rather than the app's parsed config, so each
//! problem points at the very step or text it's about.

use crate::config::{App, ConfigError, Step, line_and_column, prop_steps, step_props};
use crate::template;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// The `sys.*` names a template or a path may read.
const SYS: [&str; 8] = [
    "theme", "outlet", "errors", "events", "bus", "sources", "toasts", "asking",
];

/// Keys on a node whose value is a variable's path rather than a template.
const NODE_PATHS: [&str; 5] = ["var", "each", "data", "open", "options"];

/// Keys on a node holding steps.
const NODE_STEPS: [&str; 3] = ["on_click", "on_submit", "on_close"];

/// Keys on a node holding conditions, whose paths `check_node` already checks for scope.
const CONDITIONS: [&str; 3] = ["show", "disabled", "checks"];

/// Where a step or a template is written, which decides what it may read and write.
#[derive(Clone, Copy, Default)]
struct Context {
    /// In a component's file: `local.*` and `props.*` are its own, checked when it loads.
    component: bool,
    /// In a step: `event.*` is the event being handled.
    step: bool,
    /// Inside a cell with `each`: `item.*` is the element.
    each: bool,
    /// Inside a `form` (or a component, which may be drawn in one): `form.*` is its draft.
    form: bool,
}

/// Every wiring problem in the app's files. `files` is each file's path and text, and
/// `components` names each component file's path by the component.
pub(crate) fn check(
    app: &App,
    files: &[(String, String)],
    components: &BTreeMap<String, String>,
) -> Vec<ConfigError> {
    let mut problems = Vec::new();
    let component_paths: Vec<&str> = components.values().map(String::as_str).collect();
    for (path, text) in files {
        if path.rsplit('/').next() == Some("timers.toml") {
            continue;
        }
        let Ok(table) = DeTable::parse(text) else {
            continue;
        };
        let mut file = File {
            app,
            path,
            text,
            problems: &mut problems,
        };
        let context = Context {
            component: component_paths.contains(&path.as_str()),
            ..Context::default()
        };
        for (key, value) in table.get_ref().iter() {
            match (key.get_ref().as_ref(), value.get_ref()) {
                ("on", DeValue::Table(handlers)) => {
                    for (_, steps) in handlers.iter() {
                        file.steps(
                            steps,
                            Context {
                                step: true,
                                ..context
                            },
                        );
                    }
                }
                ("actions", DeValue::Table(actions)) => {
                    for (_, action) in actions.iter() {
                        if let DeValue::Table(action) = action.get_ref() {
                            for (key, steps) in action.iter() {
                                if key.get_ref().as_ref() == "do" {
                                    file.steps(
                                        steps,
                                        Context {
                                            step: true,
                                            ..context
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                ("sources", DeValue::Table(sources)) => {
                    for (_, source) in sources.iter() {
                        file.source(source);
                    }
                }
                ("body", DeValue::Array(nodes)) => {
                    let context = Context {
                        form: context.component,
                        ..context
                    };
                    for node in nodes.iter() {
                        file.node(node, context);
                    }
                }
                _ => {}
            }
        }
    }
    problems
}

/// One file being checked.
struct File<'a> {
    app: &'a App,
    path: &'a str,
    text: &'a str,
    problems: &'a mut Vec<ConfigError>,
}

impl File<'_> {
    fn problem(&mut self, at: &Range<usize>, message: String) {
        let (line, column) = line_and_column(self.text, at.start);
        self.problems.push(ConfigError {
            file: Some(self.path.to_string()),
            message,
            line: Some(line),
            column: Some(column),
        });
    }

    /// A node: its steps, its paths, and every template in it, then its children.
    fn node(&mut self, node: &Spanned<DeValue>, context: Context) {
        let DeValue::Table(fields) = node.get_ref() else {
            return;
        };
        let field = |name: &str| {
            fields
                .iter()
                .find(|(k, _)| k.get_ref().as_ref() == name)
                .map(|(_, v)| v)
        };
        let is_form = matches!(
            field("type").map(Spanned::get_ref),
            Some(DeValue::String(kind)) if kind.as_ref() == "form"
        );
        let inner = Context {
            each: context.each || field("each").is_some(),
            form: context.form || is_form,
            ..context
        };
        // The props a used component runs as steps are steps here, where they're written.
        let steps_props: BTreeSet<String> = match field("component").map(Spanned::get_ref) {
            Some(DeValue::String(name)) => match self.app.components.get(name.as_ref()) {
                Some(component) => {
                    let mut seen = BTreeSet::from([name.to_string()]);
                    step_props(&component.body, &self.app.components, &mut seen)
                }
                None => BTreeSet::new(),
            },
            _ => BTreeSet::new(),
        };
        for (key, value) in fields.iter() {
            let key = key.get_ref().as_ref();
            match key {
                "children" => {
                    if let DeValue::Array(children) = value.get_ref() {
                        for child in children.iter() {
                            self.node(child, inner);
                        }
                    }
                }
                _ if NODE_STEPS.contains(&key) => {
                    self.steps(
                        value,
                        Context {
                            step: true,
                            ..inner
                        },
                    );
                }
                "props" if !steps_props.is_empty() => {
                    if let DeValue::Table(props) = value.get_ref() {
                        for (prop, value) in props.iter() {
                            if steps_props.contains(prop.get_ref().as_ref()) {
                                self.steps(
                                    value,
                                    Context {
                                        step: true,
                                        ..inner
                                    },
                                );
                            } else {
                                self.templates(value, inner);
                            }
                        }
                    }
                }
                _ if CONDITIONS.contains(&key) => self.condition(value),
                _ if NODE_PATHS.contains(&key) => {
                    if let DeValue::String(path) = value.get_ref() {
                        // A form's `data` and a control's `var` are read where the node is, and
                        // an `each` before its own rows.
                        let at = if key == "each" { context } else { inner };
                        if let Some(why) = self.path_problem(path, at) {
                            self.problem(&value.span(), why);
                        }
                    }
                }
                _ => self.templates(value, inner),
            }
        }
    }

    /// A condition, or a list of checks: only that each `app.*` it reads is declared, since
    /// `check_node` checks the rest.
    fn condition(&mut self, value: &Spanned<DeValue>) {
        match value.get_ref() {
            DeValue::Table(fields) => {
                for (key, value) in fields.iter() {
                    if key.get_ref().as_ref() == "var"
                        && let DeValue::String(path) = value.get_ref()
                        && let Some(why) = self.undeclared(path)
                    {
                        self.problem(&value.span(), why);
                    } else {
                        self.condition(value);
                    }
                }
            }
            DeValue::Array(items) => items.iter().for_each(|item| self.condition(item)),
            _ => {}
        }
    }

    /// A source: the operation it calls, and its params' templates.
    fn source(&mut self, source: &Spanned<DeValue>) {
        let DeValue::Table(fields) = source.get_ref() else {
            return;
        };
        for (key, value) in fields.iter() {
            match (key.get_ref().as_ref(), value.get_ref()) {
                ("call", DeValue::String(op)) => {
                    if let Some(why) = self.operation_problem(op) {
                        self.problem(&value.span(), why);
                    }
                }
                ("params", _) => self.templates(value, Context::default()),
                _ => {}
            }
        }
    }

    /// One step, or a list of them, as written for a handler, an action, or a node.
    fn steps(&mut self, value: &Spanned<DeValue>, context: Context) {
        match value.get_ref() {
            DeValue::Array(steps) => steps.iter().for_each(|step| self.step(step, context)),
            _ => self.step(value, context),
        }
    }

    fn step(&mut self, value: &Spanned<DeValue>, context: Context) {
        let at = value.span();
        // The step reads as it did when the app loaded; one that didn't is reported already.
        let Ok(OneStep { s: step }) =
            toml::from_str::<OneStep>(&format!("s = {}", &self.text[at.clone()]))
        else {
            return;
        };
        if let Some(why) = self.step_problem(&step, context) {
            self.problem(&at, why);
        }
        // Templates are checked where they're written, so each problem points at its text.
        if let DeValue::Table(fields) = value.get_ref() {
            for (key, value) in fields.iter() {
                if matches!(
                    key.get_ref().as_ref(),
                    "to" | "with" | "params" | "log" | "toast" | "confirm"
                ) {
                    self.templates(value, context);
                }
            }
        }
    }

    /// What's wrong with a step itself, besides its templates.
    fn step_problem(&self, step: &Step, context: Context) -> Option<String> {
        let app = self.app;
        match step {
            // A component's own steps are checked for what they reach when it loads.
            Step::Run(name) if context.component || prop_steps(name).is_some() => None,
            Step::Run(name) if name.contains("{{") => None,
            Step::Run(name) => (!app.actions.contains_key(name)).then(|| {
                format!(
                    "there's no custom action named `{name}`; define it as [actions.{name}] with `do = [...]`"
                )
            }),
            Step::Set { set: var, .. }
            | Step::Step { step: var, .. }
            | Step::Toggle { toggle: var } => self.write_problem(var, context),
            Step::Call { call, into, .. } => self.operation_problem(call).or_else(|| {
                into.keys()
                    .find_map(|var| self.write_problem(var, context))
                    .map(|why| format!("`into`: {why}"))
            }),
            Step::Reload { reload } => reload.iter().find_map(|var| {
                let source = var.strip_prefix("app.").filter(|n| app.sources.contains_key(*n));
                source.is_none().then(|| {
                    format!("`reload` takes sources, as \"app.NAME\" for each [sources.NAME], and {var:?} isn't one")
                })
            }),
            // A `go` to a page that isn't there fails when it runs, so a page can show up later.
            Step::Start { start: name } | Step::Stop { stop: name } if !context.component => {
                (!crate::config::timers(app).contains_key(name)).then(|| {
                    format!("there's no timer named `{name}`; a timer is a table in timers.toml, like [{name}] with `ms`")
                })
            }
            _ => None,
        }
    }

    /// Why a step can't write `var`, if it can't.
    fn write_problem(&self, var: &str, context: Context) -> Option<String> {
        match var.split_once('.') {
            Some(("app", _)) if context.component => None,
            Some(("app", key)) => self.declared(key).then_some(()).map_or_else(
                || {
                    Some(format!(
                        "`{var}` isn't declared; give it a starting value in [state]"
                    ))
                },
                |()| None,
            ),
            Some(("local", _)) if context.component => None,
            Some(("local", _)) => Some(format!(
                "`{var}` is a component's own variable, so only its own steps write it"
            )),
            Some(("sys", "theme")) => None,
            Some(("props", _)) if context.component => None,
            _ => Some(format!(
                "`{var}` can't be written; steps write `app.*`, `sys.theme`, and in a component its `local.*`"
            )),
        }
    }

    /// Why `op` can't be called, if it can't.
    fn operation_problem(&self, op: &str) -> Option<String> {
        self.app.operation(op).is_none().then(|| {
            let known = self.app.operation_names();
            if known.is_empty() {
                format!("there's no operation named `{op}`, and no connection declares any")
            } else {
                format!(
                    "there's no operation named `{op}`; the connections declare `{}`",
                    known.join("`, `")
                )
            }
        })
    }

    /// Every template in a value (strings anywhere inside it), checked where it's written.
    fn templates(&mut self, value: &Spanned<DeValue>, context: Context) {
        match value.get_ref() {
            DeValue::String(text) if text.contains("{{") => {
                let why = template::check(text).err().or_else(|| {
                    template::paths(text)
                        .into_iter()
                        .find_map(|path| self.path_problem(path, context))
                });
                if let Some(why) = why {
                    self.problem(&value.span(), why);
                }
            }
            DeValue::Array(items) => items.iter().for_each(|item| self.templates(item, context)),
            DeValue::Table(fields) => {
                for (_, value) in fields.iter() {
                    self.templates(value, context);
                }
            }
            _ => {}
        }
    }

    /// Why `path` can't be read where `context` says it's written, if it can't.
    fn path_problem(&self, path: &str, context: Context) -> Option<String> {
        let mut parts = path.split('.');
        let root = parts.next().unwrap_or_default();
        let next = parts.next();
        match root {
            "app" => self.undeclared(path),
            "sys" => match next {
                Some(name) if SYS.contains(&name) => None,
                _ => Some(format!(
                    "`{path}` isn't one of Genesis's own variables: `sys.{}`",
                    SYS.join("`, `sys.")
                )),
            },
            "event" if context.step => None,
            "event" => Some(format!(
                "`{path}` reads the event being handled, so it works only in a step"
            )),
            "item" if context.step => Some(format!(
                "a step reads the row it fired in as `event.item`, not `{path}`"
            )),
            "item" if context.each => None,
            "item" => Some(format!(
                "`{path}` reads the element of an `each`, so it works only inside one"
            )),
            "local" | "props" if context.component => None,
            "local" | "props" => Some(format!(
                "`{path}` reads a component's `{root}.*`, so it works only inside one"
            )),
            "form" if !context.step && context.form => None,
            "form" if context.step => Some(format!(
                "a step reads a submitted form as `event.form`, not `{path}`"
            )),
            "form" => Some(format!(
                "`{path}` reads a form's draft, so it works only inside a `form`"
            )),
            "creds" | "session" => Some(format!(
                "`{path}` works only in a connection, in [connections.NAME]"
            )),
            _ => Some(format!(
                "`{path}` reads nothing: a template reads `app.*`, `sys.*`, `event.*` (in a step), `item.*` (in an `each`), `form.*` (in a form), or in a component its `local.*` and `props.*`"
            )),
        }
    }

    /// Why an `app.*` path can't be read, if its variable isn't declared.
    fn undeclared(&self, path: &str) -> Option<String> {
        let key = path.strip_prefix("app.")?.split('.').next()?;
        (!self.declared(key)).then(|| {
            format!("`app.{key}` isn't declared; give it a starting value in [state], or make it a source")
        })
    }

    /// Whether `app.KEY` is declared: in `[state]`, or as a source.
    fn declared(&self, key: &str) -> bool {
        self.app.state.contains_key(key) || self.app.sources.contains_key(key)
    }
}

/// One step, read from its text alone.
#[derive(Deserialize)]
struct OneStep {
    s: Step,
}
