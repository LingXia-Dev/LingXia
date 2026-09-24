//! Watching Logic `fetch` without changing it: the bounded call log that
//! failed specs carry into their report, and recording real traffic into a
//! scenario file. Pure Rust; the `fetch` wrapper feeds it through
//! `observe`/`settle`.

use super::registry::{Decision, UrlMatcher, now_ms};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Value, json};
use std::collections::VecDeque;

/// Calls kept per process; the oldest are dropped first.
pub(crate) const MAX_CALLS: usize = 200;
/// Calls a failed spec attaches to its report.
pub(crate) const REPORT_CALLS: usize = 20;
/// Longest error text kept per call.
const MAX_ERROR_CHARS: usize = 300;
/// Exchanges one recording keeps; later ones are counted, not stored.
pub(crate) const MAX_EXCHANGES: usize = 500;
/// A text body larger than this is noted instead of recorded.
pub(crate) const MAX_TEXT_BODY_BYTES: usize = 1024 * 1024;
/// A binary body larger than this is noted instead of recorded.
pub(crate) const MAX_BINARY_BODY_BYTES: usize = 64 * 1024;
/// Bodies one recording keeps in total.
const MAX_RECORDED_BYTES: usize = 16 * 1024 * 1024;
/// What a redacted value becomes in a report or a recorded body.
pub(crate) const REDACTED: &str = "***";

/// JSON fields whose values are never recorded. Compared case-insensitively
/// with `_` and `-` removed, so `access_token` and `accessToken` both match.
const SECRET_FIELDS: [&str; 4] = ["accesstoken", "refreshtoken", "idtoken", "password"];
/// Query parameters whose values are never logged or recorded.
const SECRET_PARAMS: [&str; 12] = [
    "accesstoken",
    "refreshtoken",
    "idtoken",
    "password",
    "token",
    "apikey",
    "secret",
    "clientsecret",
    "signature",
    "sig",
    "auth",
    "authorization",
];

fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_' && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn is_secret_field(name: &str) -> bool {
    SECRET_FIELDS.contains(&normalized(name).as_str())
}

fn is_secret_param(name: &str) -> bool {
    let name = percent_decode(name);
    SECRET_PARAMS.contains(&normalized(&name).as_str())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(byte) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `url` with credentials removed: userinfo dropped and the value of every
/// secret-named query parameter replaced by `mask`.
pub(crate) fn redact_url(url: &str, mask: &str) -> String {
    let (base, fragment) = match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url, None),
    };
    let (path, query) = match base.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (base, None),
    };
    let path = strip_userinfo(path);
    let mut out = path;
    if let Some(query) = query {
        let pairs: Vec<String> = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((name, _)) if is_secret_param(name) => format!("{name}={mask}"),
                _ => pair.to_string(),
            })
            .collect();
        out.push('?');
        out.push_str(&pairs.join("&"));
    }
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

fn strip_userinfo(path: &str) -> String {
    let Some(scheme_end) = path.find("://") else {
        return path.to_string();
    };
    let authority_start = scheme_end + 3;
    let authority_end = path[authority_start..]
        .find('/')
        .map_or(path.len(), |end| authority_start + end);
    match path[authority_start..authority_end].rfind('@') {
        Some(at) => format!(
            "{}{}",
            &path[..authority_start],
            &path[authority_start + at + 1..]
        ),
        None => path.to_string(),
    }
}

/// Replace the value of every secret-named field, at any depth.
pub(crate) fn redact_json(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (name, field) in fields.iter_mut() {
                if is_secret_field(name) && !field.is_null() {
                    *field = Value::String(REDACTED.into());
                } else {
                    redact_json(field);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_json),
        _ => {}
    }
}

/// How a route answered an observed call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CallRoute {
    pub pattern: String,
    /// `fulfill`, `abort`, `continue`, or `hang`.
    pub action: &'static str,
    /// The route came from a dev-session scenario.
    pub dev: bool,
}

