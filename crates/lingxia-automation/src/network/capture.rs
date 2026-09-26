//! Watching Logic `fetch` without changing it. One observation feeds three
//! consumers:
//!
//! - the bounded call log that failed specs carry into their report;
//! - a recording of real traffic into a scenario file (`--record-network`,
//!   `lxdev network record`);
//! - the response capture a contract check reads (`captureResponses()`,
//!   `lxdev test --openapi`).
//!
//! The `fetch` wrapper opens a call with `observe`, which says whether a
//! consumer wants the response body; it then reads that body once, hands the
//! app an equivalent buffered `Response`, and closes the call with `settle`.
//! Each consumer applies its own bounds and redaction here. Pure Rust, so it
//! is unit-testable without a JS engine.

use super::registry::{Decision, UrlMatcher, now_ms};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Value, json};
use std::collections::VecDeque;
use std::sync::LazyLock;

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
/// Captured body bytes per response unless the run asks for another limit.
pub(crate) const DEFAULT_CAPTURE_BODY_BYTES: usize = 256 * 1024;
/// Largest per-response capture limit a run may ask for.
pub(crate) const MAX_CAPTURE_BODY_BYTES: usize = 1024 * 1024;
/// Captured responses kept per process; the oldest are dropped first.
const MAX_CAPTURED: usize = 500;
/// Captured body bytes kept per process across all entries.
const MAX_CAPTURED_BYTES: usize = 8 * 1024 * 1024;

/// Names whose values are never recorded or logged, as JSON fields and as
/// query parameters. Compared case-insensitively with `_` and `-` removed,
/// so `access_token`, `accessToken` and `Access-Token` all match.
const SECRET_NAMES: [&str; 14] = [
    "token",
    "accesstoken",
    "refreshtoken",
    "idtoken",
    "apikey",
    "secret",
    "clientsecret",
    "password",
    "authorization",
    "session",
    "sessionid",
    "cookie",
    "setcookie",
    "xapikey",
];
/// Query parameters that are secret besides [`SECRET_NAMES`].
const SECRET_PARAMS: [&str; 3] = ["signature", "sig", "auth"];

fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_' && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn is_secret_field(name: &str) -> bool {
    SECRET_NAMES.contains(&normalized(name).as_str())
}

fn is_secret_param(name: &str) -> bool {
    let name = normalized(&percent_decode(name));
    SECRET_NAMES.contains(&name.as_str()) || SECRET_PARAMS.contains(&name.as_str())
}

fn percent_decode(text: &str) -> String {
    lingxia_control_protocol::text::percent_decode(text, true)
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

/// `scheme://host[:port]/path` of `url`: userinfo, query and fragment can
/// carry credentials, and a contract check only needs the path.
pub(crate) fn contract_url(url: &str) -> String {
    let cut = url.find(['?', '#']).unwrap_or(url.len());
    strip_userinfo(&url[..cut])
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

/// Replace the value of every secret-named field, at any depth, and mask
/// credential-shaped text (see [`redact_text`]) in every string.
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
        Value::String(text) => {
            if let std::borrow::Cow::Owned(masked) = redact_text(text) {
                *text = masked;
            }
        }
        _ => {}
    }
}

/// A JSON Web Token: three base64url segments, the first a JSON header.
static JWT: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*").expect("JWT pattern")
});
/// An HTTP bearer credential.
static BEARER: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]+").expect("bearer pattern")
});

/// `text` with credential-shaped values masked: JSON Web Tokens and
/// `Bearer <token>` credentials. Borrowed when nothing matched.
pub(crate) fn redact_text(text: &str) -> std::borrow::Cow<'_, str> {
    let masked = JWT.replace_all(text, REDACTED);
    if !BEARER.is_match(&masked) {
        return masked;
    }
    std::borrow::Cow::Owned(
        BEARER
            .replace_all(&masked, format!("${{1}} {REDACTED}").as_str())
            .into_owned(),
    )
}

/// How a route answered an observed call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CallRoute {
    pub pattern: String,
    /// `fulfill`, `abort`, `continue`, or `hang`.
    pub action: &'static str,
    /// A `continue` that merge-patches the real response.
    pub patch: bool,
    /// The route came from a dev-session scenario.
    pub dev: bool,
    /// The scenario rule that answered: `(rule index, "name:variant")`.
    pub rule: Option<(usize, String)>,
}

/// Which consumers want an observed call's response body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Watch {
    /// The active recording keeps the response, whatever its type.
    pub record: bool,
    /// A contract capture keeps the response, and its body when JSON.
    pub contract: bool,
}

