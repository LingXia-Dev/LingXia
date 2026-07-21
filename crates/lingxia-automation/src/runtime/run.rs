//! Per-run shared state: the state machine, event buffer, and limits.

use super::protocol::*;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// State shared between host callers and the run task on the automation
/// worker. All mutation happens under one mutex; JS-side callbacks
/// only take it for short, non-blocking sections.
pub(crate) struct RunShared {
    pub run_id: String,
    pub started_at: Instant,
    pub timeout: Duration,
    cancel_tx: watch::Sender<bool>,
    /// Set by the manager just before it fires the engine interrupt for this
    /// run (deadline watchdog or cancel). The worker task uses it to
    /// distinguish an interrupt-induced abort from a genuine thrown error,
    /// deterministically rather than by racing the clock.
    preemption_requested: AtomicBool,
    inner: Mutex<RunInner>,
}

struct RunInner {
    state: AutomationRunState,
    events: Vec<AutomationEvent>,
    next_seq: u64,
    /// Approximate bytes of retained, undelivered non-artifact events.
    retained_event_bytes: usize,
    truncation_warned: bool,
    attachment_total: usize,
    attachment_names: HashSet<String>,
    result: Option<AutomationRunResult>,
    completed_at: Option<Instant>,
}

impl RunShared {
    pub fn new(run_id: String, timeout: Duration) -> Self {
        let (cancel_tx, _) = watch::channel(false);
        Self {
            run_id,
            started_at: Instant::now(),
            timeout,
            cancel_tx,
            preemption_requested: AtomicBool::new(false),
            inner: Mutex::new(RunInner {
                state: AutomationRunState::Running,
                events: Vec::new(),
                next_seq: 1,
                retained_event_bytes: 0,
                truncation_warned: false,
                attachment_total: 0,
                attachment_names: HashSet::new(),
                result: None,
                completed_at: None,
            }),
        }
    }

    pub fn state(&self) -> AutomationRunState {
        self.inner.lock().unwrap().state
    }

    pub fn completed_at(&self) -> Option<Instant> {
        self.inner.lock().unwrap().completed_at
    }

    /// Deadline left for JS evaluation, measured from the slot claim.
    pub fn remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.started_at.elapsed())
    }

    pub fn cancel_requested(&self) -> bool {
        *self.cancel_tx.borrow()
    }

    pub fn request_cancel(&self) {
        // `start` publishes the run before the worker subscribes. Preserve an
        // early cancellation even while the channel has no receivers yet.
        self.cancel_tx.send_replace(true);
    }

    pub fn cancel_receiver(&self) -> watch::Receiver<bool> {
        self.cancel_tx.subscribe()
    }

    /// Mark that the manager is about to (or did) fire the engine interrupt
    /// for this run.
    pub fn request_preemption(&self) {
        self.preemption_requested.store(true, Ordering::SeqCst);
    }

    pub fn preemption_requested(&self) -> bool {
        self.preemption_requested.load(Ordering::SeqCst)
    }

    pub fn push_console(&self, level: &str, message: String) {
        let mut inner = self.inner.lock().unwrap();
        if inner.state.is_terminal() {
            return;
        }
        let mut message = message;
        if message.len() > MAX_CONSOLE_EVENT_BYTES {
            truncate_utf8(&mut message, MAX_CONSOLE_EVENT_BYTES);
            message.push_str("… [truncated]");
        }
        let payload = AutomationEventPayload::Console {
            level: level.to_string(),
            message,
        };
        inner.push_retained_event(payload);
        inner.evict_if_needed();
    }

    pub fn push_event(&self, value: serde_json::Value) {
        let mut inner = self.inner.lock().unwrap();
        if inner.state.is_terminal() {
            return;
        }
        inner.push_retained_event(AutomationEventPayload::Event { value });
        inner.evict_if_needed();
    }

    /// Validate and store one artifact. `Err` carries a JS-facing message.
    pub fn push_artifact(
        &self,
        name: &str,
        mime_type: &str,
        base64: String,
        decoded_len: usize,
    ) -> Result<(), String> {
        let name = validate_attachment_name(name)?;
        if mime_type.is_empty() || mime_type.len() > 255 {
            return Err("attachment mimeType must be 1-255 characters".to_string());
        }
        let mut inner = self.inner.lock().unwrap();
        if inner.state.is_terminal() {
            return Err("automation run is no longer active".to_string());
        }
        if decoded_len > MAX_ATTACHMENT_BYTES {
            return Err(format!(
                "attachment '{name}' is {decoded_len} bytes; the limit is {MAX_ATTACHMENT_BYTES}"
            ));
        }
        let attachment_total = inner
            .attachment_total
            .checked_add(decoded_len)
            .ok_or_else(|| "attachment size overflow".to_string())?;
        if attachment_total > MAX_RUN_ATTACHMENT_BYTES {
            return Err(format!(
                "attachment '{name}' would exceed the {MAX_RUN_ATTACHMENT_BYTES}-byte run total"
            ));
        }
        if !inner.attachment_names.insert(name.clone()) {
            return Err(format!("duplicate attachment name '{name}'"));
        }
        inner.attachment_total = attachment_total;
        inner.push_event(AutomationEventPayload::Artifact {
            name,
            mime_type: mime_type.to_string(),
            base64,
        });
        Ok(())
    }

    /// Transition to a terminal state exactly once. Later calls are ignored so
    /// a watchdog and the worker task can race safely.
    pub fn finalize(
        &self,
        state: AutomationRunState,
        error: Option<AutomationRunError>,
        output: Option<serde_json::Value>,
    ) -> bool {
        debug_assert!(state.is_terminal());
        let mut inner = self.inner.lock().unwrap();
        if inner.state.is_terminal() {
            return false;
        }
        inner.state = state;
        inner.completed_at = Some(Instant::now());
        inner.result = Some(AutomationRunResult {
            duration_ms: self.started_at.elapsed().as_millis() as u64,
            error,
            output,
        });
        true
    }

    /// Events after `after_seq`; delivering also releases everything at or
    /// below it (the client acknowledged them by asking for later ones).
    pub fn poll(&self, after_seq: u64) -> AutomationPollResponse {
        let mut inner = self.inner.lock().unwrap();
        let mut released = 0usize;
        inner.events.retain(|event| {
            let keep = event.seq > after_seq;
            if !keep {
                released += retained_event_size(&event.payload);
            }
            keep
        });
        inner.retained_event_bytes = inner.retained_event_bytes.saturating_sub(released);

        let mut event_bytes = 0usize;
        let events = inner
            .events
            .iter()
            .take_while(|event| {
                let size = serialized_event_size(event);
                if event_bytes > 0 && event_bytes.saturating_add(size) > MAX_POLL_EVENT_BYTES {
                    return false;
                }
                event_bytes = event_bytes.saturating_add(size);
                true
            })
            .cloned()
            .collect();

        AutomationPollResponse {
            run_id: self.run_id.clone(),
            state: inner.state,
            next_seq: inner.next_seq,
            events,
            result: if inner.state.is_terminal() {
                inner.result.clone()
            } else {
                None
            },
        }
    }
}

