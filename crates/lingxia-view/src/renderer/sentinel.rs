//! GPU-initialization crash sentinel. A vendor shader compiler that segfaults
//! cannot be caught, so the first pipeline compilation of a process is
//! recorded in a persistent store and cleared once a GPU frame reaches the
//! screen. A probation left by a dead process is a strike; a second strike in
//! a row for the same adapter within a day blocks it, so one kill during a
//! cold start does not cost the GPU route. Platforms supply only the store.
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Persists one record across processes, e.g. a file in app-private storage.
pub trait SentinelStore: Send + Sync {
    fn read(&self) -> Option<String>;
    fn write(&self, record: &str) -> Result<(), String>;
    fn clear(&self);
}

/// Why a page renders on the CPU, if it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// A previous process died while this adapter's driver compiled our pipelines.
    Driver,
}

/// A strike expires after a day: two kills far apart are not a crashing driver.
const STRIKE_TTL_SECS: u64 = 24 * 60 * 60;

/// Persisted as lines: `probation <adapter>` at most once, `strike <unix secs> <adapter>`,
/// then `blocked <adapter>`.
#[derive(Default)]
struct Record {
    probation: Option<String>,
    /// Probation of a process stopped normally before its first present.
    paused: bool,
    strikes: Vec<(u64, String)>,
    blocked: Vec<String>,
}

impl Record {
    fn parse(text: &str) -> Self {
        let mut record = Self::default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("probation", adapter)) => record.probation = Some(adapter.into()),
                Some(("paused", adapter)) => {
                    record.probation = Some(adapter.into());
                    record.paused = true;
                }
                Some(("blocked", adapter)) => record.blocked.push(adapter.into()),
                Some(("strike", rest)) => {
                    if let Some((at, adapter)) = rest.split_once(' ')
                        && let Ok(at) = at.parse()
                    {
                        record.strikes.push((at, adapter.into()));
                    }
                }
                _ => {}
            }
        }
        record
    }

    fn encode(&self) -> String {
        let mut text = String::new();
        if let Some(adapter) = &self.probation {
            let state = if self.paused { "paused" } else { "probation" };
            text.push_str(&format!("{state} {adapter}\n"));
        }
        for (at, adapter) in &self.strikes {
            text.push_str(&format!("strike {at} {adapter}\n"));
        }
        for adapter in &self.blocked {
            text.push_str(&format!("blocked {adapter}\n"));
        }
        text
    }

    fn is_empty(&self) -> bool {
        self.probation.is_none() && self.strikes.is_empty() && self.blocked.is_empty()
    }
}

#[derive(Default)]
struct State {
    record: Record,
    /// Adapters that presented a GPU frame in this process.
    proven: HashSet<String>,
    /// A previous process died during GPU initialization twice in a row.
    inherited: bool,
    /// A known-crashing driver was refused in this process.
    refused: bool,
}

pub struct Sentinel {
    store: Box<dyn SentinelStore>,
    state: Mutex<State>,
}

impl Sentinel {
    pub fn new(store: Box<dyn SentinelStore>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::at(store, now)
    }

