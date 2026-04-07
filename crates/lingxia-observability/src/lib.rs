use lingxia_provider::{BoxFuture, ProviderError};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::io;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// Default live subscriber capacity for the in-memory log pipeline.
pub const DEFAULT_LOG_LIVE_CAPACITY: usize = 1024;
/// Default recent history capacity retained in memory.
pub const DEFAULT_LOG_HISTORY_CAPACITY: usize = 2048;
/// Recommended recent replay window for devtools log viewers.
pub const DEFAULT_DEVTOOLS_RECENT_LIMIT: usize = 500;

/// Log levels that match Android/iOS common levels.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogTag {
    Native,
    WebViewConsole,
    LxAppServiceConsole,
}

impl LogTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::WebViewConsole => "JSView",
            Self::LxAppServiceConsole => "JSService",
        }
    }
}

/// Structured log message forwarded to system loggers and network sinks.
#[derive(Debug, Clone, Serialize)]
pub struct LogMessage {
    pub timestamp_ms: u64,
    pub tag: LogTag,
    pub level: LogLevel,
    pub appid: Option<String>,
    pub path: Option<String>,
    pub target: Option<String>,
    pub message: String,
}

impl Default for LogMessage {
    fn default() -> Self {
        Self {
            timestamp_ms: 0,
            tag: LogTag::Native,
            level: LogLevel::Info,
            appid: None,
            path: None,
            target: None,
            message: String::new(),
        }
    }
}

impl LogMessage {
    pub fn new(tag: LogTag, message: impl Into<String>) -> Self {
        Self {
            timestamp_ms: now_timestamp_ms(),
            tag,
            level: LogLevel::Info,
            appid: None,
            path: None,
            target: None,
            message: message.into(),
        }
    }

    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    pub fn with_appid(mut self, appid: impl Into<String>) -> Self {
        self.appid = normalize_optional_string(Some(appid.into()));
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = normalize_optional_string(Some(path.into()));
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = normalize_optional_string(Some(target.into()));
        self
    }
}

/// Compressed in-memory log archive payload.
#[derive(Debug, Clone)]
pub struct CollectedLogArchive {
    pub file_name: String,
    pub content_type: &'static str,
    pub encoding: &'static str,
    pub entry_count: usize,
    pub lxapp_ids: Vec<String>,
    pub bytes: Vec<u8>,
}

/// Metadata returned after a collected log archive has been uploaded.
#[derive(Debug, Clone)]
pub struct CollectedLogArchiveInfo {
    pub file_name: String,
    pub content_type: &'static str,
    pub encoding: &'static str,
    pub entry_count: usize,
    pub lxapp_ids: Vec<String>,
}

impl CollectedLogArchive {
    pub fn from_entries(entries: &[LogMessage]) -> io::Result<Self> {
        let mut lxapp_ids = Vec::new();
        let mut seen_lxapp_ids = HashSet::new();
        let mut jsonl = Vec::new();
        for entry in entries {
            if let Some(appid) = entry.appid.as_deref()
                && seen_lxapp_ids.insert(appid.to_string())
            {
                lxapp_ids.push(appid.to_string());
            }
            serde_json::to_writer(&mut jsonl, entry)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            jsonl.push(b'\n');
        }

        let bytes = zstd::stream::encode_all(io::Cursor::new(jsonl), 3)?;
        Ok(Self {
            file_name: format!("lingxia-logs-{}.jsonl.zst", now_timestamp_ms()),
            content_type: "application/zstd",
            encoding: "jsonl+zstd",
            entry_count: entries.len(),
            lxapp_ids,
            bytes,
        })
    }

    pub fn info(&self) -> CollectedLogArchiveInfo {
        CollectedLogArchiveInfo {
            file_name: self.file_name.clone(),
            content_type: self.content_type,
            encoding: self.encoding,
            entry_count: self.entry_count,
            lxapp_ids: self.lxapp_ids.clone(),
        }
    }
}

/// Combined recent replay plus live log receiver for devtools and diagnostics.
pub struct AttachedLogStream {
    pub recent: Vec<LogMessage>,
    pub receiver: broadcast::Receiver<LogMessage>,
}

