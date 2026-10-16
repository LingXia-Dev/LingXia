//! Real traffic from Rong's network observer.
//!
//! Every request sent through Rong's HTTP client reaches it: Logic `fetch`
//! and `Rong.SSE` and `lx` transfers (owner: the lxapp), and host subsystems
//! that tag themselves (`host:<subsystem>`). Routes and mocks never touch the
//! network, so the `fetch` wrapper keeps answering and logging those.
//!
//! The observer callback only hands events to a bounded queue; one thread
//! builds a record per request id and files a finished one into the recent
//! list and into the recording that was active when the request started.
//! Bodies are kept only for recorded requests, as wire bytes; a
//! `Content-Encoding` is undone only when a record is shown or replayed.

use super::capture::{self, Exchange, REDACTED, RecordedBody};
use super::registry::{self, DEV_SESSION_OWNER, UrlMatcher, now_ms};
use super::scenario;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rong_rt::http::{
    NetworkEvent, NetworkObserver, NetworkOutcome, NetworkRequestKind, NetworkRequestStart,
    NetworkResponseHead,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, Once, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Events waiting for the record builder; past it they are counted, not kept.
const QUEUE: usize = 16_384;
/// Finished requests kept for `lxdev network status`, without bodies.
const MAX_RECENT: usize = capture::MAX_CALLS;
/// Requests in flight at once; the oldest is forgotten past it.
const MAX_IN_FLIGHT: usize = 1_024;
/// Chunk arrivals and stream frames one record keeps.
const MAX_CHUNK_LOG: usize = 1_000;
const MAX_FRAMES: usize = 1_000;
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// A recording is active: body chunks are worth queueing.
static BODIES: AtomicBool = AtomicBool::new(false);
/// Events the full queue refused.
static DROPPED: AtomicU64 = AtomicU64::new(0);
static QUEUE_TX: OnceLock<SyncSender<Message>> = OnceLock::new();
static STATE: Mutex<State> = Mutex::new(State::new());

// Events are nearly every message; boxing them would allocate per event.
#[allow(clippy::large_enum_variant)]
enum Message {
    Event(NetworkEvent),
    Flush(std::sync::mpsc::Sender<()>),
}

struct Forwarder(SyncSender<Message>);

impl NetworkObserver for Forwarder {
    fn on_event(&self, event: NetworkEvent) {
        if matches!(
            event,
            NetworkEvent::RequestBodyChunk(_) | NetworkEvent::ResponseBodyChunk(_)
        ) && !BODIES.load(Ordering::Relaxed)
        {
            return;
        }
        if self.0.try_send(Message::Event(event)).is_err() {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Install the process observer. Idempotent.
pub(crate) fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let (tx, rx) = sync_channel(QUEUE);
        let spawned = std::thread::Builder::new()
            .name("lx-network-observer".into())
            .spawn(move || run(rx));
        if let Err(err) = spawned {
            log::warn!("network observer unavailable: {err}");
            return;
        }
        let _ = QUEUE_TX.set(tx.clone());
        rong_rt::http::set_network_observer(Some(Arc::new(Forwarder(tx))));
    });
}

fn run(queue: Receiver<Message>) {
    for message in queue {
        match message {
            Message::Event(event) => {
                // Asked before taking STATE: the route table lock is never
                // taken under it.
                let runs_active = matches!(event, NetworkEvent::RequestStart(_))
                    && BODIES.load(Ordering::Relaxed)
                    && registry::runs_active();
                state().apply(event, runs_active, DROPPED.load(Ordering::Relaxed));
            }
            Message::Flush(ack) => {
                let _ = ack.send(());
            }
        }
    }
}

/// Wait until every event queued so far is applied. Never call it under the
/// route table lock: the builder takes that lock.
fn flush() {
    let Some(tx) = QUEUE_TX.get() else {
        return;
    };
    let (ack, done) = std::sync::mpsc::channel();
    if tx.send(Message::Flush(ack)).is_ok() {
        let _ = done.recv_timeout(FLUSH_TIMEOUT);
    }
}

fn state() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// -------------------------------- events --------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Outcome {
    Completed,
    Failed(String),
    Aborted,
    Dropped,
    Blocked(String),
}

