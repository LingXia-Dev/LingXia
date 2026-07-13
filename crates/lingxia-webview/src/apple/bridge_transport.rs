use crate::webview::WebTag;
use crate::{SystemPipeReader, WebViewError};
use std::collections::VecDeque;
use std::io::Write;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub(crate) const APPLE_BRIDGE_DOWNSTREAM_URL: &str = "lx-apple://bridge/downstream";
pub(crate) const APPLE_BRIDGE_DOWNSTREAM_CSP_SOURCE: &str = "lx-apple:";
pub(super) const APPLE_INTERNAL_SCHEME: &str = "lx-apple";
const APPLE_BRIDGE_DOWNSTREAM_HOST: &str = "bridge";
const APPLE_BRIDGE_DOWNSTREAM_PATH: &str = "/downstream";
// Frames retained for reconnect replay. WebKit replaces the streaming fetch on
// its own (navigation, throttling, suspension); the client resumes by asking
// for everything after the last seq it saw, so retained frames must outlast a
// disconnect window. 4096 frames is far more than any realistic gap.
const APPLE_BRIDGE_REPLAY_LIMIT: usize = 4096;
// Query key carrying the client's last-seen transport seq on (re)connect.
const APPLE_BRIDGE_FROM_QUERY: &str = "from";
// A WKURLSchemeTask response carrying initial or replayed frames is completed
// after its first burst. WebKit can otherwise buffer later `didReceiveData`
// chunks indefinitely; EOF flushes them, and the client resumes by sequence.
const APPLE_BRIDGE_BOOTSTRAP_IDLE: Duration = Duration::from_millis(10);
// Heartbeat cadence for an idle connection. Keeps the socket warm and lets the
// client detect a silently dead connection (half-open socket) it would
// otherwise never see a read error for.
const APPLE_BRIDGE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
// A single frame write may not exceed this. Past it the client is treated as
// gone: the connection is dropped rather than blocking the writer thread.
const APPLE_BRIDGE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn is_bridge_downstream_request(request: &http::Request<Vec<u8>>) -> bool {
    let uri = request.uri();
    uri.scheme_str() == Some(APPLE_INTERNAL_SCHEME)
        && uri.authority().is_some_and(|authority| {
            authority
                .as_str()
                .eq_ignore_ascii_case(APPLE_BRIDGE_DOWNSTREAM_HOST)
        })
        && uri.path() == APPLE_BRIDGE_DOWNSTREAM_PATH
}