impl Watch {
    pub(crate) fn wants_body(self) -> bool {
        self.record || self.contract
    }
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
    /// Why no scenario rule answered, when its targets matched.
    pub no_match: Option<String>,
    pub(crate) raw_url: String,
    /// Who wants this call's response.
    pub(crate) watch: Watch,
    /// Owner of the recording that keeps this call's response.
    pub(crate) recorder: Option<String>,
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
            if let Some((index, scenario)) = &route.rule {
                out["route"]["rule"] = json!(index);
                out["route"]["scenario"] = json!(scenario);
            }
        }
        out["answeredBy"] = json!(self.answered_by());
        if let Some(no_match) = &self.no_match {
            out["noMatch"] = json!(no_match);
        }
        out
    }

    /// `rule 2 (wifi:b)`, `route **/x`, or `real`.
    pub(crate) fn answered_by(&self) -> String {
        match &self.route {
            Some(route) if route.action == "continue" && !route.patch => "real".to_string(),
            Some(CallRoute {
                rule: Some((index, scenario)),
                ..
            }) => format!("rule {index} ({scenario})"),
            Some(route) => format!("route {}", route.pattern),
            None => "real".to_string(),
        }
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
        watch: Watch,
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
            no_match: None,
            raw_url: url.to_string(),
            watch,
            recorder: None,
        });
        self.next_id
    }

    pub(crate) fn find(&mut self, id: u64) -> Option<&mut Call> {
        self.calls.iter_mut().rev().find(|call| call.id == id)
    }

    /// Note the route that answered call `id`. Returns the call when a
    /// contract capture wants a response the route fulfilled: the route's
    /// answer is known here, so the wrapper does not report its body.
    pub(crate) fn routed(&mut self, id: u64, decision: &Decision) -> Option<Call> {
        use super::registry::RouteAction;
        let call = self.find(id)?;
        let action = decision.action.kind();
        // A recording captures what the server said, not a route.
        if !matches!(decision.action, RouteAction::Continue) {
            call.watch.record = false;
        }
        call.route = Some(CallRoute {
            pattern: decision.pattern.clone(),
            action,
            patch: matches!(decision.action, RouteAction::Patch(_)),
            dev: decision.owner == super::registry::DEV_SESSION_OWNER,
            rule: decision.rule.clone(),
        });
        let fulfilled = matches!(decision.action, RouteAction::Fulfill(_));
        let contract = call.watch.contract && fulfilled;
        // A pass-through or patched response is captured when it settles;
        // an abort, a hang or an event stream has no body to check.
        if !matches!(
            decision.action,
            RouteAction::Continue | RouteAction::Patch(_)
        ) {
            call.watch.contract = false;
        }
        contract.then(|| call.clone())
    }

    /// Finish a call; returns it when a recording or a contract capture
    /// wants its response.
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
        call.watch.wants_body().then(|| call.clone())
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

    /// The scenario file this recording amounts to: one `http` rule per
    /// method and URL, in first-seen order, answering what the network answered —
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
                route.insert("http".into(), json!(format!("{method} {url}")));
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
            "rules": routes,
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

/// `application/json`, `text/json` and `application/<x>+json`, parameters
/// ignored. Streaming JSON types (`application/x-ndjson`, `json-seq`) are
/// not: a contract capture must not buffer a stream the app reads
/// incrementally. The `fetch` wrapper's `JSON_TYPE` must agree.
pub(crate) fn is_json_type(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let Some((kind, subtype)) = essence.split_once('/') else {
        return false;
    };
    (kind == "application" || kind == "text")
        && (subtype == "json"
            || subtype
                .rsplit_once('+')
                .is_some_and(|(_, suffix)| suffix == "json"))
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
            answer.insert("body".into(), json!(redact_text(text)));
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

/// Who produced a captured response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// A route fulfilled it.
    Route,
    /// The real server answered and a route merge-patched the body.
    Patch,
    /// The real server answered (no route, or a plain `continue`).
    Network,
}

impl Source {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Patch => "patch",
            Self::Network => "network",
        }
    }
}

/// One captured response, as `NetworkDriver.responses()` reports it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResponseEntry {
    /// Process-wide, increasing; `responses({ since })` reads past it.
    pub seq: u64,
    pub method: String,
    /// `scheme://host[:port]/path`, see [`contract_url`].
    pub url: String,
    pub source: Source,
    /// The route's pattern for `route` and `patch` responses.
    pub pattern: Option<String>,
    pub status: u16,
    pub content_type: Option<String>,
    /// JSON body text, cut to the run's limit; `None` for other types.
    pub body: Option<String>,
    pub body_truncated: bool,
    pub timestamp_ms: u64,
    run_id: String,
    appid: String,
}