impl RunInner {
    fn push_event(&mut self, payload: AutomationEventPayload) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(AutomationEvent { seq, payload });
    }

    fn push_retained_event(&mut self, payload: AutomationEventPayload) {
        self.retained_event_bytes = self
            .retained_event_bytes
            .saturating_add(retained_event_size(&payload));
        self.push_event(payload);
    }

    /// Drop the oldest undelivered non-artifact events once the retained cap
    /// is exceeded; warn once per run. Attachments have separate limits.
    fn evict_if_needed(&mut self) {
        if self.retained_event_bytes <= MAX_RETAINED_EVENT_BYTES {
            return;
        }
        let mut to_release = self.retained_event_bytes - MAX_RETAINED_EVENT_BYTES;
        self.events.retain(|event| {
            if to_release == 0 {
                return true;
            }
            let size = retained_event_size(&event.payload);
            if size == 0 {
                true
            } else {
                to_release = to_release.saturating_sub(size);
                self.retained_event_bytes = self.retained_event_bytes.saturating_sub(size);
                false
            }
        });
        if !self.truncation_warned {
            self.truncation_warned = true;
            self.push_retained_event(AutomationEventPayload::Console {
                level: "warn".to_string(),
                message: "automation output outpaced the client; oldest events were dropped"
                    .to_string(),
            });
        }
    }
}

fn retained_event_size(payload: &AutomationEventPayload) -> usize {
    match payload {
        AutomationEventPayload::Artifact { .. } => 0,
        AutomationEventPayload::Console { level, message } => level.len() + message.len() + 32,
        AutomationEventPayload::Event { value } => value.to_string().len() + 32,
    }
}

fn serialized_event_size(event: &AutomationEvent) -> usize {
    serde_json::to_vec(event).map_or(0, |encoded| encoded.len())
}

fn truncate_utf8(value: &mut String, max_len: usize) {
    let mut cut = max_len;
    while cut > 0 && !value.is_char_boundary(cut) {
        cut -= 1;
    }
    value.truncate(cut);
}

