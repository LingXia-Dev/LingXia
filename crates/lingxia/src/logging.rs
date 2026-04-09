use log::{Level, LevelFilter, Log, Metadata, Record};
use lxapp::log::{LogLevel as LxLogLevel, LogManager, LogMessage, LogTag};
use std::sync::OnceLock;

static LOGGING_INIT: OnceLock<()> = OnceLock::new();
static DOWNSTREAM_LOGGER: OnceLock<Box<dyn Log + Send + Sync>> = OnceLock::new();
static SDK_LOGGER: SdkLogger = SdkLogger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownstreamLoggerError {
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

    let _ = LogManager::init(|message| {
        platform_logger().write(message);
    });

    if log::set_logger(&SDK_LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }

    let _ = LOGGING_INIT.set(());
}

pub fn register_downstream_logger(
    logger: Box<dyn Log + Send + Sync>,
) -> Result<(), DownstreamLoggerError> {
    DOWNSTREAM_LOGGER
        .set(logger)
        .map_err(|_| DownstreamLoggerError::AlreadyRegistered)
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

        lxapp::log::LogBuilder::new(LogTag::Native, format!("{}", record.args()))
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
        Self {
            #[cfg(target_os = "android")]
            android: android_logger::AndroidLogger::new(
                android_logger::Config::default()
                    .with_max_level(LevelFilter::Info)
                    .with_tag("Rust"),
            ),
            #[cfg(target_env = "ohos")]
            harmony: ohos_hilog::OhosLogger::new(
                ohos_hilog::Config::default()
                    .with_max_level(LevelFilter::Info)
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
            return;
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
            return;
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
            return;
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
