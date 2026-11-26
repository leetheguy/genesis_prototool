//! Conditions: display logic's one test on one variable ([spec 080](../../../specs/080_display_logic.md)).
//!
//! A condition is a TOML table, never an expression: it names a variable and makes one test, or
//! combines other conditions with `all`, `any`, and `not`. Conditions go only in `show` (any node)
//! and `disabled` (buttons and controls), so they can't grow into a language. A test on the wrong
//! type is false, never an error.
//!
//! ```
//! use genesis_core::condition::Condition;
//! use serde_json::{json, Value};
//!
//! let shown: Condition = serde_json::from_value(json!({ "var": "app.view", "is": "contacts" })).unwrap();
//! let look = |path: &str| (path == "app.view").then(|| json!("contacts"));
//! assert!(shown.holds(&look));
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A condition: one test on one variable, or a combination of conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum Condition {
    /// `all = [...]`: every one holds.
    All(Vec<Condition>),
    /// `any = [...]`: at least one holds.
    Any(Vec<Condition>),
    /// `not = {...}`: it doesn't hold.
    Not(Box<Condition>),
    /// `var` and one test on its value.
    Test {
        /// The variable's path: `app.view`, `item.status`, `form.email`, `local.open`, …
        var: String,
        /// The test made on it.
        test: Test,
    },
}

/// One test on a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Test {
    /// `is = X`: equals `X`. Numbers are equal by value, so `1` is `1.0`.
    Is(Value),
    /// `in = [X, Y]`: equals one of them.
    In(Vec<Value>),
    /// `empty = true`: missing, `null`, `""`, `[]`, or `{}`; `empty = false` is the opposite.
    Empty(bool),
    /// `gt = N`: a number greater than `N`.
    Gt(f64),
    /// `gte = N`: a number at least `N`.
    Gte(f64),
    /// `lt = N`: a number less than `N`.
    Lt(f64),
    /// `lte = N`: a number at most `N`.
    Lte(f64),
    /// `matches = "regex"`: text the regular expression finds a match in.
    Matches(Pattern),
    /// `min_len = N`: text with at least `N` characters, or a list with at least `N` items.
    MinLen(usize),
    /// `max_len = N`: text with at most `N` characters, or a list with at most `N` items.
    MaxLen(usize),
    /// `status = "loading"`: a source with that status ([spec 050](../../../specs/050_state.md)).
    Status(String),
}

/// The names of the tests, in the order the spec gives them.
pub const TESTS: [&str; 11] = [
    "is", "in", "empty", "gt", "gte", "lt", "lte", "matches", "min_len", "max_len", "status",
];

/// The statuses a source can have.
const STATUSES: [&str; 4] = ["idle", "loading", "ok", "failed"];

/// A regular expression, checked when it loads, kept with the text it was written as.
#[derive(Debug, Clone)]
pub struct Pattern {
    text: String,
    regex: regex_lite::Regex,
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Pattern {
    /// Reads `text` as a regular expression, or says why it isn't one.
    pub fn new(text: &str) -> Result<Pattern, String> {
        regex_lite::Regex::new(text)
            .map(|regex| Pattern {
                text: text.to_string(),
                regex,
            })
            .map_err(|e| {
                format!("`matches` takes a regular expression, and {text:?} isn't one: {e}")
            })
    }

    /// The text it was written as.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Test {
    /// Reads the test named `key` with its value, or says what's wrong with it.
    pub fn parse(key: &str, value: Value) -> Result<Test, String> {
        let number = |value: &Value| {
            value
                .as_f64()
                .ok_or_else(|| format!("`{key}` takes a number, not {value}"))
        };
        Ok(match key {
            "is" => Test::Is(value),
            "in" => match value {
                Value::Array(items) => Test::In(items),
                other => return Err(format!("`in` takes a list of values, not {other}")),
            },
            "empty" => match value {
                Value::Bool(b) => Test::Empty(b),
                other => return Err(format!("`empty` takes true or false, not {other}")),
            },
            "gt" => Test::Gt(number(&value)?),
            "gte" => Test::Gte(number(&value)?),
            "lt" => Test::Lt(number(&value)?),
            "lte" => Test::Lte(number(&value)?),
            "min_len" | "max_len" => {
                let n = value
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| format!("`{key}` takes a whole number, not {value}"))?;
                if key == "min_len" {
                    Test::MinLen(n)
                } else {
                    Test::MaxLen(n)
                }
            }
            "matches" => match value {
                Value::String(text) => Test::Matches(Pattern::new(&text)?),
                other => return Err(format!("`matches` takes a regular expression, not {other}")),
            },
            "status" => match value.as_str() {
                Some(status) if STATUSES.contains(&status) => Test::Status(status.to_string()),
                _ => {
                    return Err(format!(
                        "`status` is one of {}, not {value}",
                        STATUSES.map(|s| format!("{s:?}")).join(", ")
                    ));
                }
            },
            other => {
                return Err(format!(
                    "there's no test `{other}`; a test is one of {}",
                    TESTS.map(|t| format!("`{t}`")).join(", ")
                ));
            }
        })
    }

