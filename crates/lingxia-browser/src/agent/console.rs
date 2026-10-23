//! Per-tab console capture: what the page logged and threw, collected from
//! the page's own recorder whenever the agent runs, and kept across
//! navigations in a bounded ring. It is page data, never the trusted log.

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 500;
const MAX_TEXT_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConsoleEntry {
    /// Increases per tab; pass the last one seen as `since` to read only new
    /// entries.
    pub seq: u64,
    pub level: String,
    pub text: String,
    /// The page that logged it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConsoleList {
    pub entries: Vec<ConsoleEntry>,
    /// Entries pushed out of the ring before they were read.
    pub dropped: u64,
    pub next_seq: u64,
}

#[derive(Default)]
struct TabConsole {
    entries: VecDeque<ConsoleEntry>,
    next_seq: u64,
    dropped: u64,
    /// The page recorder's own drop count, per document, to count only new
    /// losses.
    page_dropped: Option<(String, u64)>,
}

fn key(tab_id: &str) -> String {
    crate::tabs::normalize_runtime_tab_id(tab_id).unwrap_or_else(|| tab_id.to_string())
}

static CONSOLES: LazyLock<Mutex<HashMap<String, TabConsole>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Entries a document's recorder had not handed over before, and that
/// recorder's running count of entries it pushed out unread.
pub(crate) fn record_page(
    tab_id: &str,
    generation: &str,
    entries: &[serde_json::Value],
    url: Option<&str>,
    page_dropped: u64,
) {
    let mut consoles = CONSOLES.lock().unwrap_or_else(|e| e.into_inner());
    let console = consoles.entry(key(tab_id)).or_default();
    let previous = match &console.page_dropped {
        Some((g, n)) if g == generation => *n,
        _ => 0,
    };
    console.dropped += page_dropped.saturating_sub(previous);
    console.page_dropped = Some((generation.to_string(), page_dropped));
    for entry in entries {
        let level = entry
            .get("level")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("log");
        let text = entry
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let timestamp_ms = entry
            .get("timestamp_ms")
            .and_then(serde_json::Value::as_u64);
        push(console, level, text, url.map(str::to_string), timestamp_ms);
    }
}

#[cfg(test)]
fn record(tab_id: &str, level: &str, text: &str, url: Option<String>) {
    let mut consoles = CONSOLES.lock().unwrap_or_else(|e| e.into_inner());
    let console = consoles.entry(key(tab_id)).or_default();
    push(console, level, text, url, None);
}

fn push(
    console: &mut TabConsole,
    level: &str,
    text: &str,
    url: Option<String>,
    timestamp_ms: Option<u64>,
) {
    let mut text = text.to_string();
    if text.chars().count() > MAX_TEXT_CHARS {
        text = text.chars().take(MAX_TEXT_CHARS).collect::<String>() + "…";
    }
    let timestamp_ms = timestamp_ms.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    });
    console.next_seq += 1;
    let seq = console.next_seq;
    console.entries.push_back(ConsoleEntry {
        seq,
        level: level.to_string(),
        text,
        url,
        timestamp_ms,
    });
    if console.entries.len() > MAX_ENTRIES {
        console.entries.pop_front();
        console.dropped += 1;
    }
}

/// Entries after `since` (all when `None`), optionally only `levels`.
pub fn list(tab_id: &str, since: Option<u64>, levels: &[String]) -> ConsoleList {
    let consoles = CONSOLES.lock().unwrap_or_else(|e| e.into_inner());
    let Some(console) = consoles.get(&key(tab_id)) else {
        return ConsoleList {
            entries: Vec::new(),
            dropped: 0,
            next_seq: 0,
        };
    };
    let entries = console
        .entries
        .iter()
        .filter(|e| since.is_none_or(|s| e.seq > s))
        .filter(|e| levels.is_empty() || levels.iter().any(|l| l == &e.level))
        .cloned()
        .collect();
    ConsoleList {
        entries,
        dropped: console.dropped,
        next_seq: console.next_seq,
    }
}

pub fn clear(tab_id: &str) {
    if let Some(console) = CONSOLES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(tab_id))
    {
        console.entries.clear();
        // `page_dropped` stays: later page drops count from here.
        console.dropped = 0;
    }
}

pub(crate) fn forget_tab(tab_id: &str) {
    CONSOLES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&key(tab_id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_the_newest_and_counts_drops() {
        let tab = "console-test-ring";
        for i in 0..(MAX_ENTRIES + 3) {
            record(tab, "log", &format!("m{i}"), None);
        }
        let all = list(tab, None, &[]);
        assert_eq!(all.entries.len(), MAX_ENTRIES);
        assert_eq!(all.dropped, 3);
        assert_eq!(
            all.entries.last().unwrap().text,
            format!("m{}", MAX_ENTRIES + 2)
        );
        let newer = list(tab, Some(all.next_seq - 1), &[]);
        assert_eq!(newer.entries.len(), 1);
        forget_tab(tab);
    }

    #[test]
    fn page_drops_count_once_per_document() {
        let tab = "console-test-page";
        let entry = serde_json::json!({ "level": "warn", "text": "w", "timestamp_ms": 5 });
        record_page(
            tab,
            "g1",
            std::slice::from_ref(&entry),
            Some("https://a/"),
            2,
        );
        record_page(tab, "g1", &[], None, 3);
        record_page(tab, "g2", &[], None, 1);
        let all = list(tab, None, &[]);
        assert_eq!(all.dropped, 4);
        assert_eq!(all.entries[0].timestamp_ms, 5);
        forget_tab(tab);
    }

    #[test]
    fn levels_filter_and_clear() {
        let tab = "console-test-levels";
        record(tab, "error", "bad", Some("https://a/".into()));
        record(tab, "log", "fine", None);
        let errors = list(tab, None, &["error".to_string()]);
        assert_eq!(errors.entries.len(), 1);
        assert_eq!(errors.entries[0].url.as_deref(), Some("https://a/"));
        clear(tab);
        assert!(list(tab, None, &[]).entries.is_empty());
        forget_tab(tab);
    }
}
