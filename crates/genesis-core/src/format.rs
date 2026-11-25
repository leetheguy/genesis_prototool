//! Formats: how a value looks, never what it is ([spec 080](../../../specs/080_display_logic.md),
//! "Formats").
//!
//! A `text` node's `format` applies to every value its templates fill in. Like a condition, a
//! format is a TOML table, never an expression: `{ currency = "USD" }`, `{ date = "%b %d, %Y" }`.
//!
//! The logic is all here, and the words and digits a language writes come from a [`Locale`]. The
//! browser's comes from its `Intl` (in genesis-ui); [`English`] is the one used everywhere else,
//! and in tests. A value a format can't handle is an `Err`, and is shown as it is.
//!
//! ```
//! use genesis_core::format::{English, Format};
//! use serde_json::json;
//!
//! let money: Format = serde_json::from_value(json!({ "currency": "USD" })).unwrap();
//! assert_eq!(money.apply(&json!(1234.5), &English::at(0)), Ok("$1,234.50".into()));
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A format: one key, and what it takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum Format {
    /// `{ number = 2 }`: grouped, with that many decimals.
    Number(u8),
    /// `{ currency = "USD" }`: money in that currency, by its ISO 4217 code.
    Currency(String),
    /// `{ percent = 1 }`: the number × 100, with that many decimals and `%`.
    Percent(u8),
    /// `{ date = "%b %d, %Y" }`: a date or time, by a strftime pattern.
    Date(DatePattern),
    /// `{ ago = true }`: how long ago (or how far ahead) a time is: `3 hours ago`.
    Ago,
    /// `{ bytes = true }`: a size: `1.2 KB`.
    Bytes,
    /// `{ case = "upper" }`: text in `upper`, `lower`, or `title` case.
    Case(Case),
    /// `{ truncate = 80 }`: text cut to that many characters, `…` included.
    Truncate(usize),
    /// `{ map = { active = "Active" } }`: the label for each value; others as they are.
    Map(Map<String, Value>),
}

/// The formats, in the order the spec gives them.
pub const FORMATS: [&str; 9] = [
    "number", "currency", "percent", "date", "ago", "bytes", "case", "truncate", "map",
];

/// A text case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// `HELLO WORLD`
    Upper,
    /// `hello world`
    Lower,
    /// `Hello World`
    Title,
}

/// How a number is written out, which a [`Locale`] does in its own language.
#[derive(Debug, Clone, PartialEq)]
pub enum Style {
    /// Grouped, with at least `min` and at most `max` decimals.
    Decimal {
        /// The fewest decimals shown.
        min: u8,
        /// The most decimals shown.
        max: u8,
    },
    /// Money, by its ISO 4217 code, with the currency's own decimals.
    Currency(String),
    /// A share: `0.123` is `12.3%`, with that many decimals.
    Percent(u8),
}

/// A unit of time, for [`Locale::ago`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Seconds.
    Second,
    /// Minutes.
    Minute,
    /// Hours.
    Hour,
    /// Days.
    Day,
    /// Months, of 30 days.
    Month,
    /// Years, of 365 days.
    Year,
}

impl Unit {
    /// Its name, as `Intl.RelativeTimeFormat` takes it: `"second"`, `"hour"`, …
    pub fn name(self) -> &'static str {
        match self {
            Unit::Second => "second",
            Unit::Minute => "minute",
            Unit::Hour => "hour",
            Unit::Day => "day",
            Unit::Month => "month",
            Unit::Year => "year",
        }
    }
}

/// What a language writes: digits and separators, names of months and days, and how long ago.
pub trait Locale {
    /// `n`, written out in `style`.
    fn number(&self, n: f64, style: &Style) -> String;
    /// A month's name, `0` being January; `long` is `January`, otherwise `Jan`.
    fn month(&self, month: u32, long: bool) -> String;
    /// A weekday's name, `0` being Sunday; `long` is `Sunday`, otherwise `Sun`.
    fn weekday(&self, day: u32, long: bool) -> String;
    /// `AM` or `PM`, or the language's own.
    fn day_period(&self, pm: bool) -> String;
    /// `amount` of `unit` from now: `-3` hours is `3 hours ago`, `2` days is `in 2 days`.
    fn ago(&self, amount: i64, unit: Unit) -> String;
    /// The local time zone's offset from UTC, in minutes, at the time `ms` after 1970.
    fn offset_minutes(&self, ms: i64) -> i32;
    /// Now, in milliseconds since 1970.
    fn now_ms(&self) -> i64;
}

