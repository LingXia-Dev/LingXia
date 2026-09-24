//! Route table for test network interception: matching, fulfillment specs,
//! and the matched-request log. Pure Rust so it is unit-testable without a
//! JS engine; `super` adapts it to the automation driver and Logic `fetch`.

use super::capture::{CallLog, Captures, Observed, Recording, Settled, Source, Watch};
use regex::Regex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Matched-request entries kept per process; the oldest are dropped first.
const MAX_LOG_ENTRIES: usize = 1_000;
/// Fulfillment bodies are test fixtures, not payload transfer.
pub(crate) const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// Request bodies are recorded as text up to this many bytes.
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
/// Recorded request bodies kept per process across all entries.
const MAX_LOG_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Longest fulfillment delay a route may ask for.
pub(crate) const MAX_DELAY_MS: u32 = 30_000;
/// Owner of the routes a dev session installed with `lxdev network scenario
/// use`. Automation run ids are UUIDs, so it never collides with one.
pub(crate) const DEV_SESSION_OWNER: &str = "@dev-session";

#[derive(Debug, Clone)]
pub(crate) enum UrlMatcher {
    Glob {
        pattern: String,
        regex: Regex,
    },
    Regex {
        source: String,
        flags: String,
        regex: Regex,
    },
}

impl UrlMatcher {
    /// Playwright-style URL glob over the whole URL: `**` matches any run of
    /// characters, `*` any run without `/`, `{a,b}` alternatives. Everything
    /// else, `?` included, is literal so query strings can be written as-is.
    pub(crate) fn glob(pattern: &str) -> Result<Self, String> {
        if pattern.is_empty() {
            return Err("route URL pattern must not be empty".into());
        }
        let mut out = String::from("^");
        let mut chars = pattern.chars().peekable();
        let mut in_group = false;
        while let Some(c) = chars.next() {
            match c {
                '*' if chars.peek() == Some(&'*') => {
                    chars.next();
                    out.push_str(".*");
                }
                '*' => out.push_str("[^/]*"),
                '{' if !in_group => {
                    in_group = true;
                    out.push_str("(?:");
                }
                '}' if in_group => {
                    in_group = false;
                    out.push(')');
                }
                ',' if in_group => out.push('|'),
                other => out.push_str(&regex::escape(other.encode_utf8(&mut [0; 4]))),
            }
        }
        if in_group {
            return Err(format!(
                "route URL pattern '{pattern}' has an unclosed '{{'"
            ));
        }
        out.push('$');
        let regex = Regex::new(&out).map_err(|err| format!("invalid URL glob: {err}"))?;
        Ok(Self::Glob {
            pattern: pattern.to_string(),
            regex,
        })
    }

    /// A JS `RegExp` (`source` + `flags`), searched like `RegExp.test`.
    /// Rust `regex` syntax applies: no lookaround or backreferences.
    pub(crate) fn regex(source: &str, flags: &str) -> Result<Self, String> {
        let mut inline = String::new();
        for flag in flags.chars() {
            match flag {
                'i' | 's' | 'm' => inline.push(flag),
                // Stateful or encoding flags do not change a single test.
                'g' | 'y' | 'u' | 'd' | 'v' => {}
                other => return Err(format!("unsupported RegExp flag '{other}'")),
            }
        }
        let pattern = if inline.is_empty() {
            source.to_string()
        } else {
            format!("(?{inline}){source}")
        };
        let regex = Regex::new(&pattern)
            .map_err(|err| format!("route RegExp /{source}/{flags} is not supported: {err}"))?;
        Ok(Self::Regex {
            source: source.to_string(),
            flags: flags.to_string(),
            regex,
        })
    }

