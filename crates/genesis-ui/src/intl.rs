//! The browser's own language, through `Intl`, for formats ([`genesis_core::format`]). Every call
//! into `Intl` goes through `Reflect`, so a language tag the browser refuses falls back to its own
//! language rather than throwing.

use genesis_core::format::{Locale, Style, Unit};
use js_sys::{Array, Date, Function, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};

/// The browser's language, or the one `[app] locale` names.
pub struct Browser {
    /// `[app] locale`, if it names one.
    pub tag: Option<String>,
}

/// A table of options, as `Intl` takes them.
fn options(pairs: &[(&str, JsValue)]) -> Object {
    let options = Object::new();
    for (key, value) in pairs {
        let _ = Reflect::set(&options, &JsValue::from_str(key), value);
    }
    options
}

impl Browser {
    /// `new Intl.KIND(tag, options)`, with the browser's own language if the tag is refused.
    fn make(&self, kind: &str, options: &Object) -> Option<JsValue> {
        let intl = Reflect::get(&js_sys::global(), &JsValue::from_str("Intl")).ok()?;
        let made: Function = Reflect::get(&intl, &JsValue::from_str(kind))
            .ok()?
            .dyn_into()
            .ok()?;
        let tags = Array::new();
        if let Some(tag) = &self.tag {
            tags.push(&JsValue::from_str(tag));
        }
        Reflect::construct(&made, &Array::of2(&tags, options))
            .or_else(|_| Reflect::construct(&made, &Array::of2(&Array::new(), options)))
            .ok()
    }

    /// `formatter.METHOD(args...)` as text.
    fn call(formatter: &JsValue, method: &str, args: &[JsValue]) -> Option<String> {
        let method: Function = Reflect::get(formatter, &JsValue::from_str(method))
            .ok()?
            .dyn_into()
            .ok()?;
        let args: Array = args.iter().collect();
        method.apply(formatter, &args).ok()?.as_string()
    }

    /// A date's part named by `option` (`month`, `weekday`) in `style` (`long`, `short`), in UTC.
    fn part(&self, ms: f64, option: &str, style: &str) -> Option<String> {
        let formatter = self.make(
            "DateTimeFormat",
            &options(&[(option, style.into()), ("timeZone", "UTC".into())]),
        )?;
        Self::call(&formatter, "format", &[Date::new(&ms.into()).into()])
    }
}

impl Locale for Browser {
    fn number(&self, n: f64, style: &Style) -> String {
        let pairs: Vec<(&str, JsValue)> = match style {
            Style::Decimal { min, max } => vec![
                ("minimumFractionDigits", (*min).into()),
                ("maximumFractionDigits", (*max).into()),
            ],
            Style::Currency(code) => vec![
                ("style", "currency".into()),
                ("currency", code.as_str().into()),
            ],
            Style::Percent(digits) => vec![
                ("style", "percent".into()),
                ("minimumFractionDigits", (*digits).into()),
                ("maximumFractionDigits", (*digits).into()),
            ],
        };
        self.make("NumberFormat", &options(&pairs))
            .and_then(|f| Self::call(&f, "format", &[n.into()]))
            .unwrap_or_else(|| n.to_string())
    }

    fn month(&self, month: u32, long: bool) -> String {
        let ms = Date::utc(2000.0, f64::from(month));
        self.part(ms, "month", if long { "long" } else { "short" })
            .unwrap_or_else(|| (month + 1).to_string())
    }

    fn weekday(&self, day: u32, long: bool) -> String {
        // 2 January 2000 was a Sunday.
        let ms = Date::utc(2000.0, 0.0) + f64::from(day + 1) * 86_400_000.0;
        self.part(ms, "weekday", if long { "long" } else { "short" })
            .unwrap_or_default()
    }

    fn day_period(&self, pm: bool) -> String {
        let hour = if pm { 15.0 } else { 9.0 };
        let ms = Date::utc(2000.0, 0.0) + hour * 3_600_000.0;
        let formatter = self.make(
            "DateTimeFormat",
            &options(&[
                ("hour", "numeric".into()),
                ("hour12", true.into()),
                ("timeZone", "UTC".into()),
            ]),
        );
        let parts = formatter.and_then(|f| {
            let method: Function = Reflect::get(&f, &"formatToParts".into())
                .ok()?
                .dyn_into()
                .ok()?;
            method
                .call1(&f, &Date::new(&ms.into()))
                .ok()?
                .dyn_into::<Array>()
                .ok()
        });
        parts
            .map(|parts| parts.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .find(|part| {
                Reflect::get(part, &"type".into())
                    .ok()
                    .and_then(|t| t.as_string())
                    .as_deref()
                    == Some("dayPeriod")
            })
            .and_then(|part| Reflect::get(&part, &"value".into()).ok()?.as_string())
            .unwrap_or_else(|| if pm { "PM" } else { "AM" }.into())
    }

    fn ago(&self, amount: i64, unit: Unit) -> String {
        self.make(
            "RelativeTimeFormat",
            &options(&[("numeric", "auto".into())]),
        )
        .and_then(|f| Self::call(&f, "format", &[(amount as f64).into(), unit.name().into()]))
        .unwrap_or_else(|| genesis_core::format::English::at(0).ago(amount, unit))
    }

    fn offset_minutes(&self, ms: i64) -> i32 {
        // The browser counts minutes behind UTC; a locale counts them ahead.
        -(Date::new(&(ms as f64).into()).get_timezone_offset() as i32)
    }

    fn now_ms(&self) -> i64 {
        Date::now() as i64
    }
}