/// US English, in UTC, at a fixed now: what genesis-core uses outside a browser, and tests use.
#[derive(Debug, Clone, Copy)]
pub struct English {
    now_ms: i64,
}

impl English {
    /// English with `now` at `now_ms` milliseconds after 1970.
    pub fn at(now_ms: i64) -> English {
        English { now_ms }
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

impl Locale for English {
    fn number(&self, n: f64, style: &Style) -> String {
        match style {
            Style::Decimal { min, max } => grouped(n, *min, *max),
            Style::Percent(digits) => format!("{}%", grouped(n * 100.0, *digits, *digits)),
            Style::Currency(code) => {
                let digits = currency_digits(code);
                let amount = grouped(n.abs(), digits, digits);
                let sign = if n < 0.0 && amount.chars().any(|c| c.is_ascii_digit() && c != '0') {
                    "-"
                } else {
                    ""
                };
                match symbol(code) {
                    Some(symbol) => format!("{sign}{symbol}{amount}"),
                    None => format!("{sign}{code} {amount}"),
                }
            }
        }
    }

    fn month(&self, month: u32, long: bool) -> String {
        let name = MONTHS[month as usize % 12];
        if long { name.into() } else { name[..3].into() }
    }

    fn weekday(&self, day: u32, long: bool) -> String {
        let name = WEEKDAYS[day as usize % 7];
        if long { name.into() } else { name[..3].into() }
    }

    fn day_period(&self, pm: bool) -> String {
        if pm { "PM" } else { "AM" }.into()
    }

    fn ago(&self, amount: i64, unit: Unit) -> String {
        if unit == Unit::Second && amount.abs() < 10 {
            return "now".into();
        }
        let n = amount.unsigned_abs();
        let unit = unit.name();
        let plural = if n == 1 { "" } else { "s" };
        if amount < 0 {
            format!("{n} {unit}{plural} ago")
        } else {
            format!("in {n} {unit}{plural}")
        }
    }

    fn offset_minutes(&self, _ms: i64) -> i32 {
        0
    }

    fn now_ms(&self) -> i64 {
        self.now_ms
    }
}

/// How many decimals a currency has, for [`English`]: none for yen and won, else two.
fn currency_digits(code: &str) -> u8 {
    match code {
        "JPY" | "KRW" | "VND" | "CLP" | "ISK" => 0,
        _ => 2,
    }
}

/// The symbol US English writes for a currency, where it has one.
fn symbol(code: &str) -> Option<&'static str> {
    Some(match code {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "INR" => "₹",
        "KRW" => "₩",
        "CAD" => "CA$",
        "AUD" => "A$",
        "NZD" => "NZ$",
        "MXN" => "MX$",
        "CNY" => "CN¥",
        "BRL" => "R$",
        _ => return None,
    })
}

/// `n` rounded to at most `max` decimals (and at least `min`), its whole part grouped by
/// thousands with `,`, the way US English writes it.
fn grouped(n: f64, min: u8, max: u8) -> String {
    // Halves round away from zero, as `Intl` does: 0.5 is 1, not Rust's 0.
    let factor = 10f64.powi(i32::from(max));
    let fixed = format!("{:.*}", max as usize, (n.abs() * factor).round() / factor);
    let (whole, mut fraction) = match fixed.split_once('.') {
        Some((w, f)) => (w.to_string(), f.to_string()),
        None => (fixed.clone(), String::new()),
    };
    while fraction.len() > min as usize && fraction.ends_with('0') {
        fraction.pop();
    }
    let mut out = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(&fraction);
    }
    let zero = out.chars().all(|c| matches!(c, '0' | ',' | '.'));
    if n < 0.0 && !zero {
        out.insert(0, '-');
    }
    out
}