    pub(crate) fn is_match(&self, url: &str) -> bool {
        match self {
            Self::Glob { regex, .. } | Self::Regex { regex, .. } => regex.is_match(url),
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Glob { pattern, .. } => pattern.clone(),
            Self::Regex { source, flags, .. } => format!("/{source}/{flags}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResponseBody {
    Text(String),
    Binary(Vec<u8>),
}

impl ResponseBody {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Binary(bytes) => bytes.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fulfill {
    pub status: u16,
    pub status_text: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<ResponseBody>,
    /// Milliseconds before the response resolves; at most [`MAX_DELAY_MS`].
    pub delay_ms: u32,
}

/// One item of an SSE answer, in stream order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SseStep {
    Event {
        event: Option<String>,
        data: String,
        id: Option<String>,
        retry: Option<u64>,
    },
    Comment(String),
    /// Pause the stream this many milliseconds.
    Delay(u32),
    /// Close the stream here, as a server dropping the connection would.
    Drop,
}

impl SseStep {
    /// The `text/event-stream` wire form of an event or comment.
    pub(crate) fn frame(&self) -> Option<String> {
        match self {
            Self::Event {
                event,
                data,
                id,
                retry,
            } => {
                let mut out = String::new();
                if let Some(event) = event {
                    out.push_str(&format!("event: {event}\n"));
                }
                if let Some(id) = id {
                    out.push_str(&format!("id: {id}\n"));
                }
                if let Some(retry) = retry {
                    out.push_str(&format!("retry: {retry}\n"));
                }
                for line in data.split('\n') {
                    let line = line.strip_suffix('\r').unwrap_or(line);
                    out.push_str(&format!("data: {line}\n"));
                }
                out.push('\n');
                Some(out)
            }
            Self::Comment(text) => Some(
                text.split('\n')
                    .map(|line| format!(": {}\n", line.strip_suffix('\r').unwrap_or(line)))
                    .collect(),
            ),
            Self::Delay(_) | Self::Drop => None,
        }
    }
}

/// A `text/event-stream` answer. Without a trailing [`SseStep::Drop`] the
/// stream stays open after its last item, like a live server, until the
/// route is removed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SseAnswer {
    pub headers: Vec<(String, String)>,
    pub steps: Vec<SseStep>,
    /// Milliseconds before the response opens.
    pub delay_ms: u32,
    /// Held-request token while the stream stays open; 0 in a route spec.
    pub hold: u64,
}

impl SseAnswer {
    pub(crate) fn stays_open(&self) -> bool {
        !matches!(self.steps.last(), Some(SseStep::Drop))
    }
}

/// Transport failures a route can emulate. Only failures the `fetch` wrapper
/// reproduces faithfully belong here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbortKind {
    /// `TypeError: fetch failed`, as for an unreachable host.
    Failed,
}

impl AbortKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RouteAction {
    Fulfill(Fulfill),
    /// Reject like a transport failure.
    Abort(AbortKind),
    Continue,
    /// Let the request reach the network, then apply this RFC 7396 JSON
    /// merge patch to the real response body.
    Patch(serde_json::Value),
    /// Never answer until the route is removed (the spec ends) or the run
    /// ends. `token` is 0 in a route spec; a decision carries the held
    /// request's own token.
    Hang {
        token: u64,
    },
    /// Answer with a `text/event-stream` built from these items.
    Sse(SseAnswer),
}

impl RouteAction {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Fulfill(_) | Self::Sse(_) => "fulfill",
            Self::Abort(_) => "abort",
            Self::Continue | Self::Patch(_) => "continue",
            Self::Hang { .. } => "hang",
        }
    }
}

/// RFC 7396 JSON merge patch: an object patch merges key by key (`null`
/// removes a key); anything else replaces the target.
pub(crate) fn merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let serde_json::Value::Object(fields) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = serde_json::Value::Object(serde_json::Map::new());
    }
    let Some(object) = target.as_object_mut() else {
        return;
    };
    for (key, value) in fields {
        if value.is_null() {
            object.remove(key);
        } else {
            merge_patch(
                object.entry(key.clone()).or_insert(serde_json::Value::Null),
                value,
            );
        }
    }
}

/// A request a `hang` route is holding.
#[derive(Debug)]
struct Held {
    token: u64,
    run_id: String,
    appid: String,
    route_id: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RouteSpec {
    pub matcher: UrlMatcher,
    /// Upper-case method; `None` matches every method.
    pub method: Option<String>,
    /// Remaining matches before the route removes itself.
    pub times: Option<u32>,
    /// Answers in call order; the last one repeats. Never empty.
    pub answers: Vec<RouteAction>,
    /// Requests this route answered so far.
    pub served: usize,
}

impl RouteSpec {
    pub(crate) fn new(
        matcher: UrlMatcher,
        method: Option<String>,
        times: Option<u32>,
        answers: Vec<RouteAction>,
    ) -> Self {
        debug_assert!(!answers.is_empty());
        Self {
            matcher,
            method,
            times,
            answers,
            served: 0,
        }
    }

    /// The answer for the next request, advancing the sequence.
    fn next_answer(&mut self) -> RouteAction {
        let index = self.served.min(self.answers.len().saturating_sub(1));
        self.served += 1;
        self.answers[index].clone()
    }
}

/// The route that answered a request, and how.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Decision {
    pub action: RouteAction,
    pub owner: String,
    pub route_id: u64,
    pub pattern: String,
}