/// Attachment names must be normalized relative paths that stay below the
/// output directory: no absolute paths, no `..`, no platform prefixes.
fn validate_attachment_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 512 {
        return Err("attachment name must be 1-512 characters".to_string());
    }
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(format!("attachment name '{name}' must be a relative path"));
    }
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!(
                "attachment name '{name}' must not contain empty, '.' or '..' segments"
            ));
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> RunShared {
        RunShared::new("run".into(), Duration::from_secs(60))
    }

    #[test]
    fn finalize_is_single_shot() {
        let run = shared();
        assert!(run.finalize(AutomationRunState::Succeeded, None, None));
        assert!(!run.finalize(AutomationRunState::Failed, None, None));
        assert_eq!(run.state(), AutomationRunState::Succeeded);
    }

    #[test]
    fn poll_acks_and_releases_delivered_events() {
        let run = shared();
        run.push_console("info", "one".into());
        run.push_console("info", "two".into());
        let first = run.poll(0);
        assert_eq!(first.events.len(), 2);
        assert_eq!(first.next_seq, 3);
        let second = run.poll(2);
        assert!(second.events.is_empty());
        assert_eq!(run.inner.lock().unwrap().retained_event_bytes, 0);
    }

    #[test]
    fn structured_events_are_ordered_and_acknowledged() {
        let run = shared();
        run.push_event(serde_json::json!({ "type": "started" }));
        run.push_event(serde_json::json!({ "type": "finished" }));

        let first = run.poll(0);
        assert!(matches!(
            &first.events[0].payload,
            AutomationEventPayload::Event { value } if value["type"] == "started"
        ));
        assert!(matches!(
            &first.events[1].payload,
            AutomationEventPayload::Event { value } if value["type"] == "finished"
        ));
        assert!(run.poll(2).events.is_empty());
    }

    #[test]
    fn console_events_are_capped_per_event() {
        let run = shared();
        run.push_console("info", "x".repeat(MAX_CONSOLE_EVENT_BYTES + 10));
        let poll = run.poll(0);
        let AutomationEventPayload::Console { message, .. } = &poll.events[0].payload else {
            panic!("expected console event");
        };
        assert!(message.len() <= MAX_CONSOLE_EVENT_BYTES + "… [truncated]".len());
    }

    #[test]
    fn eviction_drops_oldest_and_warns_once() {
        let run = shared();
        let chunk = "y".repeat(MAX_CONSOLE_EVENT_BYTES);
        for _ in 0..((MAX_RETAINED_EVENT_BYTES / MAX_CONSOLE_EVENT_BYTES) + 4) {
            run.push_console("info", chunk.clone());
        }
        let inner = run.inner.lock().unwrap();
        assert!(inner.retained_event_bytes <= MAX_RETAINED_EVENT_BYTES);
        assert!(inner.truncation_warned);
        let warnings = inner
            .events
            .iter()
            .filter(|event| {
                matches!(&event.payload, AutomationEventPayload::Console { message, .. }
                    if message.contains("outpaced"))
            })
            .count();
        assert_eq!(warnings, 1);
        // The oldest event was evicted; the newest survives.
        assert!(inner.events.first().unwrap().seq > 1);
    }

    #[test]
    fn attachment_limits_and_names() {
        let run = shared();
        assert!(
            run.push_artifact("a.png", "image/png", "AAAA".into(), 3)
                .is_ok()
        );
        assert!(
            run.push_artifact("a.png", "image/png", "AAAA".into(), 3)
                .unwrap_err()
                .contains("duplicate")
        );
        for bad in ["/abs.png", "../up.png", "a/../b.png", "C:\\x.png", ""] {
            assert!(
                run.push_artifact(bad, "image/png", "AAAA".into(), 3)
                    .is_err()
            );
        }
        assert!(
            run.push_artifact(
                "big.bin",
                "application/octet-stream",
                String::new(),
                MAX_ATTACHMENT_BYTES + 1
            )
            .is_err()
        );
        assert!(run.push_artifact("bad", "", "AAAA".into(), 3).is_err());
        // Backslashes normalize to forward slashes.
        assert!(
            run.push_artifact("dir\\nested.png", "image/png", "AAAA".into(), 3)
                .is_ok()
        );
        let poll = run.poll(0);
        assert!(poll.events.iter().any(|event| {
            matches!(&event.payload, AutomationEventPayload::Artifact { name, .. } if name == "dir/nested.png")
        }));
    }

    #[test]
    fn poll_chunks_large_artifacts() {
        let run = shared();
        let base64 = "A".repeat(13 * 1024 * 1024);
        run.push_artifact(
            "one.bin",
            "application/octet-stream",
            base64.clone(),
            9 * 1024 * 1024,
        )
        .unwrap();
        run.push_artifact(
            "two.bin",
            "application/octet-stream",
            base64,
            9 * 1024 * 1024,
        )
        .unwrap();

        let first = run.poll(0);
        assert_eq!(first.events.len(), 1);
        let second = run.poll(first.events[0].seq);
        assert_eq!(second.events.len(), 1);
    }

    #[test]
    fn terminal_run_rejects_new_output() {
        let run = shared();
        run.finalize(AutomationRunState::Cancelled, None, None);
        run.push_console("info", "late".into());
        assert!(
            run.push_artifact("x.png", "image/png", "AAAA".into(), 3)
                .is_err()
        );
        assert!(run.poll(0).events.is_empty());
    }
}