/// A strftime pattern, checked when it loads.
#[derive(Debug, Clone, PartialEq)]
pub struct DatePattern {
    text: String,
    pieces: Vec<DatePiece>,
}

/// One piece of a date pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DatePiece {
    /// A run of plain text, by its byte range in the pattern.
    Text(usize, usize),
    /// A field, by its letter, and whether it's padded with `0` (`%-d` isn't).
    Field(char, bool),
}

/// The fields a date pattern can use, with what each writes.
pub const DATE_FIELDS: [(&str, &str); 15] = [
    ("%Y", "the year: 2026"),
    ("%y", "the year's last two digits: 26"),
    ("%m", "the month: 01 to 12"),
    ("%B", "the month's name: November"),
    ("%b", "the month's short name: Nov"),
    ("%d", "the day of the month: 01 to 31"),
    ("%e", "the day of the month, unpadded: 1 to 31"),
    ("%A", "the weekday: Saturday"),
    ("%a", "the weekday's short name: Sat"),
    ("%H", "the hour, 00 to 23"),
    ("%I", "the hour, 01 to 12"),
    ("%p", "AM or PM"),
    ("%M", "the minute"),
    ("%S", "the second"),
    ("%%", "a %"),
];

impl DatePattern {
    /// Reads a strftime pattern, or says what's wrong with it. `-` after `%` drops the padding:
    /// `%-d`, `%-m`, `%-H`, `%-I`.
    pub fn new(text: &str) -> Result<DatePattern, String> {
        let mut pieces = Vec::new();
        let mut start = 0;
        let mut chars = text.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if c != '%' {
                continue;
            }
            if start < at {
                pieces.push(DatePiece::Text(start, at));
            }
            let unpadded = chars.next_if(|&(_, c)| c == '-').is_some();
            let field = chars.next().map(|(_, c)| c);
            let known = "YymBbdeAaHIpMS%";
            match field {
                Some(f) if known.contains(f) && !(unpadded && !"mdHIMS".contains(f)) => {
                    pieces.push(DatePiece::Field(f, !unpadded));
                }
                _ => {
                    let written = &text[at..chars.peek().map_or(text.len(), |&(i, _)| i)];
                    return Err(format!(
                        "a date pattern takes {}, and {written:?} isn't one of them",
                        DATE_FIELDS.map(|(f, _)| f).join(", ")
                    ));
                }
            }
            start = chars.peek().map_or(text.len(), |&(i, _)| i);
        }
        if start < text.len() {
            pieces.push(DatePiece::Text(start, text.len()));
        }
        Ok(DatePattern {
            text: text.to_string(),
            pieces,
        })
    }

    /// The pattern as it was written.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    fn write(&self, at: &Civil, locale: &dyn Locale) -> String {
        let pad = |n: u32, padded: bool| {
            if padded {
                format!("{n:02}")
            } else {
                n.to_string()
            }
        };
        let hour12 = match at.hour % 12 {
            0 => 12,
            h => h,
        };
        self.pieces
            .iter()
            .map(|piece| match *piece {
                DatePiece::Text(from, to) => self.text[from..to].to_string(),
                DatePiece::Field(field, padded) => match field {
                    'Y' => at.year.to_string(),
                    'y' => format!("{:02}", at.year.rem_euclid(100)),
                    'm' => pad(at.month, padded),
                    'B' => locale.month(at.month - 1, true),
                    'b' => locale.month(at.month - 1, false),
                    'd' => pad(at.day, padded),
                    'e' => at.day.to_string(),
                    'A' => locale.weekday(at.weekday, true),
                    'a' => locale.weekday(at.weekday, false),
                    'H' => pad(at.hour, padded),
                    'I' => pad(hour12, padded),
                    'p' => locale.day_period(at.hour >= 12),
                    'M' => pad(at.minute, padded),
                    'S' => pad(at.second, padded),
                    _ => "%".into(),
                },
            })
            .collect()
    }
}

/// A day and time on the calendar, as a clock on the wall shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Civil {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    /// `0` is Sunday.
    weekday: u32,
}