/// The routes a dev session installed from a scenario file.
#[derive(Debug, Clone)]
pub(crate) struct DevScenario {
    pub name: Option<String>,
    pub source: Option<String>,
    pub appid: String,
    pub route_ids: Vec<u64>,
    pub installed_ms: u64,
}

#[derive(Debug)]
struct Route {
    id: u64,
    run_id: String,
    appid: String,
    spec: RouteSpec,
}

/// What the app sent, as far as the `fetch` wrapper can read it synchronously.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SentRequest {
    /// Lower-case names, in the order `Headers` iterates them.
    pub headers: Vec<(String, String)>,
    /// Text body, cut to [`MAX_REQUEST_BODY_BYTES`] on a UTF-8 boundary.
    pub body: Option<String>,
    pub body_truncated: bool,
}

impl SentRequest {
    /// Record `body`, cutting it to the byte limit. `overflow` reports that
    /// the caller already dropped bytes past the limit.
    pub(crate) fn new(
        headers: Vec<(String, String)>,
        body: Option<String>,
        overflow: bool,
    ) -> Self {
        let mut truncated = overflow;
        let body = body.map(|mut text| {
            if text.len() > MAX_REQUEST_BODY_BYTES {
                let mut end = MAX_REQUEST_BODY_BYTES;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                truncated = true;
            }
            text
        });
        Self {
            headers,
            body,
            body_truncated: truncated,
        }
    }

