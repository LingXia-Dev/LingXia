//! Page failures recorded while a host test run is active, for the runner to grade.
//! Outside a run nothing is kept and recording is one atomic load.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

const CAPACITY: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageEventKind {
    PageFailed,
    WebviewCrashed,
    WebviewLoadFailed,
    FrameGap,
    BlankPage,
}

/// What a hook reports; the ring adds `seq` and the time.
#[derive(Clone, Debug)]
pub struct PageEventInput {
    pub kind: PageEventKind,
    pub app_id: String,
    pub page: Option<String>,
    pub instance: Option<String>,
    pub surface: Option<String>,
    pub code: Option<String>,
    pub message: String,
    pub gap_ms: Option<u64>,
}

impl PageEventInput {
    /// An event on page `instance`; a leading `CODE:` of `error` becomes the code.
    pub fn page(
        kind: PageEventKind,
        app_id: String,
        page: String,
        instance: String,
        error: &str,
    ) -> Self {
        let (code, message) = split_code(error);
        Self {
            kind,
            app_id,
            page: Some(page),
            instance: Some(instance),
            surface: None,
            code,
            message,
            gap_ms: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageEvent {
    pub seq: u64,
    pub at_ms: u64,
    pub kind: PageEventKind,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_ms: Option<u64>,
}

struct Ring {
    token: u64,
    started: Instant,
    next_seq: u64,
    events: VecDeque<PageEvent>,
}

static ARMED: AtomicBool = AtomicBool::new(false);
static TOKENS: AtomicU64 = AtomicU64::new(1);
static RING: Mutex<Option<Ring>> = Mutex::new(None);

fn ring() -> std::sync::MutexGuard<'static, Option<Ring>> {
    RING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start recording for a run, dropping whatever an earlier run left.
pub fn arm() -> u64 {
    let token = TOKENS.fetch_add(1, Ordering::Relaxed);
    *ring() = Some(Ring {
        token,
        started: Instant::now(),
        next_seq: 1,
        events: VecDeque::new(),
    });
    ARMED.store(true, Ordering::Release);
    token
}

/// Stop recording, unless a later run armed the ring since.
pub fn disarm(token: u64) {
    let mut ring = ring();
    if ring.as_ref().is_some_and(|ring| ring.token == token) {
        *ring = None;
        ARMED.store(false, Ordering::Release);
    }
}

/// A run is recording.
pub fn armed() -> bool {
    ARMED.load(Ordering::Acquire)
}

pub fn record(input: PageEventInput) {
    if !armed() {
        return;
    }
    let mut guard = ring();
    let Some(ring) = guard.as_mut() else {
        return;
    };
    let seq = ring.next_seq;
    ring.next_seq += 1;
    if ring.events.len() == CAPACITY {
        ring.events.pop_front();
    }
    ring.events.push_back(PageEvent {
        seq,
        at_ms: ring.started.elapsed().as_millis() as u64,
        kind: input.kind,
        app_id: input.app_id,
        page: input.page,
        instance: input.instance,
        surface: input.surface,
        code: input.code,
        message: input.message,
        gap_ms: input.gap_ms,
    });
}

/// Events after `since`, and how many after it were lost to the bound; `None` outside a run.
pub fn read(since: u64) -> Option<(Vec<PageEvent>, u64)> {
    let guard = ring();
    let ring = guard.as_ref()?;
    let first = ring.events.front().map_or(ring.next_seq, |event| event.seq);
    let dropped = first.saturating_sub(since + 1);
    let events = ring
        .events
        .iter()
        .filter(|event| event.seq > since)
        .cloned()
        .collect();
    Some((events, dropped))
}

/// `UI_TIMEOUT: render took too long` → the code and the rest.
fn split_code(error: &str) -> (Option<String>, String) {
    if let Some((code, rest)) = error.split_once(':') {
        let is_code = code.len() >= 2
            && code.starts_with(|c: char| c.is_ascii_uppercase())
            && code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if is_code {
            return (Some(code.to_owned()), rest.trim_start().to_owned());
        }
    }
    (None, error.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(message: &str) -> PageEventInput {
        PageEventInput::page(
            PageEventKind::PageFailed,
            "app".into(),
            "pages/home".into(),
            "i1".into(),
            message,
        )
    }

    // One test: the ring is process-wide.
    #[test]
    fn records_only_while_armed_and_counts_what_the_bound_lost() {
        record(event("UI_TIMEOUT: before"));
        assert!(read(0).is_none());

        let token = arm();
        record(event("UI_TIMEOUT: render took too long"));
        let (events, dropped) = read(0).unwrap();
        assert_eq!(dropped, 0);
        assert_eq!(events[0].code.as_deref(), Some("UI_TIMEOUT"));
        assert_eq!(events[0].message, "render took too long");
        assert!(read(1).unwrap().0.is_empty());

        for n in 0..CAPACITY + 3 {
            record(event(&format!("plain {n}")));
        }
        let (events, dropped) = read(1).unwrap();
        assert_eq!(events.len(), CAPACITY);
        assert_eq!(dropped, 3);
        assert_eq!(events[0].code, None);

        let later = arm();
        disarm(token);
        assert!(armed(), "a stale disarm leaves the newer run recording");
        disarm(later);
        assert!(!armed());
        assert!(read(0).is_none());
    }
}
