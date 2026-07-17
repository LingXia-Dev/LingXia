use lingxia_provider::{BoxFuture, ProviderError};
use serde::Serialize;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing_subscriber::{Registry, layer::Layer, prelude::*};

/// Default live subscriber capacity for the in-memory log pipeline.
pub const DEFAULT_LOG_LIVE_CAPACITY: usize = 1024;
/// Default recent history capacity retained in memory.
pub const DEFAULT_LOG_HISTORY_CAPACITY: usize = 2048;
/// Default recent replay window used by SDK/devtool consumers.
pub const DEFAULT_LOG_STREAM_RECENT_LIMIT: usize = 500;

thread_local! {
    static LOG_DISPATCH_GUARD: Cell<bool> = const { Cell::new(false) };
}

static GLOBAL_LOG_MANAGER: OnceLock<Arc<LogManager>> = OnceLock::new();
static TRACING_SUBSCRIBER_READY: OnceLock<()> = OnceLock::new();
static LOG_PROVIDER: OnceLock<Box<dyn LogProvider>> = OnceLock::new();
static NO_OP_LOG_PROVIDER: NoOpLogProvider = NoOpLogProvider;

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
    BrowserConsole,
}

impl LogTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::WebViewConsole => "LXView",
            Self::LxAppServiceConsole => "LXLogic",
            Self::BrowserConsole => "Browser",
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

/// Combined recent replay plus live log receiver for diagnostics consumers.
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

/// Reusable in-memory log pipeline shared by SDK runtimes and diagnostics tooling.
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

    pub fn push(&self, message: LogMessage) {
        // Separate the two concerns: the live broadcast (what `lxdev logs`
        // subscribes to) gets every record at the console level, but only warn+
        // is retained in bounded recent history. Routine info/debug stay visible
        // live without evicting the diagnostics that replay exists to preserve.
        if matches!(message.level, LogLevel::Warn | LogLevel::Error) {
            let mut history = self
                .history
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if history.len() >= self.config.history_capacity.max(1) {
                history.pop_front();
            }
            history.push_back(message.clone());
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

/// Realtime forwarding plus diagnostic collection contract.
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

    /// Trigger provider-owned diagnostic log collection.
    ///
    /// The provider owns retention, record selection, encoding, and transport.
    fn collect_logs(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}

struct NoOpLogProvider;

impl LogProvider for NoOpLogProvider {}

#[derive(Debug, thiserror::Error)]
pub enum LogStreamError {
    #[error("log manager is not initialized")]
    NotInitialized,
}

/// Global structured log manager.
///
/// The manager owns the in-memory history/live stream, forwards every accepted
/// entry to the registered `LogProvider`, and finally mirrors the entry to the
/// native platform logger supplied by the host crate.
pub struct LogManager {
    buffer: LogBuffer,
    logger: Box<dyn Fn(&LogMessage) + Send + Sync>,
}

pub struct LogTracingLayer;

struct DispatchGuardReset;

impl Drop for DispatchGuardReset {
    fn drop(&mut self) {
        LOG_DISPATCH_GUARD.with(|guard| guard.set(false));
    }
}

impl LogManager {
    /// Initialize the global logger instance.
    pub fn init<F>(logger: F) -> Arc<Self>
    where
        F: Fn(&LogMessage) + Send + Sync + 'static,
    {
        let manager = GLOBAL_LOG_MANAGER
            .get_or_init(|| {
                Arc::new(LogManager {
                    buffer: LogBuffer::new(LogBufferConfig::default()),
                    logger: Box::new(logger),
                })
            })
            .clone();

        // The tracing layer is part of the log manager contract because JS/appservice
        // console output is emitted through tracing events rather than the Rust `log` facade.
        init_tracing();

        manager
    }

    /// Gets global log manager instance if initialized.
    pub fn get() -> Option<Arc<Self>> {
        GLOBAL_LOG_MANAGER.get().cloned()
    }

    /// Subscribe to the live log stream.
    pub fn subscribe(&self) -> broadcast::Receiver<LogMessage> {
        self.buffer.subscribe()
    }

    /// Atomically attach a log stream with a recent replay window.
    ///
    /// The returned `recent` snapshot and `receiver` are stitched together under the
    /// history lock so callers do not see gaps between the replay window and live events.
    pub fn attach(&self, recent_limit: usize) -> AttachedLogStream {
        self.buffer.attach(recent_limit)
    }

    /// Attach a log stream with the SDK's default replay window.
    pub fn attach_default(&self) -> AttachedLogStream {
        self.attach(DEFAULT_LOG_STREAM_RECENT_LIMIT)
    }

    /// Print a log message to the native logger.
    pub fn print_to_native(&self, message: &LogMessage) {
        (self.logger)(message);
    }

    /// Snapshot recent logs from the in-memory ring buffer.
    pub fn snapshot_recent(&self, limit: usize) -> Vec<LogMessage> {
        self.buffer.snapshot_recent(limit)
    }

    fn dispatch(&self, message: LogMessage) {
        let should_dispatch = LOG_DISPATCH_GUARD.with(|guard| {
            if guard.get() {
                false
            } else {
                guard.set(true);
                true
            }
        });

        if !should_dispatch {
            return;
        }

        let _reset_guard = DispatchGuardReset;
        self.buffer.push(message.clone());
        get_log_provider().on_log(&message);
        (self.logger)(&message);
    }
}

/// Register an optional log provider. Must be called at app startup before SDK initialization.
pub fn register_log_provider(provider: Box<dyn LogProvider>) {
    if LOG_PROVIDER.set(provider).is_err() {
        panic!("register_log_provider called more than once");
    }
}

fn get_log_provider() -> &'static dyn LogProvider {
    LOG_PROVIDER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&NO_OP_LOG_PROVIDER)
}