impl Outcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed(_) => "failed",
            Self::Aborted => "aborted",
            Self::Dropped => "dropped",
            Self::Blocked(_) => "blocked",
        }
    }
}

impl From<NetworkOutcome> for Outcome {
    fn from(outcome: NetworkOutcome) -> Self {
        match outcome {
            NetworkOutcome::Completed => Self::Completed,
            NetworkOutcome::Failed(message) => Self::Failed(message),
            NetworkOutcome::Aborted => Self::Aborted,
            NetworkOutcome::Dropped => Self::Dropped,
            NetworkOutcome::Blocked(reason) => Self::Blocked(reason),
            other => Self::Failed(format!("{other:?}")),
        }
    }
}

fn header_pairs(headers: &http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect()
}

fn kind_name(kind: NetworkRequestKind) -> &'static str {
    match kind {
        NetworkRequestKind::Fetch => "fetch",
        NetworkRequestKind::Sse => "sse",
        NetworkRequestKind::Download => "download",
        NetworkRequestKind::Upload => "upload",
        _ => "other",
    }
}

// -------------------------------- records --------------------------------

/// One request as the network saw it.
#[derive(Debug, Clone)]
pub(crate) struct Record {
    pub id: u64,
    pub owner: Option<String>,
    pub kind: &'static str,
    pub method: String,
    pub url: String,
    pub redirected_from: Option<u64>,
    pub started_ms: u64,
    pub request_headers: Vec<(String, String)>,
    pub request_size: Option<u64>,
    pub request_body: Vec<u8>,
    pub request_bytes: u64,
    pub status: Option<u16>,
    pub version: Option<String>,
    pub response_headers: Vec<(String, String)>,
    pub ttfb: Option<Duration>,
    /// Wire bytes, before any content decoding, up to `body_limit`.
    pub response_body: Vec<u8>,
    pub response_bytes: u64,
    pub chunks: usize,
    /// `(ms since start, bytes)` per response chunk.
    pub chunk_log: Vec<(u64, usize)>,
    pub outcome: Option<Outcome>,
    pub duration: Option<Duration>,
    /// Why the bodies are missing although the request was recorded.
    pub body_note: Option<String>,
    /// Queue overflow lost events while this request was in flight.
    pub incomplete: bool,
    recorder: Option<String>,
    body_limit: usize,
    at: Instant,
    dropped_at_start: u64,
}

impl Record {
    fn new(start: NetworkRequestStart, recorder: Option<String>, dropped: u64) -> Self {
        Self {
            id: start.id.as_u64(),
            owner: start.owner.map(|owner| owner.as_str().to_string()),
            kind: kind_name(start.kind),
            method: start.method.as_str().to_string(),
            url: start.uri.to_string(),
            redirected_from: start.redirected_from.map(|id| id.as_u64()),
            started_ms: start
                .started_at_system
                .duration_since(UNIX_EPOCH)
                .map_or_else(|_| now_ms(), |since| since.as_millis() as u64),
            request_headers: header_pairs(&start.headers),
            request_size: start.body_size,
            request_body: Vec::new(),
            request_bytes: 0,
            status: None,
            version: None,
            response_headers: Vec::new(),
            ttfb: None,
            response_body: Vec::new(),
            response_bytes: 0,
            chunks: 0,
            chunk_log: Vec::new(),
            outcome: None,
            duration: None,
            body_note: None,
            incomplete: false,
            recorder,
            body_limit: capture::MAX_TEXT_BODY_BYTES,
            at: start.started_at,
            dropped_at_start: dropped,
        }
    }

    fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn content_type(&self) -> Option<&str> {
        Self::header(&self.response_headers, "content-type")
    }

    fn request_chunk(&mut self, chunk: &[u8]) {
        self.request_bytes += chunk.len() as u64;
        if self.recorder.is_some() {
            let room = registry::MAX_REQUEST_BODY_BYTES.saturating_sub(self.request_body.len());
            self.request_body
                .extend_from_slice(&chunk[..chunk.len().min(room)]);
        }
    }