impl AttachedLogStream {
    /// Borrow the stitched replay window returned when the stream was attached.
    pub fn recent(&self) -> &[LogMessage] {
        &self.recent
    }

    /// Consume the stream and return `(recent, receiver)` for custom integrations.
    pub fn into_parts(self) -> (Vec<LogMessage>, broadcast::Receiver<LogMessage>) {
        (self.recent, self.receiver)
    }

    /// Receive the next live log item.
    pub async fn recv(&mut self) -> Result<LogMessage, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    /// Try to receive the next live log item without awaiting.
    pub fn try_recv(&mut self) -> Result<LogMessage, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogBufferConfig {
    pub live_capacity: usize,
    pub history_capacity: usize,
}

impl Default for LogBufferConfig {
    fn default() -> Self {
        Self {
            live_capacity: DEFAULT_LOG_LIVE_CAPACITY,
            history_capacity: DEFAULT_LOG_HISTORY_CAPACITY,
        }
    }
}

/// Reusable in-memory log pipeline shared by SDK runtimes and tooling bridges.
pub struct LogBuffer {
    sender: broadcast::Sender<LogMessage>,
    history: Mutex<VecDeque<LogMessage>>,
    config: LogBufferConfig,
}

impl LogBuffer {
    pub fn new(config: LogBufferConfig) -> Self {
        let (sender, _) = broadcast::channel(config.live_capacity.max(1));
        Self {
            sender,
            history: Mutex::new(VecDeque::with_capacity(config.history_capacity.max(1))),
            config,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogMessage> {
        self.sender.subscribe()
    }

    pub fn attach(&self, recent_limit: usize) -> AttachedLogStream {
        let history = self
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.sender.subscribe();
        let recent_limit = clamp_recent_limit(recent_limit, history.len());
        let recent = history
            .iter()
            .skip(history.len().saturating_sub(recent_limit))
            .cloned()
            .collect();
        AttachedLogStream { recent, receiver }
    }

    pub fn snapshot_recent(&self, limit: usize) -> Vec<LogMessage> {
        let history = self
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let limit = clamp_recent_limit(limit, history.len());
        history
            .iter()
            .skip(history.len().saturating_sub(limit))
            .cloned()
            .collect()
    }

    pub fn collect_archive(&self, limit: usize) -> io::Result<CollectedLogArchive> {
        let entries = self.snapshot_recent(limit);
        CollectedLogArchive::from_entries(&entries)
    }

    pub fn push(&self, message: LogMessage) {
        let entry = message.clone();
        {
            let mut history = self
                .history
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if history.len() >= self.config.history_capacity.max(1) {
                history.pop_front();
            }
            history.push_back(entry);
        }

        let _ = self.sender.send(message);
    }
}

fn clamp_recent_limit(requested: usize, available: usize) -> usize {
    if requested == 0 {
        available
    } else {
        requested.min(available)
    }
}

pub fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Realtime plus diagnostic log upload contract.
///
/// # Re-entrancy
///
/// `on_log` is called synchronously inside the log dispatch path.
/// Implementations **must not** emit lingxia log events (e.g. via `info!()` or `tracing`)
/// from within `on_log`, as this would re-enter the log pipeline.  The SDK guards against
/// same-thread re-entrancy, but cross-thread re-entrancy is not detected and may cause
/// unbounded recursion on multi-threaded runtimes.
pub trait LogProvider: Send + Sync + 'static {
    /// Realtime log hook.
    ///
    /// Called synchronously for every structured log event that enters the SDK log pipeline.
    /// Implementations are expected to enqueue quickly and avoid blocking I/O.
    /// **Must not** emit lingxia log events — see trait-level re-entrancy note.
    fn on_log(&self, _message: &LogMessage) {}

    /// Upload a collected compressed log archive for diagnostics.
    fn upload_collected_logs<'a>(
        &'a self,
        _archive: CollectedLogArchive,
    ) -> BoxFuture<'a, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}