/// The `from` transport seq a (re)connecting client has already received, parsed
/// from the downstream request query. Absent/invalid means a fresh client (0).
pub(super) fn downstream_from_seq(request: &http::Request<Vec<u8>>) -> u64 {
    request
        .uri()
        .query()
        .into_iter()
        .flat_map(|query| query.split('&'))
        .find_map(|pair| {
            pair.strip_prefix(APPLE_BRIDGE_FROM_QUERY)?
                .strip_prefix('=')
        })
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

pub(super) fn bridge_downstream_cors_origin(request: &http::Request<Vec<u8>>) -> String {
    let Some(origin) = request
        .headers()
        .get(http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return "null".to_string();
    };

    if origin == "null" || origin.starts_with("lingxia://") || origin.starts_with("lx://") {
        origin.to_string()
    } else {
        "null".to_string()
    }
}

/// Wrap a business message in the transport envelope carrying its seq. The
/// message is already-serialized JSON, so it is spliced in without re-parsing:
/// `{"lxff":<seq>,"m":<message>}`.
fn envelope(seq: u64, message: &str) -> Vec<u8> {
    let mut frame = Vec::with_capacity(message.len() + 24);
    frame.extend_from_slice(br#"{"lxff":"#);
    frame.extend_from_slice(seq.to_string().as_bytes());
    frame.extend_from_slice(br#","m":"#);
    frame.extend_from_slice(message.as_bytes());
    frame.extend_from_slice(b"}\n");
    frame
}

/// Sentinel telling the client its requested `from` is no longer replayable
/// (buffer evicted or a fresh transport): drop the resume cursor and
/// re-handshake. Rare backstop; the common path always resumes cleanly.
fn reset_frame() -> Vec<u8> {
    b"{\"lxreset\":true}\n".to_vec()
}

/// Liveness ping for an idle connection. Carries no seq and is not retained;
/// the client treats it as proof-of-life only.
fn heartbeat_frame() -> Vec<u8> {
    b"{\"lxhb\":1}\n".to_vec()
}

/// Retained transport frames plus the seq counter. Kept free of socket/thread
/// state so the replay logic is unit-testable in isolation.
struct RetainedFrame {
    seq: u64,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorState {
    Replayable,
    CaughtUp,
    Unreplayable,
}

struct FrameLog {
    // Ascending, gap-free seq. Each entry remains independently replayable.
    buffer: VecDeque<RetainedFrame>,
    // Seq to assign to the next enqueued frame (1-based).
    next_seq: u64,
    limit: usize,
}

impl FrameLog {
    fn new(limit: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            next_seq: 1,
            limit,
        }
    }

    /// Append a message, returning its assigned seq. Oldest frames past the
    /// replay window are dropped as a backstop against an unbounded backlog.
    fn push(&mut self, message: &str) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.buffer.push_back(RetainedFrame {
            seq,
            bytes: envelope(seq, message),
        });
        while self.buffer.len() > self.limit {
            self.buffer.pop_front();
        }
        seq
    }

    /// Seq of the oldest retained frame, or `next_seq` when empty (nothing to
    /// replay yet).
    fn earliest(&self) -> u64 {
        self.buffer
            .front()
            .map(|frame| frame.seq)
            .unwrap_or(self.next_seq)
    }

    /// Whether a client that last saw `from` can be resumed: it is not ahead of
    /// us, and we still retain the next frame it needs (`from + 1`).
    fn resumable(&self, from: u64) -> bool {
        from < self.next_seq && from + 1 >= self.earliest()
    }

    fn cursor_state(&self, cursor: u64) -> CursorState {
        if cursor < self.earliest() || cursor > self.next_seq {
            CursorState::Unreplayable
        } else if cursor == self.next_seq {
            CursorState::CaughtUp
        } else {
            CursorState::Replayable
        }
    }

    /// The enveloped frame with exactly `seq`, using contiguous indexing.
    fn frame_at(&self, seq: u64) -> Option<&[u8]> {
        let front = self.buffer.front()?.seq;
        if seq < front {
            return None;
        }
        self.buffer
            .get((seq - front) as usize)
            .map(|frame| frame.bytes.as_slice())
    }

    /// Drop frames the client has confirmed by resuming past them.
    fn evict_through(&mut self, acked: u64) {
        while self.buffer.front().is_some_and(|frame| frame.seq <= acked) {
            self.buffer.pop_front();
        }
    }
}

struct AppleBridgeConnection {
    id: u64,
    writer: UnixStream,
    // Next seq to write on this connection.
    cursor: u64,
    // Finish an initial/replay response after catch-up so WebKit flushes it.
    bootstrap: bool,
    bootstrap_has_data: bool,
}

struct AppleBridgeTransportState {
    log: FrameLog,
    connection: Option<AppleBridgeConnection>,
    next_connection_id: u64,
    shutdown: bool,
}

pub(super) struct AppleBridgeTransport {
    webtag: WebTag,
    state: Mutex<AppleBridgeTransportState>,
    signal: Condvar,
}

impl AppleBridgeTransport {
    pub(super) fn new(webtag: WebTag) -> Arc<Self> {
        let transport = Arc::new(Self {
            webtag,
            state: Mutex::new(AppleBridgeTransportState {
                log: FrameLog::new(APPLE_BRIDGE_REPLAY_LIMIT),
                connection: None,
                next_connection_id: 0,
                shutdown: false,
            }),
            signal: Condvar::new(),
        });
        let worker = Arc::clone(&transport);
        std::thread::spawn(move || worker.run_writer_loop());
        transport
    }

    /// Accept a (re)connecting downstream. Unlike a naive transport this does
    /// not discard queued frames: the client's `from` acks everything it has
    /// received, and the writer loop replays the rest — so a WebKit-initiated
    /// reconnect loses nothing and the bridge session survives it.
    pub(super) fn connect_downstream(&self, from: u64) -> Result<SystemPipeReader, WebViewError> {
        let (read_end, mut write_end) = UnixStream::pair().map_err(|e| {
            WebViewError::WebView(format!(
                "Failed to create Apple bridge downstream pipe: {e}"
            ))
        })?;
        let read_fd = read_end.into_raw_fd();
        let reader = unsafe { SystemPipeReader::from_raw_fd(read_fd) };

        // Bound every write so a client that stops reading (aborted fetch,
        // suspended webview) cannot block the writer thread forever and wedge
        // the transport. A timed-out write drops the connection; the client
        // reconnects and resumes from its last seq.
        let _ = write_end.set_write_timeout(Some(APPLE_BRIDGE_WRITE_TIMEOUT));

        // Avoid an idle custom-scheme streaming response. WebKit can fail a
        // fetch before native has a real bridge frame ready if no body bytes
        // arrive promptly; an empty NDJSON line is ignored by the JS parser.
        if let Err(e) = write_end.write_all(b"\n") {
            log::debug!(
                "Apple bridge downstream priming write failed webtag={}: {}",
                self.webtag,
                e
            );
        }

        let (replaced_existing, resumable, bootstrap) = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let replaced = guard.connection.is_some();
            guard.log.evict_through(from);
            let resumable = guard.log.resumable(from);
            if !resumable {
                // Cannot replay from here: tell the client to resync. A fresh
                // handshake starts a new seq run, so reset the log too.
                let _ = write_end.write_all(&reset_frame());
                guard.log = FrameLog::new(APPLE_BRIDGE_REPLAY_LIMIT);
            }
            let cursor = if resumable {
                from + 1
            } else {
                guard.log.next_seq
            };
            let bootstrap = !resumable || from == 0 || cursor < guard.log.next_seq;
            guard.next_connection_id += 1;
            let id = guard.next_connection_id;
            guard.connection = Some(AppleBridgeConnection {
                id,
                writer: write_end,
                cursor,
                bootstrap,
                bootstrap_has_data: !resumable,
            });
            (replaced, resumable, bootstrap)
        };
        self.signal.notify_all();
        log::info!(
            "Apple bridge downstream connected webtag={} from={}{}{}{}",
            self.webtag,
            from,
            if replaced_existing {
                " (replaced existing stream)"
            } else {
                ""
            },
            if resumable {
                ""
            } else {
                " (unreplayable, reset)"
            },
            if bootstrap { " (bootstrap)" } else { "" }
        );
        Ok(reader)
    }

    pub(super) fn enqueue_message(&self, message: &str) -> Result<(), WebViewError> {
        let queued_len = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.shutdown {
                return Err(WebViewError::WebView(format!(
                    "Apple bridge downstream is closed for {}",
                    self.webtag
                )));
            }
            guard.log.push(message);
            guard.log.buffer.len()
        };
        if queued_len > (APPLE_BRIDGE_REPLAY_LIMIT / 2) {
            log::warn!(
                "Apple bridge downstream backlog webtag={} retained={}",
                self.webtag,
                queued_len
            );
        }
        self.signal.notify_one();
        Ok(())
    }

    pub(super) fn shutdown(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.shutdown = true;
        guard.connection = None;
        self.signal.notify_all();
    }

    fn run_writer_loop(self: Arc<Self>) {
        loop {
            let (connection_id, mut writer, next_cursor, frame) = {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if guard.shutdown {
                        return;
                    }
                    if let Some(connection) = guard.connection.as_ref() {
                        let cursor = connection.cursor;
                        match guard.log.cursor_state(cursor) {
                            CursorState::Replayable => {
                                if let Some(frame) = guard.log.frame_at(cursor).map(<[u8]>::to_vec)
                                {
                                    let id = connection.id;
                                    match connection.writer.try_clone() {
                                        Ok(writer) => {
                                            break (id, writer, Some(cursor + 1), frame);
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Apple bridge downstream clone failed webtag={}: {}",
                                                self.webtag,
                                                e
                                            );
                                            guard.connection = None;
                                            continue;
                                        }
                                    }
                                }
                            }
                            CursorState::CaughtUp => {}
                            CursorState::Unreplayable => {
                                log::warn!(
                                    "Apple bridge downstream cursor fell outside replay window webtag={} cursor={} earliest={} next={}",
                                    self.webtag,
                                    cursor,
                                    guard.log.earliest(),
                                    guard.log.next_seq
                                );
                                guard.connection = None;
                                continue;
                            }
                        }
                    }
                    // An initial/replay response is finite: once its first
                    // frame burst is quiet, close it so WKURLSchemeTask must
                    // flush all bytes. A caught-up reconnect is long-lived.
                    let wait_duration = match guard.connection.as_ref() {
                        Some(connection)
                            if connection.bootstrap
                                && connection.bootstrap_has_data
                                && guard.log.cursor_state(connection.cursor)
                                    == CursorState::CaughtUp =>
                        {
                            APPLE_BRIDGE_BOOTSTRAP_IDLE
                        }
                        _ => APPLE_BRIDGE_HEARTBEAT_INTERVAL,
                    };
                    let (next_guard, timeout) = self
                        .signal
                        .wait_timeout(guard, wait_duration)
                        .unwrap_or_else(|e| e.into_inner());
                    guard = next_guard;
                    if timeout.timed_out() {
                        let completed_bootstrap =
                            guard.connection.as_ref().and_then(|connection| {
                                (connection.bootstrap
                                    && connection.bootstrap_has_data
                                    && guard.log.cursor_state(connection.cursor)
                                        == CursorState::CaughtUp)
                                    .then_some(connection.id)
                            });
                        if let Some(id) = completed_bootstrap {
                            guard.connection = None;
                            log::debug!(
                                "Apple bridge downstream bootstrap completed webtag={} connection={}",
                                self.webtag,
                                id
                            );
                            continue;
                        }
                        // Snapshot under the borrow, then act, so the Err arm can
                        // clear the connection without a borrow conflict.
                        let idle = match guard.connection.as_ref() {
                            Some(connection)
                                if guard.log.cursor_state(connection.cursor)
                                    == CursorState::CaughtUp =>
                            {
                                Some((connection.id, connection.writer.try_clone()))
                            }
                            _ => None,
                        };
                        if let Some((id, cloned)) = idle {
                            match cloned {
                                Ok(writer) => break (id, writer, None, heartbeat_frame()),
                                Err(_) => guard.connection = None,
                            }
                        }
                    }
                }
            };

            if let Err(e) = writer.write_all(&frame) {
                log::debug!(
                    "Apple bridge downstream write failed webtag={}: {}",
                    self.webtag,
                    e
                );
                // Drop the broken connection only; retained frames stay so the
                // next connection replays them. The bridge session is untouched.
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if guard
                    .connection
                    .as_ref()
                    .is_some_and(|connection| connection.id == connection_id)
                {
                    guard.connection = None;
                }
            } else if let Some(next) = next_cursor {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(connection) = guard.connection.as_mut()
                    && connection.id == connection_id
                {
                    connection.cursor = next;
                    if connection.bootstrap {
                        connection.bootstrap_has_data = true;
                    }
                }
            } else {
                log::debug!("Apple bridge downstream heartbeat webtag={}", self.webtag);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::fd::FromRawFd;

    fn message_of(frame: &[u8]) -> String {
        // Strip the {"lxff":N,"m": ... } envelope back to the inner message.
        let text = std::str::from_utf8(frame).unwrap().trim_end();
        let prefix = text.find(",\"m\":").unwrap() + 5;
        text[prefix..text.len() - 1].to_string()
    }

    fn seq_of(frame: &[u8]) -> u64 {
        let text = std::str::from_utf8(frame).unwrap();
        let start = text.find("\"lxff\":").unwrap() + 7;
        let end = text[start..].find(',').unwrap() + start;
        text[start..end].parse().unwrap()
    }

    #[test]
    fn push_assigns_monotonic_seq_and_wraps_message() {
        let mut log = FrameLog::new(16);
        assert_eq!(log.push(r#"{"kind":"a"}"#), 1);
        assert_eq!(log.push(r#"{"kind":"b"}"#), 2);
        let frame = log.frame_at(1).unwrap();
        assert_eq!(seq_of(frame), 1);
        assert_eq!(message_of(frame), r#"{"kind":"a"}"#);
        assert!(frame.ends_with(b"\n"));
    }

    #[test]
    fn fresh_downstream_finishes_after_first_frame_burst() {
        let transport = AppleBridgeTransport::new(WebTag::new("test", "page", None));
        let reader = transport.connect_downstream(0).unwrap();
        transport.enqueue_message(r#"{"kind":"helloAck"}"#).unwrap();

        let mut downstream = unsafe { UnixStream::from_raw_fd(reader.into_raw_fd()) };
        downstream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        downstream.read_to_end(&mut bytes).unwrap();
        transport.shutdown();

        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with('\n'));
        assert!(text.contains(r#"{"lxff":1,"m":{"kind":"helloAck"}}"#));
    }

    #[test]
    fn from_zero_with_prequeued_frames_still_finishes() {
        let transport = AppleBridgeTransport::new(WebTag::new("test", "page", None));
        transport.enqueue_message(r#"{"kind":"helloAck"}"#).unwrap();
        transport.enqueue_message(r#"{"kind":"ready"}"#).unwrap();
        let reader = transport.connect_downstream(0).unwrap();

        let mut downstream = unsafe { UnixStream::from_raw_fd(reader.into_raw_fd()) };
        downstream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        downstream.read_to_end(&mut bytes).unwrap();
        transport.shutdown();

        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains(r#"{"lxff":1,"m":{"kind":"helloAck"}}"#));
        assert!(text.contains(r#"{"lxff":2,"m":{"kind":"ready"}}"#));
    }

    #[test]
    fn resumed_downstream_finishes_after_replaying_backlog() {
        let transport = AppleBridgeTransport::new(WebTag::new("test", "page", None));
        transport.enqueue_message(r#"{"n":1}"#).unwrap();
        transport.enqueue_message(r#"{"n":2}"#).unwrap();
        transport.enqueue_message(r#"{"n":3}"#).unwrap();
        let reader = transport.connect_downstream(1).unwrap();

        let mut downstream = unsafe { UnixStream::from_raw_fd(reader.into_raw_fd()) };
        downstream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        downstream.read_to_end(&mut bytes).unwrap();
        transport.shutdown();

        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains(r#"{"lxff":1,"m":{"n":1}}"#));
        assert!(text.contains(r#"{"lxff":2,"m":{"n":2}}"#));
        assert!(text.contains(r#"{"lxff":3,"m":{"n":3}}"#));
    }

    #[test]
    fn caught_up_resume_stays_streaming() {
        let transport = AppleBridgeTransport::new(WebTag::new("test", "page", None));
        transport.enqueue_message(r#"{"n":1}"#).unwrap();
        let reader = transport.connect_downstream(1).unwrap();
        let downstream = unsafe { UnixStream::from_raw_fd(reader.into_raw_fd()) };

        let state = transport.state.lock().unwrap();
        assert!(!state.connection.as_ref().unwrap().bootstrap);
        drop(state);
        transport.shutdown();
        drop(downstream);
    }

    #[test]
    fn unreplayable_downstream_finishes_after_reset_sentinel() {
        let transport = AppleBridgeTransport::new(WebTag::new("test", "page", None));
        let reader = transport.connect_downstream(5).unwrap();

        let mut downstream = unsafe { UnixStream::from_raw_fd(reader.into_raw_fd()) };
        downstream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        downstream.read_to_end(&mut bytes).unwrap();
        transport.shutdown();

        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .contains(r#"{"lxreset":true}"#)
        );
    }

    #[test]
    fn replay_after_reconnect_loses_no_frames() {
        // The hijack scenario: many frames enqueued, client reconnects having
        // seen only the first 60 — every later frame must still replay in order.
        let mut log = FrameLog::new(4096);
        for i in 0..100 {
            log.push(&format!("{{\"n\":{i}}}"));
        }
        let from = 60;
        log.evict_through(from);
        assert!(log.resumable(from));
        let mut seqs = Vec::new();
        let mut cursor = from + 1;
        while let Some(frame) = log.frame_at(cursor) {
            seqs.push(seq_of(frame));
            cursor += 1;
        }
        assert_eq!(seqs, (61..=100).collect::<Vec<_>>());
    }

    #[test]
    fn fresh_client_from_zero_replays_everything() {
        let mut log = FrameLog::new(4096);
        for i in 0..5 {
            log.push(&format!("{{\"n\":{i}}}"));
        }
        assert!(log.resumable(0));
        assert_eq!(seq_of(log.frame_at(1).unwrap()), 1);
        assert!(log.frame_at(6).is_none());
    }

    #[test]
    fn eviction_past_the_window_makes_stale_from_unresumable() {
        let mut log = FrameLog::new(8);
        for i in 0..20 {
            log.push(&format!("{{\"n\":{i}}}"));
        }
        // Only the last 8 frames (13..=20) survive; a client stuck at seq 3
        // cannot be replayed and must be told to reset.
        assert_eq!(log.earliest(), 13);
        assert!(!log.resumable(3));
        // A client caught up to the retained window still resumes.
        assert!(log.resumable(15));
    }

    #[test]
    fn cursor_before_replay_window_is_not_treated_as_idle() {
        let mut log = FrameLog::new(2);
        log.push(r#"{"n":1}"#);
        log.push(r#"{"n":2}"#);
        log.push(r#"{"n":3}"#);

        assert_eq!(log.earliest(), 2);
        assert_eq!(log.cursor_state(1), CursorState::Unreplayable);
        assert_eq!(log.cursor_state(2), CursorState::Replayable);
        assert_eq!(log.cursor_state(4), CursorState::CaughtUp);
        assert_eq!(log.cursor_state(5), CursorState::Unreplayable);
    }

    #[test]
    fn resumable_rejects_client_ahead_of_host() {
        let mut log = FrameLog::new(16);
        log.push(r#"{"n":0}"#);
        // next_seq is 2; a client claiming to have seen seq 5 is ahead (host
        // restarted) and must resync rather than silently stall.
        assert!(!log.resumable(5));
    }

    #[test]
    fn evict_through_drops_confirmed_and_keeps_the_rest() {
        let mut log = FrameLog::new(16);
        for i in 0..5 {
            log.push(&format!("{{\"n\":{i}}}"));
        }
        log.evict_through(3);
        assert_eq!(log.earliest(), 4);
        assert!(log.frame_at(3).is_none());
        assert_eq!(seq_of(log.frame_at(4).unwrap()), 4);
    }

    #[test]
    fn from_seq_parses_query() {
        let req = |uri: &str| http::Request::builder().uri(uri).body(Vec::new()).unwrap();
        assert_eq!(
            downstream_from_seq(&req("lx-apple://bridge/downstream?from=42")),
            42
        );
        assert_eq!(downstream_from_seq(&req("lx-apple://bridge/downstream")), 0);
        assert_eq!(
            downstream_from_seq(&req("lx-apple://bridge/downstream?from=bogus")),
            0
        );
    }
}