    fn head(&mut self, head: NetworkResponseHead) {
        self.status = Some(head.status.as_u16());
        self.version = Some(format!("{:?}", head.version));
        self.response_headers = header_pairs(&head.headers);
        self.ttfb = Some(head.time_to_first_byte);
        self.body_limit = if self.content_type().is_some_and(is_textual) {
            capture::MAX_TEXT_BODY_BYTES
        } else {
            capture::MAX_BINARY_BODY_BYTES
        };
    }

    fn response_chunk(&mut self, chunk: &[u8], at: Instant) {
        self.response_bytes += chunk.len() as u64;
        self.chunks += 1;
        if self.recorder.is_none() {
            return;
        }
        if self.chunk_log.len() < MAX_CHUNK_LOG {
            let offset = at.saturating_duration_since(self.at).as_millis() as u64;
            self.chunk_log.push((offset, chunk.len()));
        }
        let room = self.body_limit.saturating_sub(self.response_body.len());
        self.response_body
            .extend_from_slice(&chunk[..chunk.len().min(room)]);
    }

    fn end(&mut self, outcome: Outcome, response_bytes: u64, duration: Duration, dropped: u64) {
        self.outcome = Some(outcome);
        self.response_bytes = self.response_bytes.max(response_bytes);
        self.duration = Some(duration);
        self.incomplete = dropped != self.dropped_at_start;
    }

    fn body_len(&self) -> usize {
        self.request_body.len() + self.response_body.len()
    }

    fn drop_bodies(&mut self, why: &str) {
        self.request_body = Vec::new();
        self.response_body = Vec::new();
        self.chunk_log = Vec::new();
        self.body_note = Some(why.to_string());
    }

    fn response_truncated(&self) -> bool {
        self.response_bytes > self.response_body.len() as u64
    }

    /// The response body as the app reads it: content encoding undone, cut
    /// at `limit`. `true` when cut, or when the wire bytes were.
    pub(crate) fn decoded_body(&self, limit: usize) -> (Vec<u8>, bool) {
        let encoding = Self::header(&self.response_headers, "content-encoding")
            .map(|encoding| encoding.trim().to_ascii_lowercase());
        let wire = self.response_body.as_slice();
        let (body, cut) = match encoding.as_deref() {
            Some("gzip" | "x-gzip") => inflate(flate2::read::GzDecoder::new(wire), limit),
            Some("deflate") => {
                let (body, cut) = inflate(flate2::read::ZlibDecoder::new(wire), limit);
                if body.is_empty() && !wire.is_empty() {
                    inflate(flate2::read::DeflateDecoder::new(wire), limit)
                } else {
                    (body, cut)
                }
            }
            _ if wire.len() > limit => (wire[..limit].to_vec(), true),
            _ => (wire.to_vec(), false),
        };
        (body, cut || self.response_truncated())
    }