/// One response the app received.
pub(crate) struct Observed<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub source: Source,
    pub pattern: Option<String>,
    pub status: u16,
    pub content_type: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl<'a> Observed<'a> {
    /// A settled call's response, `None` when it never got one.
    pub(crate) fn settled(call: &'a Call, settled: &'a Settled) -> Option<Self> {
        let (source, pattern) = match &call.route {
            Some(route) if route.patch => (Source::Patch, Some(route.pattern.clone())),
            _ => (Source::Network, None),
        };
        Some(Self {
            method: &call.method,
            url: &call.raw_url,
            source,
            pattern,
            status: settled.status?,
            content_type: settled.content_type.as_deref(),
            body: match &settled.body {
                Some(RecordedBody::Text(text)) => Some(text),
                _ => None,
            },
        })
    }
}

#[derive(Debug)]
struct Capture {
    run_id: String,
    appid: String,
    body_limit: usize,
}

/// Contract capture: which apps a run captures, and what they received.
/// Nothing that can carry a credential is kept: no headers, and the URL
/// without userinfo, query or fragment.
#[derive(Debug, Default)]
pub(crate) struct Captures {
    next_seq: u64,
    captures: Vec<Capture>,
    log: VecDeque<ResponseEntry>,
    bytes: usize,
}

impl Captures {
    pub(crate) const fn new() -> Self {
        Self {
            next_seq: 0,
            captures: Vec::new(),
            log: VecDeque::new(),
            bytes: 0,
        }
    }

    /// Start (or re-limit) capturing `appid`'s responses for `run_id`.
    /// `run_active` is checked under the lock, like a route install.
    pub(crate) fn enable(
        &mut self,
        run_id: &str,
        appid: &str,
        body_limit: usize,
        run_active: impl FnOnce() -> bool,
    ) -> Result<(), String> {
        if !run_active() {
            return Err("the automation run that owns this driver has ended".into());
        }
        if body_limit == 0 || body_limit > MAX_CAPTURE_BODY_BYTES {
            return Err(format!(
                "captureResponses maxBodyBytes must be in 1..={MAX_CAPTURE_BODY_BYTES}, got {body_limit}"
            ));
        }
        match self
            .captures
            .iter_mut()
            .find(|capture| capture.run_id == run_id && capture.appid == appid)
        {
            Some(capture) => capture.body_limit = body_limit,
            None => self.captures.push(Capture {
                run_id: run_id.to_string(),
                appid: appid.to_string(),
                body_limit,
            }),
        }
        Ok(())
    }

    /// Apps captured now, across runs.
    pub(crate) fn len(&self) -> usize {
        self.captures.len()
    }

    /// Whether `appid`'s responses are captured now.
    pub(crate) fn capturing(&self, appid: &str) -> bool {
        self.captures.iter().any(|capture| capture.appid == appid)
    }

    /// Record a response of `appid`; `false` when nobody captures it.
    pub(crate) fn record(&mut self, appid: &str, observed: Observed<'_>) -> bool {
        let Some(capture) = self
            .captures
            .iter()
            .rev()
            .find(|capture| capture.appid == appid)
        else {
            return false;
        };
        let json = observed.content_type.is_some_and(is_json_type);
        let (body, body_truncated) = match observed.body.filter(|_| json) {
            None => (None, false),
            Some(text) if text.len() <= capture.body_limit => (Some(text.to_string()), false),
            Some(text) => {
                let mut end = capture.body_limit;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                (Some(text[..end].to_string()), true)
            }
        };
        self.next_seq += 1;
        let entry = ResponseEntry {
            seq: self.next_seq,
            method: observed.method.to_ascii_uppercase(),
            url: contract_url(observed.url),
            source: observed.source,
            pattern: observed.pattern,
            status: observed.status,
            content_type: observed.content_type.map(str::to_string),
            body,
            body_truncated,
            timestamp_ms: now_ms(),
            run_id: capture.run_id.clone(),
            appid: appid.to_string(),
        };
        let len = entry.body.as_ref().map_or(0, String::len);
        while self.log.len() >= MAX_CAPTURED
            || (!self.log.is_empty() && self.bytes + len > MAX_CAPTURED_BYTES)
        {
            if let Some(old) = self.log.pop_front() {
                self.bytes -= old.body.as_ref().map_or(0, String::len);
            }
        }
        self.bytes += len;
        self.log.push_back(entry);
        true
    }