    fn body_len(&self) -> usize {
        self.body.as_ref().map_or(0, String::len)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestEntry {
    pub route_id: u64,
    pub pattern: String,
    pub method: String,
    pub url: String,
    /// `fulfill`, `abort`, `continue`, or `hang`.
    pub action: &'static str,
    pub status: Option<u16>,
    pub request: SentRequest,
    pub timestamp_ms: u64,
    run_id: String,
    appid: String,
}

#[derive(Debug, Default)]
pub(crate) struct Registry {
    next_id: u64,
    routes: Vec<Route>,
    log: VecDeque<RequestEntry>,
    log_body_bytes: usize,
    held: Vec<Held>,
    /// Host automation runs in progress. While any exists, dev-session
    /// routes stand aside and Logic `fetch` calls are logged.
    active_runs: Vec<String>,
    pub(crate) dev: Option<DevScenario>,
    pub(crate) calls: CallLog,
    /// A recording a dev session started (`lxdev network record`). It
    /// pauses while a host automation run is active, like dev routes.
    pub(crate) dev_recording: Option<Recording>,
    /// A recording a host run started (`lxdev test --record-network`).
    pub(crate) run_recording: Option<Recording>,
    /// Contract captures of host runs (`captureResponses()`).
    pub(crate) captures: Captures,
}

impl Registry {
    /// Install a route for `appid` owned by `run_id`. `run_active` is checked
    /// under the table lock, so a run that finalizes concurrently either sees
    /// this route in its `clear_run` or rejects it here.
    pub(crate) fn install(
        &mut self,
        run_id: &str,
        appid: &str,
        spec: RouteSpec,
        run_active: impl FnOnce() -> bool,
    ) -> Result<u64, String> {
        if !run_active() {
            return Err("the automation run that owns this driver has ended".into());
        }
        self.next_id += 1;
        let id = self.next_id;
        self.routes.push(Route {
            id,
            run_id: run_id.to_string(),
            appid: appid.to_string(),
            spec,
        });
        Ok(id)
    }

    /// Install `specs` together, all or none. The first spec answers before
    /// later ones, so they are installed newest-last in reverse order.
    /// Returns `(id, pattern)` in `specs` order.
    pub(crate) fn install_all(
        &mut self,
        run_id: &str,
        appid: &str,
        specs: Vec<RouteSpec>,
        run_active: impl FnOnce() -> bool,
    ) -> Result<Vec<(u64, String)>, String> {
        if !run_active() {
            return Err("the automation run that owns this driver has ended".into());
        }
        let mut installed = Vec::with_capacity(specs.len());
        for spec in specs.into_iter().rev() {
            self.next_id += 1;
            installed.push((self.next_id, spec.matcher.label()));
            self.routes.push(Route {
                id: self.next_id,
                run_id: run_id.to_string(),
                appid: appid.to_string(),
                spec,
            });
        }
        installed.reverse();
        Ok(installed)
    }

    /// Open a call-log entry for a Logic request when a run or a recording
    /// watches it: `(id, who wants its response)`. Only a `fetch` response
    /// is captured for a contract check.
    pub(crate) fn observe(
        &mut self,
        appid: &str,
        kind: &'static str,
        method: &str,
        url: &str,
    ) -> Option<(u64, Watch)> {
        // A recording replays `fetch` answers; SSE connections are not
        // recorded.
        let recorder = (kind == "fetch")
            .then(|| self.active_recording())
            .flatten()
            .filter(|recording| recording.wants(appid, url))
            .map(|recording| recording.owner.clone());
        let watch = Watch {
            record: recorder.is_some(),
            contract: kind == "fetch" && self.captures.capturing(appid),
        };
        if !watch.wants_body() && self.active_runs.is_empty() {
            return None;
        }
        let id = self.calls.begin(appid, kind, method, url, watch);
        if let Some(call) = self.calls.find(id) {
            call.recorder = recorder;
        }
        Some((id, watch))
    }

    /// The recording that captures new calls: a run's while any host run is
    /// active (a dev recording pauses, so it never captures test traffic),
    /// otherwise the dev session's.
    fn active_recording(&self) -> Option<&Recording> {
        if self.runs_active() {
            self.run_recording.as_ref()
        } else {
            self.dev_recording.as_ref()
        }
    }

    /// Close a call-log entry and hand its response to whoever wants it:
    /// the recording, the contract capture. Settling twice is a no-op.
    pub(crate) fn settle(&mut self, id: u64, settled: Settled) {
        let Some(call) = self.calls.settle(id, &settled) else {
            return;
        };
        if call.watch.contract
            && let Some(observed) = Observed::settled(&call, &settled)
        {
            self.captures.record(&call.appid, observed);
        }
        // An app cancelling its own request is not something the server said.
        if !call.watch.record
            || settled
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with("AbortError"))
        {
            return;
        }
        // The recording that was active when the call started, if it still
        // runs: a call does not move into a recording started later.
        let owner = call.recorder.as_deref();
        if let Some(recording) = [self.run_recording.as_mut(), self.dev_recording.as_mut()]
            .into_iter()
            .flatten()
            .find(|recording| Some(recording.owner.as_str()) == owner)
        {
            recording.push(&call, settled);
        }
    }

    /// Remove a route and release the requests it holds — also after the
    /// route already expired through `times`.
    pub(crate) fn remove(&mut self, run_id: &str, id: u64) -> bool {
        self.held
            .retain(|held| !(held.run_id == run_id && held.route_id == id));
        let before = self.routes.len();
        self.routes
            .retain(|route| !(route.id == id && route.run_id == run_id));
        self.routes.len() != before
    }

    pub(crate) fn remove_app(&mut self, run_id: &str, appid: &str) -> usize {
        self.held
            .retain(|held| !(held.run_id == run_id && held.appid == appid));
        let before = self.routes.len();
        self.routes
            .retain(|route| !(route.run_id == run_id && route.appid == appid));
        before - self.routes.len()
    }

    /// A host automation run started: its Logic `fetch` calls are logged
    /// and dev-session routes stand aside until it ends.
    pub(crate) fn begin_run(&mut self, run_id: &str) {
        if !self.active_runs.iter().any(|run| run == run_id) {
            self.active_runs.push(run_id.to_string());
        }
    }

    pub(crate) fn runs_active(&self) -> bool {
        !self.active_runs.is_empty()
    }

    /// Drop every route and log entry a run owns, and end it.
    pub(crate) fn clear_run(&mut self, run_id: &str) {
        self.active_runs.retain(|run| run != run_id);
        // A dev recording outlives a scenario change; the session end stops it.
        if self
            .run_recording
            .as_ref()
            .is_some_and(|recording| recording.owner == run_id)
        {
            self.run_recording = None;
        }
        if run_id == DEV_SESSION_OWNER {
            self.dev = None;
        }
        self.held.retain(|held| held.run_id != run_id);
        self.routes.retain(|route| route.run_id != run_id);
        self.log.retain(|entry| entry.run_id != run_id);
        self.log_body_bytes = self.log.iter().map(|entry| entry.request.body_len()).sum();
        self.captures.clear_run(run_id);
    }

    /// [`Self::decide_route`] without call observation.
    #[cfg(test)]
    pub(crate) fn decide(
        &mut self,
        appid: &str,
        method: &str,
        url: &str,
        request: impl FnOnce() -> SentRequest,
        allowed: impl FnOnce() -> bool,
    ) -> Option<RouteAction> {
        self.decide_route(appid, method, url, request, allowed, 0)
            .map(|decision| decision.action)
    }

    /// The route answering a request, and how. The most recently installed
    /// matching route wins; a sequence advances by one answer. `allowed`
    /// reports whether the app's network policy admits the URL: a
    /// fulfillment never answers a request the real `fetch` would have
    /// refused. `call` is the [`CallLog`] id of the request, or 0 when it is
    /// not observed.
    pub(crate) fn decide_route(
        &mut self,
        appid: &str,
        method: &str,
        url: &str,
        request: impl FnOnce() -> SentRequest,
        allowed: impl FnOnce() -> bool,
        call: u64,
    ) -> Option<Decision> {
        // A test run is never steered by a scenario a developer left on.
        let dev_aside = !self.active_runs.is_empty();
        let index = self.routes.iter().rposition(|route| {
            route.appid == appid
                && !(dev_aside && route.run_id == DEV_SESSION_OWNER)
                && route
                    .spec
                    .method
                    .as_deref()
                    .is_none_or(|expected| expected.eq_ignore_ascii_case(method))
                && route.spec.matcher.is_match(url)
        })?;
        let route = &mut self.routes[index];
        let mut action = super::scenario::render_action(&route.spec.next_answer(), now_ms());
        // Neither an answer nor a stall may stand in for a host the real
        // `fetch` would refuse.
        if matches!(
            action,
            RouteAction::Fulfill(_) | RouteAction::Hang { .. } | RouteAction::Sse(_)
        ) && !allowed()
        {
            action = RouteAction::Continue;
        }
        let hold = match &mut action {
            RouteAction::Hang { token } => Some(token),
            RouteAction::Sse(sse) if sse.stays_open() => Some(&mut sse.hold),
            _ => None,
        };
        if let Some(token) = hold {
            self.next_id += 1;
            *token = self.next_id;
            self.held.push(Held {
                token: *token,
                run_id: route.run_id.clone(),
                appid: route.appid.clone(),
                route_id: route.id,
            });
        }
        let entry = RequestEntry {
            route_id: route.id,
            pattern: route.spec.matcher.label(),
            method: method.to_ascii_uppercase(),
            url: url.to_string(),
            action: action.kind(),
            status: match &action {
                RouteAction::Fulfill(fulfill) => Some(fulfill.status),
                RouteAction::Sse(_) => Some(200),
                _ => None,
            },
            request: request(),
            timestamp_ms: now_ms(),
            run_id: route.run_id.clone(),
            appid: route.appid.clone(),
        };
        let decision = Decision {
            action: action.clone(),
            owner: route.run_id.clone(),
            route_id: route.id,
            pattern: entry.pattern.clone(),
        };
        if let Some(times) = route.spec.times.as_mut() {
            *times = times.saturating_sub(1);
            if *times == 0 {
                self.routes.remove(index);
            }
        }
        if call != 0
            && let Some(observed) = self.calls.routed(call, &decision)
            && let RouteAction::Fulfill(fulfill) = &decision.action
        {
            // The route's answer is known here; the wrapper need not read it.
            let content_type = fulfill
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                .map(|(_, value)| value.as_str());
            let body = match &fulfill.body {
                Some(ResponseBody::Text(text)) => Some(text.as_str()),
                _ => None,
            };
            self.captures.record(
                &observed.appid,
                Observed {
                    method: &observed.method,
                    url: &observed.raw_url,
                    source: Source::Route,
                    pattern: Some(decision.pattern.clone()),
                    status: fulfill.status,
                    content_type,
                    body,
                },
            );
        }
        let body_len = entry.request.body_len();
        while self.log.len() >= MAX_LOG_ENTRIES
            || (!self.log.is_empty() && self.log_body_bytes + body_len > MAX_LOG_BODY_BYTES)
        {
            if let Some(old) = self.log.pop_front() {
                self.log_body_bytes -= old.request.body_len();
            }
        }
        self.log_body_bytes += body_len;
        self.log.push_back(entry);
        Some(decision)
    }

    /// Whether a `hang` route still holds the request behind `token`.
    pub(crate) fn holds(&self, token: u64) -> bool {
        self.held.iter().any(|held| held.token == token)
    }

    pub(crate) fn requests(&self, run_id: &str, appid: &str) -> Vec<RequestEntry> {
        self.log
            .iter()
            .filter(|entry| entry.run_id == run_id && entry.appid == appid)
            .cloned()
            .collect()
    }

    /// Requests the route `id` of `owner` answered, as far as the log holds.
    pub(crate) fn hits(&self, owner: &str, id: u64) -> usize {
        self.log
            .iter()
            .filter(|entry| entry.run_id == owner && entry.route_id == id)
            .count()
    }

    /// Route ids of `owner` still installed, with the answers left in
    /// their `times` budget.
    pub(crate) fn remaining(&self, owner: &str, id: u64) -> Option<Option<u32>> {
        self.routes
            .iter()
            .find(|route| route.run_id == owner && route.id == id)
            .map(|route| route.spec.times)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.routes.len()
    }

    /// What keeps the Logic `fetch` wrapper off its fast path.
    fn watched(&self) -> usize {
        self.routes.len()
            + self.active_runs.len()
            + usize::from(self.dev_recording.is_some())
            + usize::from(self.run_recording.is_some())
            + self.captures.len()
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

static ROUTES: Mutex<Registry> = Mutex::new(Registry {
    next_id: 0,
    routes: Vec::new(),
    log: VecDeque::new(),
    log_body_bytes: 0,
    held: Vec::new(),
    active_runs: Vec::new(),
    dev: None,
    calls: CallLog::new(),
    dev_recording: None,
    run_recording: None,
    captures: Captures::new(),
});
/// Installed routes plus active runs and recordings, mirrored outside the
/// lock so Logic `fetch` pays one atomic load when nothing watches it.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// Run `f` against the process route table, keeping [`ACTIVE`] in sync.
pub(crate) fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut guard: MutexGuard<'_, Registry> = ROUTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let result = f(&mut guard);
    ACTIVE.store(guard.watched(), Ordering::Release);
    result
}

pub(crate) fn any_active() -> bool {
    ACTIVE.load(Ordering::Acquire) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fulfill(status: u16) -> RouteAction {
        RouteAction::Fulfill(Fulfill {
            status,
            status_text: None,
            headers: Vec::new(),
            body: Some(ResponseBody::Text("{}".into())),
            delay_ms: 0,
        })
    }

    fn spec(matcher: UrlMatcher, method: Option<&str>, action: RouteAction) -> RouteSpec {
        RouteSpec::new(matcher, method.map(str::to_string), None, vec![action])
    }

    #[test]
    fn merge_patch_follows_rfc_7396() {
        use serde_json::json;
        // The RFC's own example (section 3).
        let mut target = json!({
            "title": "Goodbye!",
            "author": { "givenName": "John", "familyName": "Doe" },
            "tags": ["example", "sample"],
            "content": "This will be unchanged"
        });
        merge_patch(
            &mut target,
            &json!({
                "title": "Hello!",
                "phoneNumber": "+01-123-456-7890",
                "author": { "familyName": null },
                "tags": ["example"]
            }),
        );
        assert_eq!(
            target,
            json!({
                "title": "Hello!",
                "author": { "givenName": "John" },
                "tags": ["example"],
                "content": "This will be unchanged",
                "phoneNumber": "+01-123-456-7890"
            })
        );
        // Appendix A cases: a non-object patch replaces; an object patch
        // turns a non-object target into an object.
        let mut array = json!(["a", "b"]);
        merge_patch(&mut array, &json!(["c"]));
        assert_eq!(array, json!(["c"]));
        let mut scalar = json!("text");
        merge_patch(&mut scalar, &json!({ "a": { "bb": { "ccc": null } } }));
        assert_eq!(scalar, json!({ "a": { "bb": {} } }));
    }

    #[test]
    fn a_hang_is_held_until_its_route_is_removed_even_after_it_expired() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("https://api.test/**").unwrap();
        let mut once = spec(matcher, None, RouteAction::Hang { token: 0 });
        once.times = Some(1);
        let id = registry.install("run", "app", once, || true).unwrap();
        let Some(RouteAction::Hang { token }) = registry.decide(
            "app",
            "GET",
            "https://api.test/slow",
            SentRequest::default,
            || true,
        ) else {
            panic!("expected a hang");
        };
        assert_ne!(token, 0);
        assert_eq!(registry.len(), 0, "times: 1 expired the route");
        assert!(registry.holds(token), "expiry alone does not release");
        assert_eq!(registry.requests("run", "app")[0].action, "hang");
        assert!(!registry.remove("run", id));
        assert!(!registry.holds(token), "removing the route releases it");

        // A run ending releases what its routes held.
        let matcher = UrlMatcher::glob("https://api.test/**").unwrap();
        registry
            .install(
                "run",
                "app",
                spec(matcher, None, RouteAction::Hang { token: 0 }),
                || true,
            )
            .unwrap();
        let Some(RouteAction::Hang { token }) = registry.decide(
            "app",
            "GET",
            "https://api.test/x",
            SentRequest::default,
            || true,
        ) else {
            panic!("expected a hang");
        };
        registry.clear_run("run");
        assert!(!registry.holds(token));
    }

    #[test]
    fn a_hang_never_stalls_a_refused_host() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("**").unwrap();
        registry
            .install(
                "run",
                "app",
                spec(matcher, None, RouteAction::Hang { token: 0 }),
                || true,
            )
            .unwrap();
        assert_eq!(
            registry.decide(
                "app",
                "GET",
                "https://blocked.test/",
                SentRequest::default,
                || false
            ),
            Some(RouteAction::Continue)
        );
    }