    /// The report form, bodies included.
    pub(crate) fn to_json(&self) -> Value {
        let mut out = self.summary_json();
        out["startedAt"] = json!(scenario::iso_utc(self.started_ms as i64));
        out["requestHeaders"] = headers_json(&self.request_headers);
        out["responseHeaders"] = headers_json(&self.response_headers);
        if let Some(note) = &self.body_note {
            out["bodyNote"] = json!(note);
        }
        if self.recorder.is_some() || !self.request_body.is_empty() {
            let content_type = Self::header(&self.request_headers, "content-type");
            if !self.request_body.is_empty() {
                out["requestBody"] = body_json(&self.request_body, content_type);
            }
            if self.request_bytes > self.request_body.len() as u64 {
                out["requestBodyTruncated"] = json!(true);
            }
        }
        if self.body_note.is_some() || self.response_body.is_empty() && self.response_bytes == 0 {
            return out;
        }
        let content_type = self.content_type().map(str::to_ascii_lowercase);
        let (body, cut) = self.decoded_body(capture::MAX_TEXT_BODY_BYTES);
        if let Some(encoding) = Self::header(&self.response_headers, "content-encoding") {
            out["responseDecoded"] = json!(encoding);
        }
        if cut {
            out["responseBodyTruncated"] = json!(true);
        }
        let streamed = content_type.as_deref().is_some_and(is_stream_type);
        // A stream shows as its frames, each redacted on its own; the raw
        // text beside them would carry what the frames mask.
        if !body.is_empty() && !streamed {
            out["responseBody"] = body_json(&body, content_type.as_deref());
        }
        if streamed || self.kind == "sse" {
            out["chunkLog"] = json!(self.chunk_log);
            let text = String::from_utf8_lossy(&body);
            if content_type
                .as_deref()
                .is_some_and(|kind| kind.starts_with("text/event-stream"))
            {
                out["events"] = Value::Array(sse_events(&text));
            } else if streamed {
                out["lines"] = Value::Array(
                    text.lines()
                        .filter(|line| !line.trim().is_empty())
                        .take(MAX_FRAMES)
                        .map(frame_json)
                        .collect(),
                );
            }
        }
        out
    }

    /// Without headers or bodies: what `lxdev network status` lists.
    pub(crate) fn summary_json(&self) -> Value {
        let mut out = json!({
            "id": self.id,
            "owner": self.owner,
            "kind": self.kind,
            "method": self.method,
            "url": capture::redact_url(&self.url, REDACTED),
            "time": self.started_ms,
            "status": self.status,
            "httpVersion": self.version,
            "ttfbMs": self.ttfb.map(millis),
            "durationMs": self.duration.map(millis),
            "outcome": self.outcome.as_ref().map_or("inFlight", Outcome::as_str),
            "requestBytes": self.request_size.unwrap_or(self.request_bytes),
            "responseBytes": self.response_bytes,
            "chunks": self.chunks,
        });
        if let Some(from) = self.redirected_from {
            out["redirectedFrom"] = json!(from);
        }
        if let Some(Outcome::Failed(error) | Outcome::Blocked(error)) = &self.outcome {
            out["error"] = json!(capture::redact_text(error));
        }
        if self.incomplete {
            out["incomplete"] = json!(true);
        }
        out
    }

    /// The scenario answer for a request (or redirect chain) starting at
    /// `head` and ending here; `None` for the app's own abort.
    fn exchange(&self, head: &Record) -> Option<Exchange> {
        let outcome = self.outcome.as_ref()?;
        let error = match outcome {
            Outcome::Aborted => return None,
            Outcome::Failed(error) | Outcome::Blocked(error) => Some(error.clone()),
            _ => None,
        };
        Some(Exchange {
            method: head.method.clone(),
            url: head.url.clone(),
            status: if error.is_some() { None } else { self.status },
            content_type: self.content_type().map(str::to_string),
            body: if error.is_some() {
                None
            } else {
                Some(self.scenario_body())
            },
            error,
        })
    }

    fn scenario_body(&self) -> RecordedBody {
        if let Some(note) = &self.body_note {
            return RecordedBody::Omitted(note.clone());
        }
        let content_type = self.content_type().map(str::to_ascii_lowercase);
        let content_type = content_type.as_deref().unwrap_or_default();
        if content_type.starts_with("text/event-stream") {
            return RecordedBody::Omitted(
                "event stream body not recorded; write an sse answer".into(),
            );
        }
        if is_stream_type(content_type) {
            return RecordedBody::Omitted("streaming body not recorded".into());
        }
        if self.outcome == Some(Outcome::Dropped) {
            return RecordedBody::Omitted(
                "the app did not read the response body to its end".into(),
            );
        }
        let textual = is_textual(content_type);
        let limit = if textual {
            capture::MAX_TEXT_BODY_BYTES
        } else {
            capture::MAX_BINARY_BODY_BYTES
        };
        let (body, cut) = self.decoded_body(limit);
        let kind = if textual { "text" } else { "binary" };
        if cut {
            return RecordedBody::Omitted(format!(
                "{kind} body of {} bytes not recorded",
                self.response_bytes
            ));
        }
        match String::from_utf8(body) {
            Ok(text) if textual => RecordedBody::Text(text),
            Ok(text) => RecordedBody::Binary(text.into_bytes()),
            Err(err) => RecordedBody::Binary(err.into_bytes()),
        }
    }
}