/// Install the global tracing subscriber that forwards tracing events into `LogManager`.
fn init_tracing() {
    if TRACING_SUBSCRIBER_READY.get().is_some() {
        return;
    }

    let subscriber = Registry::default().with(tracing_layer());
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        let _ = TRACING_SUBSCRIBER_READY.set(());
    }
}

pub fn tracing_layer() -> LogTracingLayer {
    LogTracingLayer
}

/// Attach a log stream with a recent replay window.
///
/// This returns a bounded recent replay plus a live receiver so callers can render
/// current logs immediately and then continue tailing new entries.
/// Pass `0` to replay the entire in-memory history window.
pub fn attach_log_stream(recent_limit: usize) -> Result<AttachedLogStream, LogStreamError> {
    let manager = LogManager::get().ok_or(LogStreamError::NotInitialized)?;
    Ok(manager.attach(recent_limit))
}

/// Attach a log stream using the SDK's default replay window.
pub fn attach_log_stream_default() -> Result<AttachedLogStream, LogStreamError> {
    let manager = LogManager::get().ok_or(LogStreamError::NotInitialized)?;
    Ok(manager.attach_default())
}

/// Global logging function for scenarios without appid/path context.
pub fn log(tag: LogTag, level: LogLevel, message: impl std::fmt::Display) {
    let mut log_message = new_log_message(tag, message);
    log_message.level = level;
    emit_log_message(log_message);
}

/// Trigger diagnostic log collection through the registered provider.
///
/// LingXia does not prescribe a retention policy or wire format. The active
/// provider selects records from its own store and owns the export lifecycle.
pub async fn collect_logs() -> Result<(), ProviderError> {
    get_log_provider().collect_logs().await
}

/// Log builder that automatically emits on drop.
pub struct LogBuilder {
    message: LogMessage,
}

impl LogBuilder {
    pub fn new(tag: LogTag, message: impl std::fmt::Display) -> Self {
        Self {
            message: new_log_message(tag, message),
        }
    }

    pub fn with_appid(mut self, appid: impl Into<String>) -> Self {
        self.message.appid = normalize_optional_string(Some(appid.into()));
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.message.path = normalize_optional_string(Some(path.into()));
        self
    }

    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.message.level = level;
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.message.target = normalize_optional_string(Some(target.into()));
        self
    }
}

impl Drop for LogBuilder {
    fn drop(&mut self) {
        emit_log_message(std::mem::take(&mut self.message));
    }
}