/// One observed Logic `fetch` (or SSE connection attempt).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Call {
    pub id: u64,
    pub appid: String,
    /// `fetch` or `sse`.
    pub kind: &'static str,
    pub method: String,
    /// Credentials removed, see [`redact_url`].
    pub url: String,
    pub started_ms: u64,
    pub duration_ms: Option<u64>,
    pub status: Option<u16>,
    pub error: Option<String>,
    pub route: Option<CallRoute>,
    raw_url: String,
    /// Whether the active recording wants this call's response.
    record: bool,
}

impl Call {
    /// The report form: no bodies, no headers.
    pub(crate) fn to_json(&self) -> Value {
        let mut out = json!({
            "time": self.started_ms,
            "kind": self.kind,
            "method": self.method,
            "url": self.url,
            "status": self.status,
            "durationMs": self.duration_ms,
            "source": match &self.route {
                Some(route) if route.action != "continue" => "route",
                _ => "network",
            },
        });
        if let Some(error) = &self.error {
            out["error"] = json!(error);
        }
        if let Some(route) = &self.route {
            out["route"] = json!({ "pattern": route.pattern, "action": route.action });
        }
        out
    }
}

/// What a finished call reports back.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    pub status: Option<u16>,
    pub error: Option<String>,
    pub content_type: Option<String>,
    pub body: Option<RecordedBody>,
}

/// The bounded log of observed calls, newest last.
#[derive(Debug, Default)]
pub(crate) struct CallLog {
    next_id: u64,
    calls: VecDeque<Call>,
}

impl CallLog {
    pub(crate) const fn new() -> Self {
        Self {
            next_id: 0,
            calls: VecDeque::new(),
        }
    }

    pub(crate) fn begin(
        &mut self,
        appid: &str,
        kind: &'static str,
        method: &str,
        url: &str,
        record: bool,
    ) -> u64 {
        self.next_id += 1;
        if self.calls.len() >= MAX_CALLS {
            self.calls.pop_front();
        }
        self.calls.push_back(Call {
            id: self.next_id,
            appid: appid.to_string(),
            kind,
            method: method.to_ascii_uppercase(),
            url: redact_url(url, REDACTED),
            started_ms: now_ms(),
            duration_ms: None,
            status: None,
            error: None,
            route: None,
            raw_url: url.to_string(),
            record,
        });
        self.next_id
    }

    fn find(&mut self, id: u64) -> Option<&mut Call> {
        self.calls.iter_mut().rev().find(|call| call.id == id)
    }

    pub(crate) fn routed(&mut self, id: u64, decision: &Decision) {
        if let Some(call) = self.find(id) {
            let action = decision.action.kind();
            // A recording captures what the server said, not a route.
            if !matches!(decision.action, super::registry::RouteAction::Continue) {
                call.record = false;
            }
            call.route = Some(CallRoute {
                pattern: decision.pattern.clone(),
                action,
                dev: decision.owner == super::registry::DEV_SESSION_OWNER,
            });
        }
    }

    /// Finish a call; returns it when a recording should keep it.
    pub(crate) fn settle(&mut self, id: u64, settled: &Settled) -> Option<Call> {
        let call = self.find(id)?;
        if call.duration_ms.is_some() {
            return None;
        }
        call.duration_ms = Some(now_ms().saturating_sub(call.started_ms));
        call.status = settled.status;
        call.error = settled
            .error
            .as_ref()
            .map(|error| error.chars().take(MAX_ERROR_CHARS).collect());
        call.record.then(|| call.clone())
    }

    /// The last `limit` calls that started at or after `since_ms`, oldest
    /// first. `mask` hides values that must not reach a report.
    pub(crate) fn recent(&self, since_ms: u64, limit: usize, mask: &[String]) -> Vec<Value> {
        let mut out: Vec<Value> = self
            .calls
            .iter()
            .rev()
            .filter(|call| call.started_ms >= since_ms)
            .take(limit)
            .map(|call| {
                let mut value = call.to_json();
                mask_strings(&mut value, mask);
                value
            })
            .collect();
        out.reverse();
        out
    }
}