fn millis(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 10_000.0).round() / 10.0
}

/// What `reader` inflates to, cut at `limit`; `true` when cut or when the
/// input ended early (a capped capture), with what came out so far.
fn inflate(mut reader: impl Read, limit: usize) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return (out, false),
            Ok(read) => {
                if out.len() + read > limit {
                    out.extend_from_slice(&buffer[..limit - out.len()]);
                    return (out, true);
                }
                out.extend_from_slice(&buffer[..read]);
            }
            Err(_) => return (out, true),
        }
    }
}

fn is_textual(content_type: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();
    content_type.starts_with("text/")
        || [
            "json",
            "xml",
            "javascript",
            "x-www-form-urlencoded",
            "graphql",
        ]
        .iter()
        .any(|kind| content_type.contains(kind))
}

fn is_stream_type(content_type: &str) -> bool {
    content_type.starts_with("text/event-stream")
        || ["ndjson", "json-seq", "jsonl"]
            .iter()
            .any(|kind| content_type.contains(kind))
}

fn headers_json(headers: &[(String, String)]) -> Value {
    Value::Array(
        headers
            .iter()
            .map(|(name, value)| {
                let value = if capture::is_secret_field(name) {
                    REDACTED.to_string()
                } else if name.eq_ignore_ascii_case("location") {
                    capture::redact_url(value, REDACTED)
                } else {
                    capture::redact_text(value).into_owned()
                };
                json!([name, value])
            })
            .collect(),
    )
}

/// A body for display: redacted JSON when it is JSON, a form with secret
/// values masked, redacted text when textual (or untyped UTF-8), else base64.
/// Multipart parts carry files and fields no masking can see into: omitted.
fn body_json(bytes: &[u8], content_type: Option<&str>) -> Value {
    let lower = content_type.map(str::to_ascii_lowercase);
    if lower
        .as_deref()
        .is_some_and(|kind| kind.starts_with("multipart/"))
    {
        return json!({ "omitted": format!("multipart body of {} bytes", bytes.len()) });
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return json!({ "base64": BASE64.encode(bytes) });
    };
    // Untyped JSON is redacted by field too.
    let json_shaped = content_type.is_some_and(capture::is_json_type)
        || content_type.is_none() && text.trim_start().starts_with(['{', '[']);
    if json_shaped && let Ok(mut value) = serde_json::from_str::<Value>(text) {
        capture::redact_json(&mut value);
        return value;
    }
    if text.contains('\0') || content_type.is_some_and(|kind| !is_textual(kind)) {
        return json!({ "base64": BASE64.encode(bytes) });
    }
    if lower
        .as_deref()
        .is_some_and(|kind| kind.contains("x-www-form-urlencoded"))
    {
        return json!(capture::redact_text(&capture::redact_form(text, REDACTED)));
    }
    json!(capture::redact_text(text))
}

/// One NDJSON line or SSE `data`: redacted by field when it is JSON.
fn frame_json(text: &str) -> Value {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) if value.is_object() || value.is_array() => {
            capture::redact_json(&mut value);
            value
        }
        _ => json!(capture::redact_text(text)),
    }
}

/// `text/event-stream` events: `{ event?, data, id? }`.
fn sse_events(text: &str) -> Vec<Value> {
    let text = text.replace("\r\n", "\n");
    text.split("\n\n")
        .filter_map(|block| {
            let mut event = None;
            let mut id = None;
            let mut data: Vec<&str> = Vec::new();
            for line in block.lines() {
                let (field, value) = line.split_once(':').unwrap_or((line, ""));
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "event" => event = Some(value),
                    "data" => data.push(value),
                    "id" => id = Some(value),
                    _ => {}
                }
            }
            if data.is_empty() && event.is_none() {
                return None;
            }
            let mut out = json!({ "data": frame_json(&data.join("\n")) });
            if let Some(event) = event {
                out["event"] = json!(event);
            }
            if let Some(id) = id {
                out["id"] = json!(id);
            }
            Some(out)
        })
        .take(MAX_FRAMES)
        .collect()
}