/// What a date's text says: a day alone, a time with no zone (as the wall clock shows it), or an
/// instant, in milliseconds since 1970.
#[derive(Debug, Clone, Copy, PartialEq)]
enum When {
    /// `2026-11-21`, or `2026-11-21T09:30:00`: shown as written, in no time zone.
    Wall(Civil),
    /// `2026-11-21T09:30:00Z` or with an offset: shown in the local time zone.
    Instant(i64),
}

/// Days since 1970-01-01 for a date on the proleptic Gregorian calendar (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date for a count of days since 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl Civil {
    fn new(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Civil {
        let weekday = (days_from_civil(year, month, day) + 4).rem_euclid(7) as u32;
        Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
        }
    }

    /// The wall clock `ms` milliseconds after 1970, in UTC.
    fn from_ms(ms: i64) -> Civil {
        let seconds = ms.div_euclid(1000);
        let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
        let of_day = seconds.rem_euclid(86_400) as u32;
        Civil::new(
            year,
            month,
            day,
            of_day / 3600,
            of_day / 60 % 60,
            of_day % 60,
        )
    }

    /// Milliseconds since 1970, reading this wall clock as UTC.
    fn to_ms(self) -> i64 {
        let days = days_from_civil(self.year, self.month, self.day);
        let seconds = i64::from(self.hour * 3600 + self.minute * 60 + self.second);
        (days * 86_400 + seconds) * 1000
    }
}

/// Reads ISO 8601 / RFC 3339: `2026-11-21`, `2026-11-21T09:30`, `2026-11-21T09:30:00.5Z`,
/// `2026-11-21 09:30:00+01:00`. `None` for anything else.
fn parse_when(text: &str) -> Option<When> {
    let number = |s: &str| -> Option<u32> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok())?
    };
    let (date, rest) = match text.find(['T', 't', ' ']) {
        Some(i) => (&text[..i], Some(&text[i + 1..])),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    let (year, month, day) = (i64::from(number(year)?), number(month)?, number(day)?);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => return None,
    };
    if day == 0 || day > days_in_month {
        return None;
    }
    let Some(rest) = rest else {
        return Some(When::Wall(Civil::new(year, month, day, 0, 0, 0)));
    };
    // The zone, if any: `Z`, or `+HH:MM` / `-HH:MM` after the time.
    let (time, zone) = if let Some(time) = rest.strip_suffix(['Z', 'z']) {
        (time, Some(0))
    } else if let Some(i) = rest.rfind(['+', '-']) {
        let (sign, offset) = (if &rest[i..=i] == "-" { -1 } else { 1 }, &rest[i + 1..]);
        let (h, m) = offset
            .split_once(':')
            .unwrap_or((offset.get(..2)?, offset.get(2..)?));
        let minutes = i64::from(number(h)?) * 60 + i64::from(number(m)?);
        (&rest[..i], Some(sign * minutes))
    } else {
        (rest, None)
    };
    let mut fields = time.split(':');
    let hour = number(fields.next()?)?;
    let minute = number(fields.next()?)?;
    let second = match fields.next() {
        Some(s) => number(s.split('.').next()?)?,
        None => 0,
    };
    if fields.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let wall = Civil::new(year, month, day, hour, minute, second.min(59));
    Some(match zone {
        None => When::Wall(wall),
        Some(offset) => When::Instant(wall.to_ms() - offset * 60_000),
    })
}