    /// `new` with the launch time given, in unix seconds.
    pub fn at(store: Box<dyn SentinelStore>, now: u64) -> Self {
        let read = store.read().unwrap_or_default();
        let mut record = Record::parse(&read);
        record
            .strikes
            .retain(|(at, _)| now.saturating_sub(*at) < STRIKE_TTL_SECS);
        // A record still on probation means its process died before presenting.
        let mut inherited = false;
        let paused = std::mem::take(&mut record.paused);
        if let Some(adapter) = record.probation.take().filter(|_| !paused) {
            if record.strikes.iter().any(|(_, struck)| *struck == adapter) {
                record.strikes.retain(|(_, struck)| *struck != adapter);
                if !record.blocked.contains(&adapter) {
                    record.blocked.push(adapter);
                }
                inherited = true;
            } else {
                record.strikes.push((now, adapter));
            }
        }
        if record.encode() != read {
            if record.is_empty() {
                store.clear();
            } else {
                let _ = store.write(&record.encode());
            }
        }
        Self {
            store,
            state: Mutex::new(State {
                record,
                proven: HashSet::new(),
                inherited,
                refused: false,
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Before compiling pipelines on `adapter`; false when it must not be used.
    pub fn arm(&self, adapter: &str) -> bool {
        let adapter = &one_line(adapter);
        let mut state = self.state();
        if known_crashing(adapter) {
            state.refused = true;
            return false;
        }
        if state.proven.contains(adapter) || state.record.probation.as_deref() == Some(adapter) {
            return true;
        }
        if state
            .record
            .blocked
            .iter()
            .any(|blocked| blocked == adapter)
        {
            return false;
        }
        // One adapter at a time; a second waits for the first to prove itself.
        if state.record.probation.is_some() {
            return false;
        }
        state.record.probation = Some(adapter.into());
        if self.store.write(&state.record.encode()).is_err() {
            // Without a record a crash would loop; stay off this adapter.
            state.record.probation = None;
            return false;
        }
        true
    }

    /// Whether `adapter` is refused: a known-crashing driver or one blocked on record.
    pub fn blocked(&self, adapter: &str) -> bool {
        let adapter = one_line(adapter);
        known_crashing(&adapter) || self.state().record.blocked.contains(&adapter)
    }

    /// A GPU frame from `adapter` reached the screen; its strikes are forgiven.
    pub fn presented(&self, adapter: &str) {
        let adapter = &one_line(adapter);
        let mut state = self.state();
        if !state.proven.insert(adapter.into()) {
            return;
        }
        state.record.strikes.retain(|(_, struck)| struck != adapter);
        if state.record.probation.as_deref() == Some(adapter.as_str()) {
            self.end(&mut state, adapter);
        } else {
            self.persist(&state);
        }
    }

    /// The platform stopped GPU work on a normal path before a first present: a
    /// process that dies while stopped did not crash in the driver.
    pub fn interrupted(&self) {
        self.set_paused(true);
    }

    /// GPU work resumes; an open probation guards it again.
    pub fn resumed(&self) {
        self.set_paused(false);
    }

    fn set_paused(&self, paused: bool) {
        let mut state = self.state();
        if state.record.probation.is_some() && state.record.paused != paused {
            state.record.paused = paused;
            self.persist(&state);
        }
    }

    /// The adapter's GPU attach failed without crashing; release its probation.
    pub fn released(&self, adapter: &str) {
        let adapter = &one_line(adapter);
        let mut state = self.state();
        self.end(&mut state, adapter);
    }

    fn end(&self, state: &mut State, adapter: &str) {
        if state.record.probation.as_deref() != Some(adapter) {
            return;
        }
        state.record.probation = None;
        state.record.paused = false;
        self.persist(state);
    }

    fn persist(&self, state: &State) {
        if state.record.is_empty() {
            self.store.clear();
        } else {
            let _ = self.store.write(&state.record.encode());
        }
    }

    /// Set when some adapter is blocked, whichever backend now renders.
    pub fn fallback(&self) -> Option<Fallback> {
        let state = self.state();
        (state.inherited || state.refused || !state.record.blocked.is_empty())
            .then_some(Fallback::Driver)
    }
}

/// The record is line-based, and some drivers report multi-line build info (Adreno).
fn one_line(adapter: &str) -> String {
    adapter.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Drivers whose shader compiler crashes on wgpu's bounds-checked shaders.
/// Huawei Maleoon (vendor 0x19e5) Vulkan segfaults in `cmpbe_v2_compile_multiple_shaders`.
fn known_crashing(adapter: &str) -> bool {
    adapter.starts_with("Vulkan|19e5:") && adapter.contains("|Maleoon")
}

static INSTALLED: OnceLock<Sentinel> = OnceLock::new();

/// Install the process sentinel once; later calls keep the first store.
pub fn install(store: Box<dyn SentinelStore>) {
    let _ = INSTALLED.set(Sentinel::new(store));
}

/// Without an installed store only known-crashing drivers are refused.
pub fn arm(adapter: &str) -> bool {
    match INSTALLED.get() {
        Some(sentinel) => sentinel.arm(adapter),
        None => !known_crashing(adapter),
    }
}

/// Whether `adapter` would be refused, without arming anything.
pub fn blocked(adapter: &str) -> bool {
    match INSTALLED.get() {
        Some(sentinel) => sentinel.blocked(adapter),
        None => known_crashing(adapter),
    }
}

pub fn presented(adapter: &str) {
    if let Some(sentinel) = INSTALLED.get() {
        sentinel.presented(adapter);
    }
}

pub fn released(adapter: &str) {
    if let Some(sentinel) = INSTALLED.get() {
        sentinel.released(adapter);
    }
}

pub fn interrupted() {
    if let Some(sentinel) = INSTALLED.get() {
        sentinel.interrupted();
    }
}

pub fn resumed() {
    if let Some(sentinel) = INSTALLED.get() {
        sentinel.resumed();
    }
}

pub fn fallback() -> Option<Fallback> {
    INSTALLED.get().and_then(Sentinel::fallback)
}

/// A store backed by one file.
pub struct FileStore(pub std::path::PathBuf);

impl SentinelStore for FileStore {
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.0).ok()
    }
    fn write(&self, record: &str) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::File::create(&self.0).map_err(|e| e.to_string())?;
        file.write_all(record.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())
    }
    fn clear(&self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
#[path = "tests/sentinel.rs"]
mod tests;