/// Replace every occurrence of a secret value in the strings of `value`.
pub(crate) fn mask_strings(value: &mut Value, secrets: &[String]) {
    if secrets.is_empty() {
        return;
    }
    match value {
        Value::String(text) => {
            for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
                if text.contains(secret.as_str()) {
                    *text = text.replace(secret.as_str(), REDACTED);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| mask_strings(item, secrets)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|field| mask_strings(field, secrets)),
        _ => {}
    }
}

/// A recorded response body.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RecordedBody {
    Text(String),
    Binary(Vec<u8>),
    /// Not recorded; the note says why.
    Omitted(String),
}

impl RecordedBody {
    fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
            Self::Omitted(_) => 0,
        }
    }
}

/// One real request and what the network answered.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Exchange {
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<RecordedBody>,
    pub error: Option<String>,
}

/// Real Logic `fetch` traffic being captured into a scenario.
#[derive(Debug)]
pub(crate) struct Recording {
    /// The run that started it, or the dev-session owner.
    pub owner: String,
    pub appid: Option<String>,
    pub matcher: Option<UrlMatcher>,
    pub started_ms: u64,
    pub exchanges: Vec<Exchange>,
    /// Exchanges past [`MAX_EXCHANGES`] or the body budget.
    pub dropped: usize,
    body_bytes: usize,
}

impl Recording {
    pub(crate) fn new(owner: &str, appid: Option<String>, matcher: Option<UrlMatcher>) -> Self {
        Self {
            owner: owner.to_string(),
            appid,
            matcher,
            started_ms: now_ms(),
            exchanges: Vec::new(),
            dropped: 0,
            body_bytes: 0,
        }
    }

    pub(crate) fn wants(&self, appid: &str, url: &str) -> bool {
        self.appid
            .as_deref()
            .is_none_or(|expected| expected == appid)
            && self
                .matcher
                .as_ref()
                .is_none_or(|matcher| matcher.is_match(url))
    }

    pub(crate) fn push(&mut self, call: &Call, mut settled: Settled) {
        settled.body = settled.body.map(|body| match body {
            RecordedBody::Text(text) if text.len() > MAX_TEXT_BODY_BYTES => {
                RecordedBody::Omitted(format!("text body of {} bytes not recorded", text.len()))
            }
            RecordedBody::Binary(bytes) if bytes.len() > MAX_BINARY_BODY_BYTES => {
                RecordedBody::Omitted(format!("binary body of {} bytes not recorded", bytes.len()))
            }
            body => body,
        });
        let body_len = settled.body.as_ref().map_or(0, RecordedBody::len);
        if self.exchanges.len() >= MAX_EXCHANGES || self.body_bytes + body_len > MAX_RECORDED_BYTES
        {
            self.dropped += 1;
            return;
        }
        self.body_bytes += body_len;
        self.exchanges.push(Exchange {
            method: call.method.clone(),
            url: call.raw_url.clone(),
            status: settled.status,
            content_type: settled.content_type,
            body: settled.body,
            error: settled.error,
        });
    }