    #[test]
    fn glob_semantics() {
        let glob = UrlMatcher::glob("**/v1/devices/*").unwrap();
        assert!(glob.is_match("https://api.example.com/v1/devices/abc"));
        assert!(!glob.is_match("https://api.example.com/v1/devices/abc/name"));
        assert!(glob.is_match("https://api.example.com/v1/devices/abc?x=1"));

        let deep = UrlMatcher::glob("https://api.example.com/**").unwrap();
        assert!(deep.is_match("https://api.example.com/a/b?c=d"));
        assert!(!deep.is_match("https://evil.example.com/a"));

        let literal = UrlMatcher::glob("**/search?q=a.b").unwrap();
        assert!(literal.is_match("http://x/search?q=a.b"));
        assert!(!literal.is_match("http://x/searchXq=aXb"));

        let alt = UrlMatcher::glob("**/*.{png,jpg}").unwrap();
        assert!(alt.is_match("http://x/a/b.png"));
        assert!(alt.is_match("http://x/b.jpg"));
        assert!(!alt.is_match("http://x/b.gif"));

        assert!(UrlMatcher::glob("").is_err());
        assert!(UrlMatcher::glob("**/{a,b").is_err());
    }

    #[test]
    fn regex_semantics_follow_regexp_test() {
        let re = UrlMatcher::regex(r"/v1/devices/\w+$", "").unwrap();
        assert!(re.is_match("https://h/v1/devices/abc"));
        assert!(!re.is_match("https://h/v1/devices/abc/x"));
        let ci = UrlMatcher::regex("DEVICES", "gi").unwrap();
        assert!(ci.is_match("https://h/v1/devices"));
        assert_eq!(ci.label(), "/DEVICES/gi");
        assert!(UrlMatcher::regex("(?=x)", "").is_err());
        assert!(UrlMatcher::regex("x", "z").is_err());
    }