// ------------------------------- recordings -------------------------------

/// Real traffic being recorded for a dev session or a run.
#[derive(Debug)]
pub(crate) struct Recording {
    pub owner: String,
    pub appid: Option<String>,
    pub matcher: Option<UrlMatcher>,
    pub started_ms: u64,
    records: Vec<Record>,
    body_bytes: usize,
    /// Records past the recording's limit.
    dropped: usize,
    /// Requests a dropped record was redirected from: their chain went on.
    dropped_followed: HashSet<u64>,
}

impl Recording {
    fn new(owner: &str, appid: Option<String>, matcher: Option<UrlMatcher>) -> Self {
        Self {
            owner: owner.to_string(),
            appid,
            matcher,
            started_ms: now_ms(),
            records: Vec::new(),
            body_bytes: 0,
            dropped: 0,
            dropped_followed: HashSet::new(),
        }
    }

    fn wants(&self, owner: Option<&str>, url: &str) -> bool {
        self.appid
            .as_deref()
            .is_none_or(|appid| owner == Some(appid))
            && self
                .matcher
                .as_ref()
                .is_none_or(|matcher| matcher.is_match(url))
    }

    fn push(&mut self, mut record: Record) {
        if self.records.len() >= capture::MAX_EXCHANGES {
            self.dropped += 1;
            self.dropped_followed.extend(record.redirected_from);
            return;
        }
        let len = record.body_len();
        if self.body_bytes + len > capture::MAX_RECORDED_BYTES {
            record.drop_bodies("the recording's body budget was spent");
        } else {
            self.body_bytes += len;
        }
        self.records.push(record);
    }

    /// The scenario of the lxapp `fetch` exchanges (a redirect chain is one
    /// exchange: its first request, its last answer), and every record.
    fn finish(self, in_flight: Vec<Record>) -> Stopped {
        let mut scenario = capture::Recording::default();
        scenario.dropped = self.dropped + in_flight.len();
        let by_id: HashMap<u64, &Record> = self.records.iter().map(|r| (r.id, r)).collect();
        // A hop that was dropped or is still in flight means the chain's
        // answer is unknown: its earlier hops are not answers either.
        let followed: HashSet<u64> = self
            .records
            .iter()
            .chain(&in_flight)
            .filter_map(|record| record.redirected_from)
            .chain(self.dropped_followed.iter().copied())
            .collect();
        for record in &self.records {
            if record.kind != "fetch"
                || !is_lxapp_owner(record.owner.as_deref())
                || followed.contains(&record.id)
            {
                continue;
            }
            let mut head = record;
            while let Some(previous) = head.redirected_from.and_then(|id| by_id.get(&id)) {
                head = previous;
            }
            if let Some(exchange) = record.exchange(head) {
                scenario.push(exchange);
            }
        }
        let mut requests: Vec<&Record> = self.records.iter().chain(in_flight.iter()).collect();
        requests.sort_by_key(|record| record.id);
        Stopped {
            started_ms: self.started_ms,
            requests: requests.into_iter().map(Record::to_json).collect(),
            scenario,
        }
    }
}

fn is_lxapp_owner(owner: Option<&str>) -> bool {
    owner.is_some_and(|owner| !owner.starts_with(lingxia_transfer::HOST_NETWORK_OWNER_PREFIX))
}

/// A stopped recording.
pub(crate) struct Stopped {
    pub started_ms: u64,
    /// Every recorded request in start order, in its report form.
    pub requests: Vec<Value>,
    pub scenario: capture::Recording,
}

// --------------------------------- state ---------------------------------

#[derive(Debug)]
pub(crate) struct State {
    in_flight: BTreeMap<u64, Record>,
    recent: VecDeque<Record>,
    /// The dev session's recording; it pauses while a run is active.
    dev: Option<Recording>,
    /// A run's recording (`--record-network`).
    run: Option<Recording>,
}

