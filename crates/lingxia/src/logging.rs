#![cfg_attr(target_os = "windows", allow(dead_code))]

use lingxia_log::{LogBuilder, LogLevel as LxLogLevel, LogManager, LogMessage, LogTag};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static LOGGING_INIT: OnceLock<()> = OnceLock::new();
static DOWNSTREAM_LOGGER: OnceLock<Box<dyn Log + Send + Sync>> = OnceLock::new();
static SDK_LOGGER: SdkLogger = SdkLogger;

const SDK_LOG_LEVEL_VERBOSE: i32 = 0;
const SDK_LOG_LEVEL_DEBUG: i32 = 1;
const SDK_LOG_LEVEL_INFO: i32 = 2;
const SDK_LOG_LEVEL_WARN: i32 = 3;
const SDK_LOG_LEVEL_ERROR: i32 = 4;

/// Explicit override for the runtime log threshold, e.g.
/// `LINGXIA_LOG_LEVEL=debug`. When unset, a dev session defaults to `debug`
/// and everything else to `info` (see [`resolve_initial_level`]); when set it
/// pins the level and the dev-session default is skipped.
const LOG_LEVEL_ENV: &str = "LINGXIA_LOG_LEVEL";

/// Single source of truth for the active minimum level, as an SDK level int
/// (0=verbose … 4=error). Both the Rust `log` facade and the host-log path
/// ([`forward_host_log`]) gate on this, so Rust and SDK logs share one policy.
/// Adjustable at runtime via [`set_log_level`] (e.g. the dev server raising it
/// for a session).
static HOST_LOG_LEVEL: AtomicI32 = AtomicI32::new(SDK_LOG_LEVEL_INFO);

/// True when `LINGXIA_LOG_LEVEL` explicitly set the level, so the dev-session
/// default ([`apply_dev_session_level`]) doesn't override an explicit choice.
static LEVEL_PINNED_BY_ENV: AtomicBool = AtomicBool::new(false);

/// Error returned when installing a downstream logger fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownstreamLoggerError {
    /// A downstream logger has already been registered.
    AlreadyRegistered,
}

impl std::fmt::Display for DownstreamLoggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRegistered => write!(f, "downstream logger is already registered"),
        }
    }
}

impl std::error::Error for DownstreamLoggerError {}

pub(crate) fn init() {
    if LOGGING_INIT.get().is_some() {
        return;
    }

    // Seed the shared threshold before any record can flow, so the platform
    // sinks and the facade agree on the level from the very first log. On
    // devices the dev-ws-url lives in app config that isn't loaded yet, so this
    // sees only the env signal here; `apply_dev_session_level` re-checks later.
    let (level, pinned) = resolve_initial_level();
    LEVEL_PINNED_BY_ENV.store(pinned, Ordering::Relaxed);
    HOST_LOG_LEVEL.store(level, Ordering::Relaxed);

    let _ = LogManager::init(|message| {
        platform_logger().write(message);
    });

    if log::set_logger(&SDK_LOGGER).is_ok() {
        log::set_max_level(sdk_level_to_filter(level));
    }

    let _ = LOGGING_INIT.set(());
}

/// Raise or lower the active log threshold at runtime (SDK level int: 0=verbose
/// … 4=error). Updates both the host-log gate and the Rust `log` facade so they
/// stay in lock-step — e.g. the dev server raising it to `debug` for a session.
///
/// Note: the Android/Harmony logcat sink's own cap is fixed when first built
/// (from the env level), so a later raise reaches the dev-server stream and the
/// in-memory buffer but not logcat/hilog.
pub fn set_log_level(level: i32) {
    if map_sdk_level(level).is_none() {
        return;
    }
    HOST_LOG_LEVEL.store(level, Ordering::Relaxed);
    log::set_max_level(sdk_level_to_filter(level));
}

/// Whether a host log at `level` (SDK int) would be recorded at the current
/// threshold. Host wrappers guard hot-path logs with this to skip building a
/// message (and crossing the FFI) that [`forward_host_log`] would only drop.
pub fn host_log_enabled(level: i32) -> bool {
    map_sdk_level(level).is_some() && level >= HOST_LOG_LEVEL.load(Ordering::Relaxed)
}

/// Initial level: an explicit `LINGXIA_LOG_LEVEL` wins and pins the choice;
/// otherwise a dev session defaults to `debug` and everything else to `info`.
/// Returns `(level, pinned_by_env)`.
fn resolve_initial_level() -> (i32, bool) {
    if let Ok(value) = std::env::var(LOG_LEVEL_ENV)
        && let Some(level) = parse_level(&value)
    {
        return (level, true);
    }
    let level = if lxapp::is_dev_session() {
        SDK_LOG_LEVEL_DEBUG
    } else {
        SDK_LOG_LEVEL_INFO
    };
    (level, false)
}