    #[test]
    fn latest_matching_route_wins_and_method_filters() {
        let mut registry = Registry::default();
        let any = UrlMatcher::glob("**/devices/*").unwrap();
        registry
            .install("run", "app", spec(any.clone(), None, fulfill(200)), || true)
            .unwrap();
        registry
            .install("run", "app", spec(any, Some("PATCH"), fulfill(501)), || {
                true
            })
            .unwrap();

        assert_eq!(
            registry.decide(
                "app",
                "patch",
                "https://h/devices/1",
                SentRequest::default,
                || true
            ),
            Some(fulfill(501))
        );
        assert_eq!(
            registry.decide(
                "app",
                "GET",
                "https://h/devices/1",
                SentRequest::default,
                || true
            ),
            Some(fulfill(200))
        );
        assert_eq!(
            registry.decide(
                "other",
                "GET",
                "https://h/devices/1",
                SentRequest::default,
                || true
            ),
            None
        );
        assert_eq!(
            registry.decide(
                "app",
                "GET",
                "https://h/users/1",
                SentRequest::default,
                || true
            ),
            None
        );
    }

    #[test]
    fn times_expire_and_log_records_actions() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("**/x").unwrap();
        let mut once = spec(matcher.clone(), None, RouteAction::Abort(AbortKind::Failed));
        once.times = Some(1);
        registry
            .install(
                "run",
                "app",
                spec(matcher, None, RouteAction::Continue),
                || true,
            )
            .unwrap();
        let aborting = registry.install("run", "app", once, || true).unwrap();

