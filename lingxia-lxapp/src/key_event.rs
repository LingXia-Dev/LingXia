use dashmap::DashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
enum KeyKind {
    Down,
    Up,
}

#[derive(Debug, Default)]
struct ListenerCounts {
    key_down: AtomicUsize,
    key_up: AtomicUsize,
}

impl ListenerCounts {
    /// Returns (target, other) counters for the given key kind.
    fn select(&self, kind: KeyKind) -> (&AtomicUsize, &AtomicUsize) {
        match kind {
            KeyKind::Down => (&self.key_down, &self.key_up),
            KeyKind::Up => (&self.key_up, &self.key_down),
        }
    }
}

type AppSessionKey = (String, u64);

fn registry() -> &'static DashMap<AppSessionKey, ListenerCounts> {
    static REGISTRY: OnceLock<DashMap<AppSessionKey, ListenerCounts>> = OnceLock::new();
    REGISTRY.get_or_init(DashMap::new)
}

fn key(appid: &str, session_id: u64) -> AppSessionKey {
    (appid.to_string(), session_id)
}

fn has(appid: &str, session_id: u64, kind: KeyKind) -> bool {
    registry()
        .get(&key(appid, session_id))
        .is_some_and(|v| v.select(kind).0.load(Ordering::Relaxed) > 0)
}

fn inc(appid: &str, session_id: u64, kind: KeyKind) {
    let counts = registry().entry(key(appid, session_id)).or_default();
    counts.select(kind).0.fetch_add(1, Ordering::Relaxed);
}

fn set(appid: &str, session_id: u64, kind: KeyKind, count: usize) {
    if count == 0 {
        let k = key(appid, session_id);
        let mut should_remove = false;
        if let Some(v) = registry().get_mut(&k) {
            let (target, other) = v.select(kind);
            target.store(0, Ordering::Relaxed);
            should_remove = other.load(Ordering::Relaxed) == 0;
        }
        if should_remove {
            registry().remove(&k);
        }
        return;
    }
    let counts = registry().entry(key(appid, session_id)).or_default();
    counts.select(kind).0.store(count, Ordering::Relaxed);
}

pub fn has_key_down(appid: &str, session_id: u64) -> bool {
    has(appid, session_id, KeyKind::Down)
}

pub fn has_key_up(appid: &str, session_id: u64) -> bool {
    has(appid, session_id, KeyKind::Up)
}

pub fn inc_key_down(appid: &str, session_id: u64) {
    inc(appid, session_id, KeyKind::Down)
}

pub fn inc_key_up(appid: &str, session_id: u64) {
    inc(appid, session_id, KeyKind::Up)
}

pub fn set_key_down(appid: &str, session_id: u64, count: usize) {
    set(appid, session_id, KeyKind::Down, count)
}

pub fn set_key_up(appid: &str, session_id: u64, count: usize) {
    set(appid, session_id, KeyKind::Up, count)
}

pub fn clear(appid: &str, session_id: u64) {
    registry().remove(&key(appid, session_id));
}