impl Format {
    /// How `value` looks in this format, or why the format can't take it (it's then shown as it
    /// is).
    pub fn apply(&self, value: &Value, locale: &dyn Locale) -> Result<String, String> {
        let number = || {
            value
                .as_f64()
                .ok_or_else(|| format!("{} takes a number, not {value}", self.name()))
        };
        let text = || {
            value
                .as_str()
                .ok_or_else(|| format!("{} takes text, not {value}", self.name()))
        };
        match self {
            Format::Number(digits) => Ok(locale.number(
                number()?,
                &Style::Decimal {
                    min: *digits,
                    max: *digits,
                },
            )),
            Format::Currency(code) => Ok(locale.number(number()?, &Style::Currency(code.clone()))),
            Format::Percent(digits) => Ok(locale.number(number()?, &Style::Percent(*digits))),
            Format::Bytes => Ok(bytes(number()?, locale)),
            Format::Date(pattern) => {
                let when = parse_when(text()?).ok_or_else(|| {
                    format!("date takes a date like 2026-11-21T09:30:00Z, not {value}")
                })?;
                let wall = match when {
                    When::Wall(wall) => wall,
                    When::Instant(ms) => {
                        Civil::from_ms(ms + i64::from(locale.offset_minutes(ms)) * 60_000)
                    }
                };
                Ok(pattern.write(&wall, locale))
            }
            Format::Ago => {
                let ms = match parse_when(text()?) {
                    Some(When::Instant(ms)) => ms,
                    // A time with no zone is the local wall clock's.
                    Some(When::Wall(wall)) => {
                        let ms = wall.to_ms();
                        ms - i64::from(locale.offset_minutes(ms)) * 60_000
                    }
                    None => {
                        return Err(format!(
                            "ago takes a time like 2026-11-21T09:30:00Z, not {value}"
                        ));
                    }
                };
                let (amount, unit) = ago(ms - locale.now_ms());
                Ok(locale.ago(amount, unit))
            }
            Format::Case(case) => {
                let text = text()?;
                Ok(match case {
                    Case::Upper => text.to_uppercase(),
                    Case::Lower => text.to_lowercase(),
                    Case::Title => title(text),
                })
            }
            Format::Truncate(n) => {
                let text = text()?;
                if text.chars().count() <= *n {
                    return Ok(text.to_string());
                }
                let kept: String = text.chars().take(n.saturating_sub(1)).collect();
                Ok(format!("{}…", kept.trim_end()))
            }
            Format::Map(labels) => {
                let key = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(_) | Value::Bool(_) => value.to_string(),
                    _ => return Ok(crate::template::as_text(value)),
                };
                Ok(match labels.get(&key) {
                    Some(Value::String(label)) => label.clone(),
                    _ => crate::template::as_text(value),
                })
            }
        }
    }

    /// Its key: `number`, `currency`, …
    pub fn name(&self) -> &'static str {
        match self {
            Format::Number(_) => "number",
            Format::Currency(_) => "currency",
            Format::Percent(_) => "percent",
            Format::Date(_) => "date",
            Format::Ago => "ago",
            Format::Bytes => "bytes",
            Format::Case(_) => "case",
            Format::Truncate(_) => "truncate",
            Format::Map(_) => "map",
        }
    }
}

/// A size in bytes, by thousands: `999 B`, `1.2 KB`, `3.4 MB`.
fn bytes(n: f64, locale: &dyn Locale) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = n.abs();
    let mut unit = 0;
    while size >= 999.95 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    let size = if n < 0.0 { -size } else { size };
    let max = if unit == 0 { 0 } else { 1 };
    let shown = locale.number(size, &Style::Decimal { min: 0, max });
    format!("{shown} {}", UNITS[unit])
}

/// How far `ms` is from now, in the largest unit that fits, rounded toward now.
fn ago(ms: i64) -> (i64, Unit) {
    let seconds = ms / 1000;
    let s = seconds.abs();
    let (amount, unit) = match s {
        s if s < 60 => (s, Unit::Second),
        s if s < 3600 => (s / 60, Unit::Minute),
        s if s < 86_400 => (s / 3600, Unit::Hour),
        s if s < 30 * 86_400 => (s / 86_400, Unit::Day),
        s if s < 365 * 86_400 => (s / (30 * 86_400), Unit::Month),
        s => (s / (365 * 86_400), Unit::Year),
    };
    (if seconds < 0 { -amount } else { amount }, unit)
}