    /// The test's key and value, as it's written.
    pub fn written(&self) -> (&'static str, Value) {
        let number = |n: f64| serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number);
        match self {
            Test::Is(v) => ("is", v.clone()),
            Test::In(v) => ("in", Value::Array(v.clone())),
            Test::Empty(b) => ("empty", Value::Bool(*b)),
            Test::Gt(n) => ("gt", number(*n)),
            Test::Gte(n) => ("gte", number(*n)),
            Test::Lt(n) => ("lt", number(*n)),
            Test::Lte(n) => ("lte", number(*n)),
            Test::Matches(p) => ("matches", Value::String(p.text.clone())),
            Test::MinLen(n) => ("min_len", Value::from(*n)),
            Test::MaxLen(n) => ("max_len", Value::from(*n)),
            Test::Status(s) => ("status", Value::String(s.clone())),
        }
    }

    /// Whether `value` (`None` when it's missing) passes. `Status` isn't tested here, since it
    /// reads the source's status, not its value; see [`Condition::holds`].
    pub fn passes(&self, value: Option<&Value>) -> bool {
        let number = value.and_then(Value::as_f64);
        match self {
            Test::Is(want) => value.is_some_and(|v| same(v, want)),
            Test::In(wants) => value.is_some_and(|v| wants.iter().any(|w| same(v, w))),
            Test::Empty(want) => is_empty(value) == *want,
            Test::Gt(n) => number.is_some_and(|v| v > *n),
            Test::Gte(n) => number.is_some_and(|v| v >= *n),
            Test::Lt(n) => number.is_some_and(|v| v < *n),
            Test::Lte(n) => number.is_some_and(|v| v <= *n),
            Test::Matches(p) => value
                .and_then(Value::as_str)
                .is_some_and(|text| p.regex.is_match(text)),
            Test::MinLen(n) => length(value).is_some_and(|len| len >= *n),
            Test::MaxLen(n) => length(value).is_some_and(|len| len <= *n),
            Test::Status(_) => false,
        }
    }
}

/// Whether two values are equal: numbers by value, everything else exactly.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// The length of text, in characters, or of a list, in items; `None` for anything else.
fn length(value: Option<&Value>) -> Option<usize> {
    match value? {
        Value::String(text) => Some(text.chars().count()),
        Value::Array(items) => Some(items.len()),
        _ => None,
    }
}

/// Missing, `null`, `""`, `[]`, or `{}`.
pub fn is_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        _ => false,
    }
}