    /// `appid`'s responses in `run_id` recorded after `since`, oldest first.
    pub(crate) fn responses(&self, run_id: &str, appid: &str, since: u64) -> Vec<ResponseEntry> {
        self.log
            .iter()
            .filter(|entry| entry.run_id == run_id && entry.appid == appid && entry.seq > since)
            .cloned()
            .collect()
    }

    /// Stop capturing for a run and drop what it recorded.
    pub(crate) fn clear_run(&mut self, run_id: &str) {
        self.captures.retain(|capture| capture.run_id != run_id);
        self.log.retain(|entry| entry.run_id != run_id);
        self.bytes = self
            .log
            .iter()
            .map(|entry| entry.body.as_ref().map_or(0, String::len))
            .sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed<'a>(
        url: &'a str,
        content_type: Option<&'a str>,
        body: Option<&'a str>,
    ) -> Observed<'a> {
        Observed {
            method: "get",
            url,
            source: Source::Network,
            pattern: None,
            status: 200,
            content_type,
            body,
        }
    }

    #[test]
    fn json_types_exclude_streams() {
        for yes in [
            "application/json",
            "application/json; charset=utf-8",
            "application/problem+json",
            "TEXT/JSON",
            "application/vnd.api+json",
        ] {
            assert!(is_json_type(yes), "{yes}");
        }
        for no in [
            "application/x-ndjson",
            "application/json-seq",
            "text/event-stream",
            "text/plain",
            "json",
            "",
        ] {
            assert!(!is_json_type(no), "{no}");
        }
    }

    #[test]
    fn contract_urls_lose_credentials_query_and_fragment() {
        assert_eq!(
            contract_url("https://user:pw@api.test:8443/v1/devices?token=abc#x"),
            "https://api.test:8443/v1/devices"
        );
        assert_eq!(contract_url("https://api.test"), "https://api.test");
        assert_eq!(contract_url("/relative?q=1"), "/relative");
    }

    #[test]
    fn captures_only_captured_apps_and_bounds_bodies() {
        let mut captures = Captures::default();
        assert!(!captures.record(
            "app",
            observed("https://a.test/x", Some("application/json"), Some("{}"))
        ));
        assert!(captures.enable("run", "app", 0, || true).is_err());
        assert!(captures.enable("run", "app", 7, || false).is_err());
        // Seven bytes end inside the two-byte `é`.
        captures.enable("run", "app", 7, || true).unwrap();
        assert!(captures.capturing("app"));
        assert!(!captures.capturing("other"));

        captures.record(
            "app",
            observed(
                "https://a.test/x?k=1",
                Some("application/json"),
                Some("{\"a\":\"é\"}"),
            ),
        );
        captures.record(
            "app",
            observed("https://a.test/y", Some("text/html"), Some("<p>")),
        );
        let log = captures.responses("run", "app", 0);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].method, "GET");
        assert_eq!(log[0].url, "https://a.test/x");
        // Cut on a character boundary, flagged.
        assert_eq!(log[0].body.as_deref(), Some("{\"a\":\""));
        assert!(log[0].body_truncated);
        // A non-JSON body is never kept.
        assert_eq!(log[1].body, None);
        assert!(!log[1].body_truncated);
        assert_eq!(captures.responses("run", "app", log[0].seq).len(), 1);

        captures.clear_run("run");
        assert!(!captures.capturing("app"));
        assert!(captures.responses("run", "app", 0).is_empty());
    }

    #[test]
    fn the_capture_log_is_bounded() {
        let mut captures = Captures::default();
        captures
            .enable("run", "app", MAX_CAPTURE_BODY_BYTES, || true)
            .unwrap();
        let body = "x".repeat(MAX_CAPTURE_BODY_BYTES);
        for _ in 0..(MAX_CAPTURED_BYTES / MAX_CAPTURE_BODY_BYTES + 3) {
            captures.record(
                "app",
                observed("https://a.test/big", Some("application/json"), Some(&body)),
            );
        }
        assert!(captures.bytes <= MAX_CAPTURED_BYTES);
        for _ in 0..(MAX_CAPTURED + 10) {
            captures.record(
                "app",
                observed("https://a.test/small", Some("application/json"), Some("1")),
            );
        }
        assert_eq!(captures.log.len(), MAX_CAPTURED);
        captures.clear_run("run");
    }
}
