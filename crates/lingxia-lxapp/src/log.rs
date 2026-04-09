pub use lingxia_observability::{
    AttachedLogStream, CollectedLogArchive, CollectedLogArchiveInfo, LogLevel, LogMessage, LogTag,
};
use lingxia_observability::{
    DEFAULT_DEVTOOLS_RECENT_LIMIT, LogBuffer, LogBufferConfig, normalize_optional_string,
};
use std::cell::Cell;
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing_subscriber::{Registry, layer::Layer, prelude::*};

thread_local! {
    static LOG_DISPATCH_GUARD: Cell<bool> = const { Cell::new(false) };
}

static GLOBAL_LOG_MANAGER: OnceLock<Arc<LogManager>> = OnceLock::new();
static TRACING_SUBSCRIBER_READY: OnceLock<()> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum LogStreamError {
    #[error("log manager is not initialized")]
    NotInitialized,
}

/// Global logger manager.
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

    /// Atomically attach a devtool log stream with a recent replay window.
    ///
    /// The returned `recent` snapshot and `receiver` are stitched together under the
    /// history lock so callers do not see gaps between the replay window and live events.
    pub fn attach(&self, recent_limit: usize) -> AttachedLogStream {
        self.buffer.attach(recent_limit)
    }

    /// Attach a devtool log stream with the SDK's recommended replay window.
    pub fn attach_for_devtools(&self) -> AttachedLogStream {
        self.attach(DEFAULT_DEVTOOLS_RECENT_LIMIT)
    }

    /// Print a log message to the native logger.
    pub fn print_to_native(&self, message: &LogMessage) {
        (self.logger)(message);
    }

    /// Snapshot recent logs from the in-memory ring buffer.
    pub fn snapshot_recent(&self, limit: usize) -> Vec<LogMessage> {
        self.buffer.snapshot_recent(limit)
    }

    /// Build a compressed JSONL archive of recent logs.
    pub fn collect_archive(&self, limit: usize) -> std::io::Result<CollectedLogArchive> {
        self.buffer.collect_archive(limit)
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
        crate::provider::get_log_provider().on_log(&message);
        (self.logger)(&message);
    }
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

/// Attach a devtool-friendly log stream.
///
/// This returns a bounded recent replay plus a live receiver so callers can render
/// current logs immediately and then continue tailing new entries.
/// Pass `0` to replay the entire in-memory history window.
pub fn attach_log_stream(recent_limit: usize) -> Result<AttachedLogStream, LogStreamError> {
    let manager = LogManager::get().ok_or(LogStreamError::NotInitialized)?;
    Ok(manager.attach(recent_limit))
}

/// Attach a devtool log stream using the SDK's recommended replay window.
pub fn attach_log_stream_default() -> Result<AttachedLogStream, LogStreamError> {
    let manager = LogManager::get().ok_or(LogStreamError::NotInitialized)?;
    Ok(manager.attach_for_devtools())
}

/// Global logging function for scenarios without appid/path context.
pub fn log(tag: LogTag, level: LogLevel, message: impl std::fmt::Display) {
    let mut log_message = new_log_message(tag, message);
    log_message.level = level;
    emit_log_message(log_message);
}

/// Upload a recent compressed log archive through the registered provider.
///
/// This is the diagnostic path for "collect log". It snapshots the recent in-memory
/// log ring buffer, encodes it as `jsonl.zst`, and delegates the network upload to
/// the active `LogProvider`.
pub async fn upload_collected_logs(
    limit: usize,
) -> Result<CollectedLogArchiveInfo, crate::provider::ProviderError> {
    let manager = LogManager::get().ok_or_else(|| {
        crate::provider::ProviderError::internal("log manager is not initialized")
    })?;
    let archive = manager.collect_archive(limit).map_err(|err| {
        crate::provider::ProviderError::internal(format!("collect logs failed: {err}"))
    })?;
    let metadata = archive.info();
    crate::provider::get_log_provider()
        .upload_collected_logs(archive)
        .await?;
    Ok(metadata)
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Warn)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Error)
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Debug)
    };
}

#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Verbose)
    };
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
                target: "lingxia.lxapp",
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
        "JSView" => Some(LogTag::WebViewConsole),
        "JSService" => Some(LogTag::LxAppServiceConsole),
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
            timestamp_ms: lingxia_observability::now_timestamp_ms(),
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