impl State {
    pub(crate) const fn new() -> Self {
        Self {
            in_flight: BTreeMap::new(),
            recent: VecDeque::new(),
            dev: None,
            run: None,
        }
    }

    fn slot(&mut self, owner: &str) -> &mut Option<Recording> {
        if owner == DEV_SESSION_OWNER {
            &mut self.dev
        } else {
            &mut self.run
        }
    }

    fn recording_mut(&mut self, owner: &str) -> Option<&mut Recording> {
        self.slot(owner)
            .as_mut()
            .filter(|recording| recording.owner == owner)
    }

    fn recorder_of(&self, id: u64) -> Option<Option<String>> {
        if let Some(record) = self.in_flight.get(&id) {
            return Some(record.recorder.clone());
        }
        [self.run.as_ref(), self.dev.as_ref()]
            .into_iter()
            .flatten()
            .find(|recording| recording.records.iter().rev().any(|record| record.id == id))
            .map(|recording| Some(recording.owner.clone()))
    }

    fn recorder_for(&self, start: &NetworkRequestStart, runs_active: bool) -> Option<String> {
        // A redirect hop goes where the request it follows went.
        if let Some(recorder) = start
            .redirected_from
            .and_then(|id| self.recorder_of(id.as_u64()))
        {
            return recorder;
        }
        let recording = if runs_active {
            self.run.as_ref()
        } else {
            self.dev.as_ref()
        };
        recording
            .filter(|recording| {
                recording.wants(
                    start.owner.as_ref().map(|owner| owner.as_str()),
                    &start.uri.to_string(),
                )
            })
            .map(|recording| recording.owner.clone())
    }

    pub(crate) fn apply(&mut self, event: NetworkEvent, runs_active: bool, dropped: u64) {
        let id = event.id().as_u64();
        match event {
            NetworkEvent::RequestStart(start) => {
                let recorder = self.recorder_for(&start, runs_active);
                if self.in_flight.len() >= MAX_IN_FLIGHT {
                    self.evict();
                }
                self.in_flight
                    .insert(id, Record::new(start, recorder, dropped));
            }
            NetworkEvent::RequestBodyChunk(chunk) => {
                if let Some(record) = self.in_flight.get_mut(&id) {
                    record.request_chunk(&chunk.chunk);
                }
            }
            NetworkEvent::ResponseHead(head) => {
                if let Some(record) = self.in_flight.get_mut(&id) {
                    record.head(head);
                }
            }
            NetworkEvent::ResponseBodyChunk(chunk) => {
                if let Some(record) = self.in_flight.get_mut(&id) {
                    record.response_chunk(&chunk.chunk, chunk.at);
                }
            }
            NetworkEvent::RequestEnd(end) => {
                if let Some(mut record) = self.in_flight.remove(&id) {
                    record.end(
                        end.outcome.into(),
                        end.response_bytes,
                        end.duration,
                        dropped,
                    );
                    self.finish(record);
                }
            }
            _ => {}
        }
    }

    /// Make room: forget the oldest unrecorded request, else file the oldest
    /// recorded one (often a long-lived stream) as incomplete so it is not
    /// silently lost.
    fn evict(&mut self) {
        let unrecorded = self
            .in_flight
            .iter()
            .find(|(_, record)| record.recorder.is_none())
            .map(|(id, _)| *id);
        let Some(id) = unrecorded.or_else(|| self.in_flight.keys().next().copied()) else {
            return;
        };
        let Some(mut record) = self.in_flight.remove(&id) else {
            return;
        };
        if let Some(owner) = record.recorder.clone()
            && let Some(recording) = self.recording_mut(&owner)
        {
            record.incomplete = true;
            record.drop_bodies("too many requests were in flight to keep following it");
            recording.push(record);
        }
    }