impl Condition {
    /// Whether it holds, looking variables up with `look`. A `status` test on `app.NAME` reads
    /// `sys.sources.NAME`.
    pub fn holds(&self, look: &dyn Fn(&str) -> Option<Value>) -> bool {
        match self {
            Condition::All(all) => all.iter().all(|c| c.holds(look)),
            Condition::Any(any) => any.iter().any(|c| c.holds(look)),
            Condition::Not(not) => !not.holds(look),
            Condition::Test {
                var,
                test: Test::Status(want),
            } => var
                .strip_prefix("app.")
                .and_then(|name| look(&format!("sys.sources.{name}")))
                .is_some_and(|status| status.as_str() == Some(want)),
            Condition::Test { var, test } => test.passes(look(var).as_ref()),
        }
    }

    /// Every test in it, with its variable, in the order written.
    pub fn tests(&self) -> Vec<(&str, &Test)> {
        match self {
            Condition::All(list) | Condition::Any(list) => {
                list.iter().flat_map(Condition::tests).collect()
            }
            Condition::Not(not) => not.tests(),
            Condition::Test { var, test } => vec![(var.as_str(), test)],
        }
    }
}

impl TryFrom<Value> for Condition {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        let Value::Object(mut fields) = value else {
            return Err(format!(
                "a condition is a table, like {{ var = \"app.view\", is = \"contacts\" }}, not {value}"
            ));
        };
        for combine in ["all", "any", "not"] {
            if let Some(inner) = fields.remove(combine) {
                if let Some(other) = fields.keys().next() {
                    return Err(format!(
                        "`{combine}` stands alone in its table, so `{other}` can't be beside it; nest it inside"
                    ));
                }
                return match (combine, inner) {
                    ("not", inner) => Ok(Condition::Not(Box::new(Condition::try_from(inner)?))),
                    (_, Value::Array(items)) if !items.is_empty() => {
                        let list = items
                            .into_iter()
                            .map(Condition::try_from)
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(if combine == "all" {
                            Condition::All(list)
                        } else {
                            Condition::Any(list)
                        })
                    }
                    (_, other) => Err(format!(
                        "`{combine}` takes a list of one or more conditions, not {other}"
                    )),
                };
            }
        }
        let var = match fields.remove("var") {
            Some(Value::String(var)) => var,
            Some(other) => {
                return Err(format!(
                    "`var` is a variable's path, like \"app.view\", not {other}"
                ));
            }
            None => {
                return Err(
                    "a condition names a variable with `var` and makes one test, or combines conditions with `all`, `any`, or `not`".into(),
                );
            }
        };
        let mut tests: Vec<(String, Value)> = fields.into_iter().collect();
        match tests.len() {
            1 => {
                let (key, value) = tests.remove(0);
                Ok(Condition::Test {
                    test: Test::parse(&key, value)?,
                    var,
                })
            }
            0 => Err(format!(
                "a condition on {var} makes one test: {}",
                TESTS.map(|t| format!("`{t}`")).join(", ")
            )),
            _ => {
                let keys: Vec<String> = tests.iter().map(|(k, _)| format!("`{k}`")).collect();
                Err(format!(
                    "a condition makes one test, and this one has {}; combine them with `all`",
                    keys.join(" and ")
                ))
            }
        }
    }
}

impl From<Condition> for Value {
    fn from(condition: Condition) -> Value {
        let mut fields = Map::new();
        match condition {
            Condition::All(list) => {
                fields.insert("all".into(), list.into_iter().map(Value::from).collect());
            }
            Condition::Any(list) => {
                fields.insert("any".into(), list.into_iter().map(Value::from).collect());
            }
            Condition::Not(not) => {
                fields.insert("not".into(), Value::from(*not));
            }
            Condition::Test { var, test } => {
                let (key, value) = test.written();
                fields.insert("var".into(), Value::String(var));
                fields.insert(key.into(), value);
            }
        }
        Value::Object(fields)
    }
}

/// A check on a control's own value ([spec 080](../../../specs/080_display_logic.md), "Checks on
/// input"): one or more tests, which all have to pass, and the message shown when they don't.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub struct Check {
    /// Its tests, in the order written.
    pub tests: Vec<Test>,
    /// What the control shows while it fails.
    pub message: String,
}