        assert_eq!(
            registry.decide("app", "GET", "http://h/x", SentRequest::default, || true),
            Some(RouteAction::Abort(AbortKind::Failed))
        );
        // The one-shot route is gone; the older continue route answers now.
        assert_eq!(
            registry.decide("app", "POST", "http://h/x", SentRequest::default, || true),
            Some(RouteAction::Continue)
        );
        assert!(!registry.remove("run", aborting));

        let log = registry.requests("run", "app");
        assert_eq!(log.len(), 2);
        assert_eq!((log[0].action, log[0].method.as_str()), ("abort", "GET"));
        assert_eq!(
            (log[1].action, log[1].method.as_str()),
            ("continue", "POST")
        );
        assert!(registry.requests("other-run", "app").is_empty());
    }

    #[test]
    fn disallowed_domains_are_not_fulfilled() {
        let mut registry = Registry::default();
        registry
            .install(
                "run",
                "app",
                spec(UrlMatcher::glob("**").unwrap(), None, fulfill(200)),
                || true,
            )
            .unwrap();
        assert_eq!(
            registry.decide(
                "app",
                "GET",
                "https://blocked/x",
                SentRequest::default,
                || false
            ),
            Some(RouteAction::Continue)
        );
        assert_eq!(registry.requests("run", "app")[0].status, None);
    }

    #[test]
    fn run_scoping_and_cleanup() {
        let mut registry = Registry::default();
        let matcher = UrlMatcher::glob("**").unwrap();
        assert!(
            registry
                .install(
                    "done",
                    "app",
                    spec(matcher.clone(), None, fulfill(200)),
                    || false
                )
                .is_err()
        );
        let a = registry
            .install(
                "a",
                "app",
                spec(matcher.clone(), None, fulfill(200)),
                || true,
            )
            .unwrap();
        registry
            .install(
                "b",
                "app",
                spec(matcher.clone(), None, fulfill(404)),
                || true,
            )
            .unwrap();
        registry
            .install("a", "other", spec(matcher, None, fulfill(500)), || true)
            .unwrap();
        // Another run cannot remove a route it does not own.
        assert!(!registry.remove("b", a));
        registry.decide("app", "GET", "http://h/", SentRequest::default, || true);

        registry.clear_run("b");
        assert_eq!(registry.len(), 2);
        assert!(registry.requests("b", "app").is_empty());
        assert_eq!(registry.remove_app("a", "app"), 1);
        registry.clear_run("a");
        assert_eq!(registry.len(), 0);
        assert_eq!(
            registry.decide("app", "GET", "http://h/", SentRequest::default, || true),
            None
        );
    }

    #[test]
    fn log_is_bounded() {
        let mut registry = Registry::default();
        registry
            .install(
                "run",
                "app",
                spec(UrlMatcher::glob("**").unwrap(), None, RouteAction::Continue),
                || true,
            )
            .unwrap();
        for i in 0..(MAX_LOG_ENTRIES + 5) {
            registry.decide(
                "app",
                "GET",
                &format!("http://h/{i}"),
                SentRequest::default,
                || true,
            );
        }
        let log = registry.requests("run", "app");
        assert_eq!(log.len(), MAX_LOG_ENTRIES);
        assert_eq!(log[0].url, "http://h/5");
    }
    #[test]
    fn request_body_is_cut_on_a_char_boundary() {
        let short = SentRequest::new(Vec::new(), Some("{}".into()), false);
        assert_eq!(
            (short.body.as_deref(), short.body_truncated),
            (Some("{}"), false)
        );

        // A 3-byte character straddles the limit and is dropped whole.
        let text = format!("{}\u{20ac}tail", "a".repeat(MAX_REQUEST_BODY_BYTES - 1));
        let long = SentRequest::new(Vec::new(), Some(text), false);
        assert!(long.body_truncated);
        assert_eq!(
            long.body.as_ref().unwrap().len(),
            MAX_REQUEST_BODY_BYTES - 1
        );

        let overflow = SentRequest::new(Vec::new(), Some("abc".into()), true);
        assert!(overflow.body_truncated);
        assert!(!SentRequest::new(Vec::new(), None, false).body_truncated);
    }

    #[test]
    fn logged_request_bodies_share_a_byte_budget() {
        let mut registry = Registry::default();
        registry
            .install(
                "run",
                "app",
                spec(UrlMatcher::glob("**").unwrap(), None, RouteAction::Continue),
                || true,
            )
            .unwrap();
        let body = "x".repeat(MAX_REQUEST_BODY_BYTES);
        let hits = MAX_LOG_BODY_BYTES / MAX_REQUEST_BODY_BYTES + 3;
        for i in 0..hits {
            registry.decide(
                "app",
                "POST",
                &format!("http://h/{i}"),
                || {
                    SentRequest::new(
                        vec![("x-n".into(), i.to_string())],
                        Some(body.clone()),
                        false,
                    )
                },
                || true,
            );
        }
        let log = registry.requests("run", "app");
        assert_eq!(log.len(), MAX_LOG_BODY_BYTES / MAX_REQUEST_BODY_BYTES);
        assert_eq!(log[0].url, "http://h/3");
        assert_eq!(
            log[0].request.headers,
            vec![("x-n".to_string(), "3".to_string())]
        );
        assert!(registry.log_body_bytes <= MAX_LOG_BODY_BYTES);
        registry.clear_run("run");
        assert_eq!(registry.log_body_bytes, 0);
    }
}