/// Each word's first letter upper case, and the rest lower.
fn title(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for c in text.chars() {
        if c.is_whitespace() {
            start = true;
            out.push(c);
        } else if start {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

impl TryFrom<Value> for Format {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, String> {
        let Value::Object(fields) = value else {
            return Err(format!(
                "a format is a table, like {{ currency = \"USD\" }}, not {value}"
            ));
        };
        let mut fields = fields.into_iter();
        let (Some((key, value)), None) = (fields.next(), fields.next()) else {
            return Err(format!(
                "a format has one key: {}",
                FORMATS.map(|f| format!("`{f}`")).join(", ")
            ));
        };
        let digits = |value: &Value| {
            value
                .as_u64()
                .filter(|n| *n <= 20)
                .map(|n| n as u8)
                .ok_or_else(|| format!("`{key}` takes how many decimals, 0 to 20, not {value}"))
        };
        let yes = |value: &Value| match value {
            Value::Bool(true) => Ok(()),
            other => Err(format!("`{key}` takes true, not {other}")),
        };
        Ok(match key.as_str() {
            "number" => Format::Number(digits(&value)?),
            "percent" => Format::Percent(digits(&value)?),
            "currency" => match value.as_str() {
                Some(code) if code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase()) => {
                    Format::Currency(code.to_string())
                }
                _ => {
                    return Err(format!(
                        "`currency` takes a three-letter currency code, like \"USD\", not {value}"
                    ));
                }
            },
            "date" => match value.as_str() {
                Some(pattern) => Format::Date(DatePattern::new(pattern)?),
                None => {
                    return Err(format!(
                        "`date` takes a pattern, like \"%b %d, %Y\", not {value}"
                    ));
                }
            },
            "ago" => {
                yes(&value)?;
                Format::Ago
            }
            "bytes" => {
                yes(&value)?;
                Format::Bytes
            }
            "case" => Format::Case(match value.as_str() {
                Some("upper") => Case::Upper,
                Some("lower") => Case::Lower,
                Some("title") => Case::Title,
                _ => {
                    return Err(format!(
                        "`case` is \"upper\", \"lower\", or \"title\", not {value}"
                    ));
                }
            }),
            "truncate" => match value.as_u64() {
                Some(n) if n >= 2 => Format::Truncate(n as usize),
                _ => {
                    return Err(format!(
                        "`truncate` takes how many characters to keep, 2 or more, not {value}"
                    ));
                }
            },
            "map" => match value {
                Value::Object(labels) => {
                    if let Some((k, v)) = labels.iter().find(|(_, v)| !v.is_string()) {
                        return Err(format!(
                            "`map` gives each value a label, and {k:?}'s is {v}, not text"
                        ));
                    }
                    Format::Map(labels)
                }
                other => {
                    return Err(format!(
                        "`map` takes a table of labels, like {{ active = \"Active\" }}, not {other}"
                    ));
                }
            },
            other => {
                return Err(format!(
                    "there's no format `{other}`; a format is one of {}",
                    FORMATS.map(|f| format!("`{f}`")).join(", ")
                ));
            }
        })
    }
}

impl From<Format> for Value {
    fn from(format: Format) -> Value {
        let value = match &format {
            Format::Number(n) | Format::Percent(n) => Value::from(*n),
            Format::Currency(code) => Value::String(code.clone()),
            Format::Date(pattern) => Value::String(pattern.text.clone()),
            Format::Ago | Format::Bytes => Value::Bool(true),
            Format::Case(case) => Value::String(
                match case {
                    Case::Upper => "upper",
                    Case::Lower => "lower",
                    Case::Title => "title",
                }
                .into(),
            ),
            Format::Truncate(n) => Value::from(*n),
            Format::Map(labels) => Value::Object(labels.clone()),
        };
        let mut fields = Map::new();
        fields.insert(format.name().into(), value);
        Value::Object(fields)
    }
}

/// What's wrong with a locale's name, if anything: it's a BCP 47 tag, like `en-GB` or `pt-BR`.
pub fn check_locale(locale: &str) -> Option<String> {
    let mut parts = locale.split('-');
    let language = parts.next().unwrap_or_default();
    let fine = (2..=3).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_lowercase())
        && parts
            .all(|p| (2..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()));
    (!fine)
        .then(|| format!("`locale` is a language tag, like \"en-GB\" or \"pt-BR\", not {locale:?}"))
}
