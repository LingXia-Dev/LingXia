//! GPU-initialization crash sentinel. A vendor shader compiler that segfaults
//! cannot be caught, so the first pipeline compilation of a process is
//! recorded in a persistent store and cleared once a GPU frame reaches the
//! screen. A launch that finds the record left by a dead process renders that
//! adapter on the CPU. Platforms supply only the store.
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

/// Persisted as lines: `probation <adapter>` at most once, then `blocked <adapter>`.
#[derive(Default)]
struct Record {
    probation: Option<String>,
    blocked: Vec<String>,
}

impl Record {
    fn parse(text: &str) -> Self {
        let mut record = Self::default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("probation", adapter)) => record.probation = Some(adapter.into()),
                Some(("blocked", adapter)) => record.blocked.push(adapter.into()),
                _ => {}
            }
        }
        record
    }

    fn encode(&self) -> String {
        let mut text = String::new();
        if let Some(adapter) = &self.probation {
            text.push_str(&format!("probation {adapter}\n"));
        }
        for adapter in &self.blocked {
            text.push_str(&format!("blocked {adapter}\n"));
        }
        text
    }
}

#[derive(Default)]
struct State {
    record: Record,
    /// Adapters that presented a GPU frame in this process.
    proven: HashSet<String>,
    /// A previous process died during GPU initialization.
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
        let mut record = Record::parse(&store.read().unwrap_or_default());
        // A record still on probation means its process died before presenting.
        let crashed = record.probation.take();
        let inherited = crashed.is_some();
        if let Some(adapter) = crashed
            && !record.blocked.contains(&adapter)
        {
            record.blocked.push(adapter);
        }
        if inherited {
            let _ = store.write(&record.encode());
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

    /// A GPU frame from `adapter` reached the screen.
    pub fn presented(&self, adapter: &str) {
        let mut state = self.state();
        state.proven.insert(adapter.into());
        self.end(&mut state, adapter);
    }

    /// The adapter's GPU attach failed without crashing; release its probation.
    pub fn released(&self, adapter: &str) {
        let mut state = self.state();
        self.end(&mut state, adapter);
    }

    fn end(&self, state: &mut State, adapter: &str) {
        if state.record.probation.as_deref() != Some(adapter) {
            return;
        }
        state.record.probation = None;
        if state.record.blocked.is_empty() {
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
