//! The live feed `lxdev network devtools` shows. Request events queue here
//! until the dev session bridge drains them into the session log, which
//! lxdev follows; nothing waits on a command. Bodies stay out of the events:
//! the finished requests the feed showed keep theirs here, bounded, until
//! DevTools asks (`session.network.body`).

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A feed nobody renewed for this long ends.
pub(super) const WATCH_LEASE: Duration = Duration::from_secs(30);
/// Events waiting for the bridge; past either bound the oldest are dropped.
pub(super) const MAX_QUEUED_EVENTS: usize = 4_096;
const MAX_QUEUED_BYTES: usize = 4 * 1024 * 1024;
/// Bodies of finished requests kept for DevTools.
const MAX_BODIES: usize = 500;
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct Watch {
    queue: VecDeque<(Value, usize)>,
    queued_bytes: usize,
    /// Events the full queue dropped, reported with the next drain.
    lost: u64,
    bodies: VecDeque<(u64, Value, usize)>,
    body_bytes: usize,
    lease_until: Instant,
}

fn size_of(value: &Value) -> usize {
    serde_json::to_string(value).map_or(0, |text| text.len())
}

impl Watch {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            queue: VecDeque::new(),
            queued_bytes: 0,
            lost: 0,
            bodies: VecDeque::new(),
            body_bytes: 0,
            lease_until: now + WATCH_LEASE,
        }
    }

    pub(super) fn expired(&self, now: Instant) -> bool {
        now > self.lease_until
    }

    pub(super) fn renew(&mut self, now: Instant) {
        self.lease_until = now + WATCH_LEASE;
    }

    pub(super) fn push(&mut self, event: Value) {
        let size = size_of(&event);
        self.queue.push_back((event, size));
        self.queued_bytes += size;
        while self.queue.len() > MAX_QUEUED_EVENTS || self.queued_bytes > MAX_QUEUED_BYTES {
            let Some((_, size)) = self.queue.pop_front() else {
                break;
            };
            self.queued_bytes -= size;
            self.lost += 1;
        }
    }

    /// Body bytes arrive in many chunks; a queued `data` event for the same
    /// request absorbs the next one instead of filling the queue.
    pub(super) fn push_data(&mut self, id: u64, bytes: usize, time: u64) {
        if let Some((event, _)) = self.queue.back_mut()
            && event["type"] == "data"
            && event["id"] == id
        {
            let total = event["bytes"].as_u64().unwrap_or(0) + bytes as u64;
            event["bytes"] = json!(total);
            event["time"] = json!(time);
            return;
        }
        self.push(json!({ "type": "data", "id": id, "bytes": bytes, "time": time }));
    }

    /// Everything queued, oldest first; a `lost` event leads when the queue
    /// overflowed since the last drain.
    pub(super) fn drain(&mut self) -> Vec<Value> {
        let mut events = Vec::with_capacity(self.queue.len() + 1);
        if self.lost > 0 {
            events.push(json!({ "type": "lost", "events": self.lost }));
            self.lost = 0;
        }
        events.extend(self.queue.drain(..).map(|(event, _)| event));
        self.queued_bytes = 0;
        events
    }

    /// Keep a finished request's report form, bodies included.
    pub(super) fn keep_body(&mut self, id: u64, report: Value) {
        let size = size_of(&report);
        if size > MAX_BODY_BYTES {
            return;
        }
        self.bodies.push_back((id, report, size));
        self.body_bytes += size;
        while self.bodies.len() > MAX_BODIES || self.body_bytes > MAX_BODY_BYTES {
            let Some((_, _, size)) = self.bodies.pop_front() else {
                break;
            };
            self.body_bytes -= size;
        }
    }

    pub(super) fn body(&self, id: u64) -> Option<&Value> {
        self.bodies
            .iter()
            .rev()
            .find(|(kept, ..)| *kept == id)
            .map(|(_, report, _)| report)
    }
}