fn parse_level(value: &str) -> Option<i32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "verbose" | "trace" => Some(SDK_LOG_LEVEL_VERBOSE),
        "debug" => Some(SDK_LOG_LEVEL_DEBUG),
        "info" => Some(SDK_LOG_LEVEL_INFO),
        "warn" | "warning" => Some(SDK_LOG_LEVEL_WARN),
        "error" => Some(SDK_LOG_LEVEL_ERROR),
        _ => None,
    }
}

/// Re-evaluate the dev-session default once app config is loaded — the device
/// dev-ws-url isn't known at [`init`]. Raises to `debug` for a dev session
/// unless `LINGXIA_LOG_LEVEL` pinned an explicit level. The Android/Harmony
/// logcat sink cap is already fixed by then, so this reaches the dev-server
/// stream and buffer (what `lxdev logs` shows) rather than raw logcat/hilog.
pub(crate) fn apply_dev_session_level() {
    if LEVEL_PINNED_BY_ENV.load(Ordering::Relaxed) {
        return;
    }
    if lxapp::is_dev_session() && HOST_LOG_LEVEL.load(Ordering::Relaxed) > SDK_LOG_LEVEL_DEBUG {
        set_log_level(SDK_LOG_LEVEL_DEBUG);
    }
}

fn sdk_level_to_filter(level: i32) -> LevelFilter {
    match level {
        SDK_LOG_LEVEL_VERBOSE => LevelFilter::Trace,
        SDK_LOG_LEVEL_DEBUG => LevelFilter::Debug,
        SDK_LOG_LEVEL_INFO => LevelFilter::Info,
        SDK_LOG_LEVEL_WARN => LevelFilter::Warn,
        SDK_LOG_LEVEL_ERROR => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

/// Registers an additional logger that receives every Rust log record emitted by LingXia.
///
/// LingXia still keeps its own platform logger and log manager. The downstream
/// logger is an observer hook for host applications that want to mirror records
/// into another sink.
pub fn register_downstream_logger(
    logger: Box<dyn Log + Send + Sync>,
) -> Result<(), DownstreamLoggerError> {
    DOWNSTREAM_LOGGER
        .set(logger)
        .map_err(|_| DownstreamLoggerError::AlreadyRegistered)
}

/// Forward a log record originating in host (non-Rust) code into the Rust log pipeline.
///
/// The `level` value is the raw FFI contract: 0=verbose, 1=debug, 2=info,
/// 3=warn, 4=error. SDK-facing wrappers should hide these integer values
/// behind platform-native enums.
///
/// **Recommended: forward errors and important warnings only.** Records that
/// arrive here are not just shown in `lxdev logs` — they are buffered for cloud
/// upload / crash diagnosis. Routing routine info/debug or high-frequency traces
/// churns that bounded buffer and evicts the errors it exists to preserve (and
/// costs an FFI crossing + eager message build per call). Keep lifecycle/info/
/// debug on the platform logger (os_log / logcat / hilog); send through this
/// path the diagnostics you'd want in an uploaded log bundle.
pub(crate) fn forward_host_log(
    level: i32,
    category: &str,
    appid: &str,
    path: &str,
    message: &str,
) -> bool {
    let level_int = level;
    let Some(level) = map_sdk_level(level_int) else {
        return false;
    };
    // Shared threshold: drop anything below the active level before doing any
    // work, so host logs honour the same policy as Rust `log::*` records
    // (previously host logs bypassed the level entirely).
    if level_int < HOST_LOG_LEVEL.load(Ordering::Relaxed) {
        return false;
    }
    // Before the pipeline is up (early cold-start), fall back to the platform
    // logger directly so bootstrap warnings still reach logcat/os_log/hilog
    // instead of vanishing.
    if LogManager::get().is_none() {
        let mut msg = LogMessage::new(LogTag::Native, message).with_level(level);
        msg.target = Some(category.to_string());
        if !appid.is_empty() {
            msg.appid = Some(appid.to_string());
        }
        if !path.is_empty() {
            msg.path = Some(path.to_string());
        }
        platform_logger().write(&msg);
        return true;
    }

    LogBuilder::new(LogTag::Native, message)
        .with_level(level)
        .with_target(category.to_string())
        .with_appid(appid.to_string())
        .with_path(path.to_string());
    true
}

struct SdkLogger;

impl Log for SdkLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        LogBuilder::new(LogTag::Native, format!("{}", record.args()))
            .with_level(map_level(record.level()))
            .with_target(record.target().to_string());

        if let Some(logger) = DOWNSTREAM_LOGGER.get()
            && logger.enabled(record.metadata())
        {
            logger.log(record);
        }
    }

    fn flush(&self) {
        if let Some(logger) = DOWNSTREAM_LOGGER.get() {
            logger.flush();
        }
    }
}