    fn finish(&mut self, record: Record) {
        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        let mut summary = record.clone();
        summary.drop_bodies("");
        summary.body_note = None;
        self.recent.push_back(summary);
        if let Some(owner) = record.recorder.clone()
            && let Some(recording) = self.recording_mut(&owner)
        {
            recording.push(record);
        }
    }

    pub(crate) fn start(
        &mut self,
        owner: &str,
        appid: Option<String>,
        matcher: Option<UrlMatcher>,
    ) -> Result<(), String> {
        let slot = self.slot(owner);
        if let Some(active) = slot {
            return Err(if active.owner == DEV_SESSION_OWNER {
                "a dev-session network recording is already running; stop it first \
                 (`lxdev network record stop`)"
                    .to_string()
            } else {
                "another test run is recording network traffic".to_string()
            });
        }
        *slot = Some(Recording::new(owner, appid, matcher));
        Ok(())
    }

    pub(crate) fn stop(&mut self, owner: &str) -> Option<Stopped> {
        let recording = self
            .slot(owner)
            .take_if(|recording| recording.owner == owner)?;
        let mut in_flight = Vec::new();
        for record in self.in_flight.values_mut() {
            if record.recorder.as_deref() == Some(owner) {
                in_flight.push(record.clone());
                record.recorder = None;
                record.drop_bodies("");
                record.body_note = None;
            }
        }
        Some(recording.finish(in_flight))
    }

    fn recording(&self) -> bool {
        self.dev.is_some() || self.run.is_some()
    }
}

// ------------------------------ the module API ------------------------------

fn sync_bodies(state: &State) {
    BODIES.store(state.recording(), Ordering::Relaxed);
}

/// Start recording real traffic for `owner` (a run id or the dev session):
/// `appid`'s requests (every owner's when `None`), optionally only URLs
/// `matcher` matches.
pub(crate) fn start_recording(
    owner: &str,
    appid: Option<String>,
    matcher: Option<UrlMatcher>,
) -> Result<(), String> {
    install();
    let mut state = state();
    state.start(owner, appid, matcher)?;
    sync_bodies(&state);
    Ok(())
}

/// Stop `owner`'s recording once the events queued so far are applied.
/// Requests still in flight are listed but not turned into exchanges.
pub(crate) fn stop_recording(owner: &str) -> Option<Stopped> {
    flush();
    let mut state = state();
    let stopped = state.stop(owner);
    sync_bodies(&state);
    stopped
}

/// Drop `owner`'s recording, if any. Returns whether one ran.
pub(crate) fn drop_recording(owner: &str) -> bool {
    let mut state = state();
    let dropped = state
        .slot(owner)
        .take_if(|recording| recording.owner == owner)
        .is_some();
    sync_bodies(&state);
    dropped
}

/// The active recording for `lxdev network status`.
pub(crate) fn recording_status(runs_active: bool) -> Option<Value> {
    let state = state();
    let recording = state.dev.as_ref().or(state.run.as_ref())?;
    let dev = recording.owner == DEV_SESSION_OWNER;
    Some(json!({
        "owner": if dev { "dev-session" } else { "test-run" },
        "appid": recording.appid,
        "match": recording.matcher.as_ref().map(UrlMatcher::label),
        "startedAt": scenario::iso_utc(recording.started_ms as i64),
        "exchanges": recording.records.len(),
        "inFlight": state
            .in_flight
            .values()
            .filter(|record| record.recorder.as_deref() == Some(recording.owner.as_str()))
            .count(),
        // A dev recording pauses while a test run is active.
        "paused": dev && runs_active,
    }))
}

/// The observer itself and the newest `limit` finished requests.
pub(crate) fn status(limit: usize) -> (Value, Value) {
    let state = state();
    let skip = state.recent.len().saturating_sub(limit);
    let recent = state
        .recent
        .iter()
        .skip(skip)
        .map(Record::summary_json)
        .collect();
    (
        json!({
            "installed": rong_rt::http::network_observer_installed(),
            "inFlight": state.in_flight.len(),
            "droppedEvents": DROPPED.load(Ordering::Relaxed),
        }),
        Value::Array(recent),
    )
}

#[cfg(test)]
mod tests;