fn emit_log_message(message: LogMessage) {
    emit_tracing_event(&message);

    if let Some(manager) = GLOBAL_LOG_MANAGER.get() {
        manager.dispatch(message);
    }
}

fn emit_tracing_event(message: &LogMessage) {
    let appid = message.appid.as_deref().unwrap_or("");
    let path = message.path.as_deref().unwrap_or("");
    let target = message.target.as_deref().unwrap_or("");
    let log_tag = message.tag.as_str();

    macro_rules! emit {
        ($level:expr) => {
            tracing::event!(
                target: "lingxia.log",
                $level,
                lx_emitted = true,
                log_tag,
                appid,
                path,
                target,
                message = %message.message
            );
        };
    }

    match message.level {
        LogLevel::Verbose => {
            emit!(tracing::Level::TRACE);
        }
        LogLevel::Debug => {
            emit!(tracing::Level::DEBUG);
        }
        LogLevel::Info => {
            emit!(tracing::Level::INFO);
        }
        LogLevel::Warn => {
            emit!(tracing::Level::WARN);
        }
        LogLevel::Error => {
            emit!(tracing::Level::ERROR);
        }
    }
}

fn log_level_from_tracing_level(level: &tracing::Level) -> LogLevel {
    match *level {
        tracing::Level::ERROR => LogLevel::Error,
        tracing::Level::WARN => LogLevel::Warn,
        tracing::Level::INFO => LogLevel::Info,
        tracing::Level::DEBUG => LogLevel::Debug,
        tracing::Level::TRACE => LogLevel::Verbose,
    }
}

fn new_log_message(tag: LogTag, message: impl std::fmt::Display) -> LogMessage {
    LogMessage::new(tag, message.to_string())
}

fn log_tag_from_str(value: &str) -> Option<LogTag> {
    match value {
        "Native" => Some(LogTag::Native),
        "LXView" => Some(LogTag::WebViewConsole),
        "LXLogic" => Some(LogTag::LxAppServiceConsole),
        "Browser" => Some(LogTag::BrowserConsole),
        _ => None,
    }
}

#[derive(Default)]
struct TracingEventVisitor {
    message: Option<String>,
    appid: Option<String>,
    path: Option<String>,
    target_field: Option<String>,
    log_tag: Option<String>,
    namespace: Option<String>,
    scope: Option<String>,
    lx_emitted: Option<String>,
}

impl TracingEventVisitor {
    fn record_value(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = Some(value),
            "appid" => self.appid = Some(value),
            "path" => self.path = Some(value),
            "target" => self.target_field = Some(value),
            "log_tag" => self.log_tag = Some(value),
            "namespace" => self.namespace = Some(value),
            "scope" => self.scope = Some(value),
            "lx_emitted" => self.lx_emitted = Some(value),
            _ => {}
        }
    }
}

impl Visit for TracingEventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, value.to_string());
    }
}

impl<S> Layer<S> for LogTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(manager) = LogManager::get() else {
            return;
        };

        let metadata = event.metadata();
        let mut visitor = TracingEventVisitor::default();
        event.record(&mut visitor);

        if visitor.lx_emitted.as_deref() == Some("true") {
            return;
        }

        let tag = if metadata.target() == "rong.js.console" {
            match visitor.scope.as_deref() {
                Some("appservice") => LogTag::LxAppServiceConsole,
                _ => LogTag::Native,
            }
        } else {
            visitor
                .log_tag
                .as_deref()
                .and_then(log_tag_from_str)
                .unwrap_or(LogTag::Native)
        };

        let target = if metadata.target() == "rong.js.console" {
            visitor.target_field
        } else {
            visitor
                .target_field
                .or_else(|| Some(metadata.target().to_string()))
        };

        let message = LogMessage {
            timestamp_ms: now_timestamp_ms(),
            tag,
            level: log_level_from_tracing_level(metadata.level()),
            appid: normalize_optional_string(visitor.appid.or(visitor.namespace)),
            path: normalize_optional_string(visitor.path),
            target: normalize_optional_string(target),
            message: visitor
                .message
                .unwrap_or_else(|| metadata.name().to_string()),
        };

        manager.dispatch(message);
    }
}