fn map_level(level: Level) -> LxLogLevel {
    match level {
        Level::Error => LxLogLevel::Error,
        Level::Warn => LxLogLevel::Warn,
        Level::Info => LxLogLevel::Info,
        Level::Debug => LxLogLevel::Debug,
        Level::Trace => LxLogLevel::Verbose,
    }
}

fn map_sdk_level(level: i32) -> Option<LxLogLevel> {
    match level {
        SDK_LOG_LEVEL_VERBOSE => Some(LxLogLevel::Verbose),
        SDK_LOG_LEVEL_DEBUG => Some(LxLogLevel::Debug),
        SDK_LOG_LEVEL_INFO => Some(LxLogLevel::Info),
        SDK_LOG_LEVEL_WARN => Some(LxLogLevel::Warn),
        SDK_LOG_LEVEL_ERROR => Some(LxLogLevel::Error),
        _ => None,
    }
}

fn format_log_message(message: &LogMessage) -> String {
    let mut prefix = String::from("[");
    prefix.push_str(message.tag.as_str());
    if let Some(appid) = message.appid.as_deref()
        && !appid.is_empty()
    {
        prefix.push(':');
        prefix.push_str(appid);
    }
    if let Some(path) = message.path.as_deref()
        && !path.is_empty()
    {
        prefix.push(':');
        prefix.push_str(path);
    }
    prefix.push(']');
    if let Some(target) = message.target.as_deref()
        && !target.is_empty()
        && target != "lingxia.lxapp"
    {
        prefix.push('[');
        prefix.push_str(target);
        prefix.push(']');
    }
    format!("{prefix} {}", message.message)
}

struct PlatformLogger {
    #[cfg(target_os = "android")]
    android: android_logger::AndroidLogger,
    #[cfg(target_env = "ohos")]
    harmony: ohos_hilog::OhosLogger,
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    apple: oslog::OsLog,
}

impl PlatformLogger {
    fn new() -> Self {
        // Match the logcat/hilog cap to the resolved threshold so a dev build
        // (LINGXIA_LOG_LEVEL=debug) actually surfaces migrated debug/verbose
        // records instead of the old hard Info cap silently dropping them.
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        let sink_filter = sdk_level_to_filter(HOST_LOG_LEVEL.load(Ordering::Relaxed));
        Self {
            #[cfg(target_os = "android")]
            android: android_logger::AndroidLogger::new(
                android_logger::Config::default()
                    .with_max_level(sink_filter)
                    .with_tag("Rust"),
            ),
            #[cfg(target_env = "ohos")]
            harmony: ohos_hilog::OhosLogger::new(
                ohos_hilog::Config::default()
                    .with_max_level(sink_filter)
                    .with_tag("LingXia.Rust"),
            ),
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            apple: oslog::OsLog::new("LingXia.Rust", "sdk"),
        }
    }

    fn write(&self, message: &LogMessage) {
        let formatted = format_log_message(message);
        #[cfg(target_os = "android")]
        {
            let target = message.target.as_deref().unwrap_or("lingxia");
            let args = format_args!("{formatted}");
            let record = Record::builder()
                .args(args)
                .level(map_sdk_level_to_log_level(message.level))
                .target(target)
                .module_path(Some(target))
                .build();
            self.android.log(&record);
        }

        #[cfg(target_env = "ohos")]
        {
            let target = message.target.as_deref().unwrap_or("lingxia");
            let args = format_args!("{formatted}");
            let record = Record::builder()
                .args(args)
                .level(map_sdk_level_to_log_level(message.level))
                .target(target)
                .module_path(Some(target))
                .build();
            self.harmony.log(&record);
        }

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            use oslog::Level as OsLevel;
            let level = match message.level {
                LxLogLevel::Verbose | LxLogLevel::Debug => OsLevel::Debug,
                LxLogLevel::Info => OsLevel::Info,
                LxLogLevel::Warn => OsLevel::Error,
                LxLogLevel::Error => OsLevel::Fault,
            };
            self.apple.with_level(level, &formatted);
        }

        #[cfg(not(any(
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_env = "ohos"
        )))]
        {
            eprintln!("{formatted}");
        }
    }
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn map_sdk_level_to_log_level(level: LxLogLevel) -> Level {
    match level {
        LxLogLevel::Verbose => Level::Trace,
        LxLogLevel::Debug => Level::Debug,
        LxLogLevel::Info => Level::Info,
        LxLogLevel::Warn => Level::Warn,
        LxLogLevel::Error => Level::Error,
    }
}

fn platform_logger() -> &'static PlatformLogger {
    static PLATFORM_LOGGER: OnceLock<PlatformLogger> = OnceLock::new();
    PLATFORM_LOGGER.get_or_init(PlatformLogger::new)
}
