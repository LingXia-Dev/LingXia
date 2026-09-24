//! Declarative scenario files and relative-time templates.
//!
//! A scenario is JSON: `{ name?, description?, routes: [...] }`. Each route
//! is `{ url, method?, times?, note? }` plus one answer in the route handler
//! shape (`status`/`json`/`body`/…, `abort`, `continue`, `hang`, `sse`) or a
//! `sequence` of them, served in call order with the last one repeating.
//! Files may also carry `bodyBase64` for a binary body. String values may
//! contain `{{now}}`, `{{now-2h}}`, `{{now+30m}}` (ISO-8601 UTC) and
//! `{{nowMs}}`, rendered each time the answer is served.

use super::parse_handler_value;
use super::registry::{
    Fulfill, ResponseBody, RouteAction, RouteSpec, SseAnswer, SseStep, UrlMatcher,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::Value;

/// Routes one scenario may install.
pub(crate) const MAX_SCENARIO_ROUTES: usize = 200;
/// Answers one `sequence` may list.
pub(crate) const MAX_SEQUENCE: usize = 100;

const SCENARIO_KEYS: [&str; 4] = ["$schema", "name", "description", "routes"];
const ROUTE_KEYS: [&str; 5] = ["url", "method", "times", "note", "description"];

/// A parsed scenario, routes in file order.
#[derive(Debug, Clone)]
pub(crate) struct Scenario {
    pub name: Option<String>,
    pub routes: Vec<RouteSpec>,
}

/// Parse and validate a scenario file. Errors name the offending route as
/// `routes[i]`.
pub(crate) fn parse_scenario(value: &Value) -> Result<Scenario, String> {
    let Value::Object(fields) = value else {
        return Err("a scenario must be a JSON object with a routes array".into());
    };
    if let Some(unknown) = fields
        .keys()
        .find(|key| !SCENARIO_KEYS.contains(&key.as_str()))
    {
        return Err(format!(
            "unknown scenario field '{unknown}' (allowed: {})",
            SCENARIO_KEYS.join(", ")
        ));
    }
    let name = match fields.get("name") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(name.clone()),
        Some(_) => return Err("scenario name must be a string".into()),
    };
    if !matches!(
        fields.get("description"),
        None | Some(Value::Null) | Some(Value::String(_))
    ) {
        return Err("scenario description must be a string".into());
    }
    let routes = match fields.get("routes") {
        Some(Value::Array(routes)) if !routes.is_empty() => routes,
        Some(Value::Array(_)) => return Err("scenario routes must not be empty".into()),
        Some(_) => return Err("scenario routes must be an array".into()),
        None => return Err("a scenario needs a routes array".into()),
    };
    if routes.len() > MAX_SCENARIO_ROUTES {
        return Err(format!(
            "a scenario may list at most {MAX_SCENARIO_ROUTES} routes, got {}",
            routes.len()
        ));
    }
    let routes = routes
        .iter()
        .enumerate()
        .map(|(index, route)| parse_route(route).map_err(|err| format!("routes[{index}]: {err}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Scenario { name, routes })
}

fn parse_route(value: &Value) -> Result<RouteSpec, String> {
    let Value::Object(fields) = value else {
        return Err("a route must be an object with a url".into());
    };
    let url = match fields.get("url") {
        Some(Value::String(url)) => url,
        Some(_) => return Err("url must be a glob or a /regex/flags string".into()),
        None => return Err("a route needs a url".into()),
    };
    let matcher = parse_url_string(url)?;
    let method = match fields.get("method") {
        None | Some(Value::Null) => None,
        Some(Value::String(method)) => parse_method(method)?,
        Some(_) => return Err("method must be a string".into()),
    };
    let times = match fields.get("times") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => Some(
            n.as_u64()
                .filter(|times| (1..=u64::from(u32::MAX)).contains(times))
                .map(|times| times as u32)
                .ok_or_else(|| format!("times must be a positive integer, got {n}"))?,
        ),
        Some(other) => return Err(format!("times must be a positive integer, got {other}")),
    };
    if !matches!(
        fields.get("note"),
        None | Some(Value::Null) | Some(Value::String(_))
    ) {
        return Err("note must be a string".into());
    }
    let answer: serde_json::Map<String, Value> = fields
        .iter()
        .filter(|(key, _)| !ROUTE_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if answer.is_empty() {
        return Err(
            "a route needs an answer (status/json/body, abort, continue, hang, sse) or a sequence"
                .into(),
        );
    }
    let answers = parse_file_answers(&Value::Object(answer))?;
    Ok(RouteSpec::new(matcher, method, times, answers))
}

/// Upper-case method, `None` for any. Shared with `route()` patterns.
pub(crate) fn parse_method(method: &str) -> Result<Option<String>, String> {
    let method = method.trim().to_ascii_uppercase();
    if method.is_empty() || method == "*" {
        return Ok(None);
    }
    if !method.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-') {
        return Err(format!("invalid route method '{method}'"));
    }
    Ok(Some(method))
}

/// A glob, or a regex written `/source/flags`.
pub(crate) fn parse_url_string(url: &str) -> Result<UrlMatcher, String> {
    if let Some(rest) = url.strip_prefix('/')
        && let Some((source, flags)) = rest.rsplit_once('/')
        && flags.chars().all(|c| c.is_ascii_alphabetic())
        && !source.is_empty()
    {
        return UrlMatcher::regex(source, flags);
    }
    UrlMatcher::glob(url)
}

/// An answer or `{ sequence: [...] }` from a file: like a route handler,
/// plus `bodyBase64` for binary bodies.
fn parse_file_answers(value: &Value) -> Result<Vec<RouteAction>, String> {
    parse_answers_with(value, &parse_file_answer)
}

fn parse_file_answer(value: &Value) -> Result<RouteAction, String> {
    let Value::Object(fields) = value else {
        return Err("an answer must be an object".into());
    };
    let Some(encoded) = fields.get("bodyBase64") else {
        return parse_handler_value(value, None);
    };
    if fields.contains_key("body") || fields.contains_key("json") {
        return Err("an answer takes one of body, json, or bodyBase64".into());
    }
    let Value::String(encoded) = encoded else {
        return Err("bodyBase64 must be a base64 string".into());
    };
    let bytes = BASE64
        .decode(encoded.trim())
        .map_err(|err| format!("bodyBase64 is not valid base64: {err}"))?;
    let mut fields = fields.clone();
    fields.remove("bodyBase64");
    fields.insert("body".into(), Value::Null);
    parse_handler_value(&Value::Object(fields), Some(bytes))
}

/// `{ sequence: [a, b, …] }` or a single answer, each parsed by `one`.
pub(crate) fn parse_answers_with(
    value: &Value,
    one: &dyn Fn(&Value) -> Result<RouteAction, String>,
) -> Result<Vec<RouteAction>, String> {
    let Value::Object(fields) = value else {
        return Err("route handler must be an object".into());
    };
    let Some(sequence) = fields.get("sequence") else {
        return one(value).map(|answer| vec![answer]);
    };
    if let Some(other) = fields.keys().find(|key| *key != "sequence") {
        return Err(format!(
            "a sequence lists whole answers; move '{other}' into its items"
        ));
    }
    let Value::Array(items) = sequence else {
        return Err("sequence must be an array of answers".into());
    };
    if items.is_empty() || items.len() > MAX_SEQUENCE {
        return Err(format!(
            "sequence must list 1..={MAX_SEQUENCE} answers, got {}",
            items.len()
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if item.get("sequence").is_some() {
                return Err(format!("sequence[{index}]: sequences do not nest"));
            }
            one(item).map_err(|err| format!("sequence[{index}]: {err}"))
        })
        .collect()
}

// ------------------------------- templates -------------------------------

/// Render the relative-time templates in `text` at `now_ms`, or `None` when
/// it has none. Unknown `{{…}}` text stays as written.
pub(crate) fn render_templates(text: &str, now_ms: u64) -> Option<String> {
    if !text.contains("{{") {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    while let Some(start) = rest.find("{{") {
        let Some(len) = rest[start + 2..].find("}}") else {
            break;
        };
        let inner = &rest[start + 2..start + 2 + len];
        out.push_str(&rest[..start]);
        match template_value(inner.trim(), now_ms) {
            Some(value) => {
                out.push_str(&value);
                changed = true;
            }
            None => out.push_str(&rest[start..start + 4 + len]),
        }
        rest = &rest[start + 4 + len..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

fn template_value(inner: &str, now_ms: u64) -> Option<String> {
    let (millis, rest) = match inner.strip_prefix("nowMs") {
        Some(rest) => (true, rest),
        None => (false, inner.strip_prefix("now")?),
    };
    let offset = if rest.is_empty() {
        0
    } else {
        parse_offset(rest.trim())?
    };
    let at = i128::from(now_ms) + offset;
    let at = i64::try_from(at).ok()?;
    Some(if millis { at.to_string() } else { iso_utc(at) })
}

/// `+30m`, `-2h`, `+1d`, `-500ms`, `+10s` in milliseconds.
fn parse_offset(text: &str) -> Option<i128> {
    let (sign, rest) = match text.as_bytes().first()? {
        b'+' => (1i128, &text[1..]),
        b'-' => (-1i128, &text[1..]),
        _ => return None,
    };
    let rest = rest.trim_start();
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let amount: i128 = rest[..digits].parse().ok()?;
    let unit = match rest[digits..].trim() {
        "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        _ => return None,
    };
    Some(sign * amount * unit)
}

/// Epoch milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ`, like JS `toISOString`.
pub(crate) fn iso_utc(epoch_ms: i64) -> String {
    let days = epoch_ms.div_euclid(86_400_000);
    let ms_of_day = epoch_ms.rem_euclid(86_400_000);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, rem) = (ms_of_day / 3_600_000, ms_of_day % 3_600_000);
    let (m, rem) = (rem / 60_000, rem % 60_000);
    let (s, ms) = (rem / 1_000, rem % 1_000);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{ms:03}Z")
}

fn render_string(text: &mut String, now_ms: u64) {
    if let Some(rendered) = render_templates(text, now_ms) {
        *text = rendered;
    }
}

fn render_json(value: &mut Value, now_ms: u64) {
    match value {
        Value::String(text) => render_string(text, now_ms),
        Value::Array(items) => items.iter_mut().for_each(|item| render_json(item, now_ms)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|field| render_json(field, now_ms)),
        _ => {}
    }
}

/// `action` with its templates rendered at `now_ms`. Binary bodies are
/// served as given.
pub(crate) fn render_action(action: &RouteAction, now_ms: u64) -> RouteAction {
    let mut action = action.clone();
    match &mut action {
        RouteAction::Fulfill(Fulfill {
            status_text,
            headers,
            body,
            ..
        }) => {
            if let Some(text) = status_text {
                render_string(text, now_ms);
            }
            for (_, value) in headers.iter_mut() {
                render_string(value, now_ms);
            }
            if let Some(ResponseBody::Text(text)) = body {
                render_string(text, now_ms);
            }
        }
        RouteAction::Patch(patch) => render_json(patch, now_ms),
        RouteAction::Sse(SseAnswer { headers, steps, .. }) => {
            for (_, value) in headers.iter_mut() {
                render_string(value, now_ms);
            }
            for step in steps.iter_mut() {
                match step {
                    SseStep::Event {
                        event, data, id, ..
                    } => {
                        render_string(data, now_ms);
                        if let Some(event) = event {
                            render_string(event, now_ms);
                        }
                        if let Some(id) = id {
                            render_string(id, now_ms);
                        }
                    }
                    SseStep::Comment(text) => render_string(text, now_ms),
                    SseStep::Delay(_) | SseStep::Drop => {}
                }
            }
        }
        RouteAction::Abort(_) | RouteAction::Continue | RouteAction::Hang { .. } => {}
    }
    action
}