impl Check {
    /// `required = true`: not empty, or for a checkbox or toggle, ticked.
    pub fn required(message: Option<String>, ticked: bool) -> Check {
        Check {
            tests: vec![if ticked {
                Test::Is(Value::Bool(true))
            } else {
                Test::Empty(false)
            }],
            message: message.unwrap_or_else(|| "Required.".into()),
        }
    }

    /// Whether `value` passes. An empty value passes every check but one that tests `empty` or
    /// `is`, so an optional field left blank isn't wrong; `required` is what asks for a value.
    pub fn passes(&self, value: Option<&Value>) -> bool {
        let asks = |t: &Test| matches!(t, Test::Empty(_) | Test::Is(_) | Test::In(_));
        if is_empty(value) && !self.tests.iter().any(asks) {
            return true;
        }
        self.tests.iter().all(|test| test.passes(value))
    }
}

impl TryFrom<Value> for Check {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        let Value::Object(mut fields) = value else {
            return Err(format!(
                "a check is a table, like {{ matches = \"@\", message = \"Needs an @.\" }}, not {value}"
            ));
        };
        let message = match fields.remove("message") {
            Some(Value::String(message)) => message,
            Some(other) => return Err(format!("a check's `message` is text, not {other}")),
            None => return Err("a check says what's wrong with `message`".into()),
        };
        if let Some(key) = ["var", "all", "any", "not"]
            .iter()
            .find(|k| fields.contains_key(**k))
        {
            return Err(format!(
                "a check tests its own control's value, so it takes no `{key}`; give it tests like `min_len` and a `message`"
            ));
        }
        if fields.is_empty() {
            return Err(format!(
                "a check makes at least one test: {}",
                TESTS.map(|t| format!("`{t}`")).join(", ")
            ));
        }
        let tests = fields
            .into_iter()
            .map(|(key, value)| match key.as_str() {
                "status" => {
                    Err("`status` tests a source, so it can't be a check on a control".into())
                }
                _ => Test::parse(&key, value),
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Check { tests, message })
    }
}

impl From<Check> for Value {
    fn from(check: Check) -> Value {
        let mut fields: Map<String, Value> = check
            .tests
            .iter()
            .map(|t| {
                let (key, value) = t.written();
                (key.to_string(), value)
            })
            .collect();
        fields.insert("message".into(), Value::String(check.message));
        Value::Object(fields)
    }
}

/// `required`: `true`, or the message to show in place of "Required.".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum Required {
    /// `required = true` (or `false`, which asks nothing).
    Yes(bool),
    /// `required = "Pick a plan."`.
    Says(String),
}

impl Required {
    /// `None` when it asks nothing; otherwise its own message, if it has one.
    pub fn message(&self) -> Option<Option<String>> {
        match self {
            Required::Yes(false) => None,
            Required::Yes(true) => Some(None),
            Required::Says(message) => Some(Some(message.clone())),
        }
    }
}

/// A control's checks, `required` first. `ticked` is for a checkbox or toggle, where `required`
/// means ticked.
pub fn control_checks(required: Option<&Required>, checks: &[Check], ticked: bool) -> Vec<Check> {
    let required = required
        .and_then(Required::message)
        .map(|message| Check::required(message, ticked));
    required.into_iter().chain(checks.iter().cloned()).collect()
}

impl TryFrom<Value> for Required {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        match value {
            Value::Bool(yes) => Ok(Required::Yes(yes)),
            Value::String(message) => Ok(Required::Says(message)),
            other => Err(format!(
                "`required` is true, or the message to show, like \"Pick a plan.\", not {other}"
            )),
        }
    }
}

impl From<Required> for Value {
    fn from(required: Required) -> Value {
        match required {
            Required::Yes(yes) => Value::Bool(yes),
            Required::Says(message) => Value::String(message),
        }
    }
}