    /// The scenario file this recording amounts to: one route per method
    /// and URL, in first-seen order, answering what the network answered —
    /// a `sequence` when the answers differed. Credentials are redacted.
    pub(crate) fn to_scenario(&self, name: &str) -> Value {
        let mut routes: Vec<(String, String, Vec<Value>, Vec<String>)> = Vec::new();
        for exchange in &self.exchanges {
            let url = url_pattern(&exchange.url);
            let (answer, note) = answer_for(exchange);
            match routes
                .iter_mut()
                .find(|(method, pattern, _, _)| *method == exchange.method && *pattern == url)
            {
                Some((_, _, answers, notes)) => {
                    answers.push(answer);
                    if let Some(note) = note
                        && !notes.contains(&note)
                    {
                        notes.push(note);
                    }
                }
                None => routes.push((
                    exchange.method.clone(),
                    url,
                    vec![answer],
                    note.into_iter().collect(),
                )),
            }
        }
        let routes: Vec<Value> = routes
            .into_iter()
            .map(|(method, url, mut answers, mut notes)| {
                let mut route = Map::new();
                route.insert("url".into(), json!(url));
                route.insert("method".into(), json!(method));
                // Replay must answer the n-th call as recorded, so only a
                // run of identical answers collapses into one.
                if answers.windows(2).all(|pair| pair[0] == pair[1]) {
                    answers.truncate(1);
                } else if answers.len() > super::scenario::MAX_SEQUENCE {
                    notes.push(format!(
                        "{} answers recorded; the first {} are kept",
                        answers.len(),
                        super::scenario::MAX_SEQUENCE
                    ));
                    answers.truncate(super::scenario::MAX_SEQUENCE);
                }
                if !notes.is_empty() {
                    route.insert("note".into(), json!(notes.join("; ")));
                }
                if answers.len() == 1 {
                    if let Some(Value::Object(fields)) = answers.pop() {
                        route.extend(fields);
                    }
                } else {
                    route.insert("sequence".into(), Value::Array(answers));
                }
                Value::Object(route)
            })
            .collect();
        let mut description = format!(
            "Recorded from {} real request{}",
            self.exchanges.len(),
            if self.exchanges.len() == 1 { "" } else { "s" }
        );
        if self.dropped > 0 {
            description.push_str(&format!(
                "; {} more were not kept (recording limit)",
                self.dropped
            ));
        }
        json!({
            "name": name,
            "description": description,
            "routes": routes,
        })
    }
}

/// A route URL for exactly `url`, secrets wildcarded: a glob when `url` has
/// no glob syntax of its own, otherwise an anchored regex string.
pub(crate) fn url_pattern(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url);
    if !url.contains(['*', '{', '}']) {
        return redact_url(url, "*");
    }
    let placeholder = "\u{0}";
    let masked = redact_url(url, placeholder);
    let escaped = regex::escape(&masked).replace(placeholder, "[^&#]*");
    format!("/^{escaped}$/")
}

fn is_json_type(content_type: &str) -> bool {
    let essence = content_type.split(';').next().unwrap_or("").trim();
    essence == "application/json" || essence.ends_with("+json")
}

/// The scenario answer for one exchange, plus a note for what was left out.
fn answer_for(exchange: &Exchange) -> (Value, Option<String>) {
    let Some(status) = exchange.status else {
        return (json!({ "abort": "failed" }), None);
    };
    let mut answer = Map::new();
    answer.insert("status".into(), json!(status));
    let content_type = exchange.content_type.clone().filter(|t| !t.is_empty());
    let mut note = None;
    match &exchange.body {
        Some(RecordedBody::Text(text)) if text.is_empty() => {}
        Some(RecordedBody::Text(text))
            if content_type.as_deref().is_some_and(is_json_type)
                && serde_json::from_str::<Value>(text).is_ok() =>
        {
            let mut value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
            redact_json(&mut value);
            if content_type.as_deref() != Some("application/json") {
                answer.insert("contentType".into(), json!(content_type));
            }
            answer.insert("json".into(), value);
        }
        Some(RecordedBody::Text(text)) => {
            if let Some(content_type) = &content_type {
                answer.insert("contentType".into(), json!(content_type));
            }
            answer.insert("body".into(), json!(text));
        }
        Some(RecordedBody::Binary(bytes)) => {
            if let Some(content_type) = &content_type {
                answer.insert("contentType".into(), json!(content_type));
            }
            answer.insert("bodyBase64".into(), json!(BASE64.encode(bytes)));
        }
        Some(RecordedBody::Omitted(why)) => {
            if let Some(content_type) = &content_type {
                answer.insert("contentType".into(), json!(content_type));
            }
            note = Some(why.clone());
        }
        None => {
            if let Some(content_type) = &content_type {
                answer.insert("contentType".into(), json!(content_type));
            }
        }
    }
    if matches!(status, 204 | 205 | 304) {
        answer.remove("json");
        answer.remove("body");
        answer.remove("bodyBase64");
    }
    (Value::Object(answer), note)
}
