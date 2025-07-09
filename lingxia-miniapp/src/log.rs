use std::sync::{Arc, OnceLock};
use tokio::sync::watch;

/// Log levels that match Android/iOS common levels
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum LogTag {
    Native,                // For logs from Rust/native code
    WebViewConsole,        // For logs from WebView's JavaScript console
    LxAppServiceConsole, // For logs from LxApp service layer
}

impl LogTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogTag::Native => "Native",
            LogTag::WebViewConsole => "JSView",
            LogTag::LxAppServiceConsole => "JSService",
        }
    }
}

/// Log message structure
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub tag: LogTag,
    pub level: LogLevel,
    pub appid: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

impl LogMessage {
    /// Create a new LogMessage with default level Info
    fn new(tag: LogTag, message: impl std::fmt::Display) -> Self {
        Self {
            tag,
            level: LogLevel::Info,
            appid: None,
            path: None,
            message: message.to_string(),
        }
    }
}

/// Global logger manager using OnceLock for singleton pattern
///
/// Usage pattern:
/// - Use `init()` to initialize the logger once
/// - Everywhere else: Use `get()` for concurrent read access
///
/// This avoids locking overhead since the manager is initialized once and then only read.
/// The underlying platform logging (Android log, etc.) handles concurrency well.
static GLOBAL_LOG_MANAGER: OnceLock<Arc<LogManager>> = OnceLock::new();

/// Global logger manager
pub struct LogManager {
    sender: watch::Sender<LogMessage>,
    logger: Box<dyn Fn(&LogMessage) + Send + Sync>,
}

impl LogManager {
    /// Initialize the global logger instance
    pub fn init<F>(logger: F) -> Arc<Self>
    where
        F: Fn(&LogMessage) + Send + Sync + 'static,
    {
        GLOBAL_LOG_MANAGER
            .get_or_init(|| {
                let (sender, _receiver) = watch::channel(LogMessage {
                    tag: LogTag::Native,
                    level: LogLevel::Info,
                    appid: None,
                    path: None,
                    message: Default::default(),
                });

                Arc::new(LogManager {
                    sender,
                    logger: Box::new(logger),
                })
            })
            .clone()
    }

    /// Gets global log manager instance if initialized
    pub fn get() -> Option<Arc<Self>> {
        GLOBAL_LOG_MANAGER.get().cloned()
    }

    /// Subscribe to log messages for network transmission
    pub fn subscribe(&self) -> watch::Receiver<LogMessage> {
        self.sender.subscribe()
    }

    /// Print a log message to the native logger
    /// This is useful for receivers who want to selectively print messages
    pub fn print_to_native(&self, message: &LogMessage) {
        (self.logger)(message);
    }

    /// Log a message
    fn log(&self, message: LogMessage) {
        if self.sender.receiver_count() > 0 {
            let _ = self.sender.send(message.clone());
        } else {
            // Print all messages when not subscribed
            (self.logger)(&message);
        }
    }
}

/// Global logging function for scenarios without appid and path context
pub fn log(tag: LogTag, level: LogLevel, message: impl std::fmt::Display) {
    if let Some(manager) = GLOBAL_LOG_MANAGER.get() {
        let log_message = LogMessage {
            tag,
            level,
            appid: None,
            path: None,
            message: message.to_string(),
        };
        manager.log(log_message);
    }
}

/// Macros for convenient logging
///
/// These macros provide a convenient way to create log messages.
///
/// Usage:
/// ```rust
/// // Simple usage - prints immediately
/// info!("Simple message");
///
/// // With context - use fluent API
/// info!("Message with context")
///     .with_appid("my_app")
///     .with_path("pages/home");
/// ```
/// Create an info log message
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
    };
}

/// Create a warning log message
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Warn)
    };
}

/// Create an error log message
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Error)
    };
}

/// Create a debug log message
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Debug)
    };
}

/// Create a verbose log message
#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        $crate::log::LogBuilder::new($crate::log::LogTag::Native, format!($($arg)*))
            .with_level($crate::log::LogLevel::Verbose)
    };
}

/// Log builder that automatically prints when dropped
/// This provides a fluent API without requiring explicit print() calls
pub struct LogBuilder {
    message: LogMessage,
}

impl LogBuilder {
    /// Create a new log builder
    pub fn new(tag: LogTag, message: impl std::fmt::Display) -> Self {
        Self {
            message: LogMessage::new(tag, message),
        }
    }

    /// Set the app ID for this log message
    pub fn with_appid(mut self, appid: impl Into<String>) -> Self {
        self.message.appid = Some(appid.into());
        self
    }

    /// Set the path for this log message
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.message.path = Some(path.into());
        self
    }

    /// Set the log level for this log message
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.message.level = level;
        self
    }
}

impl Drop for LogBuilder {
    fn drop(&mut self) {
        if let Some(manager) = GLOBAL_LOG_MANAGER.get() {
            manager.log(self.message.clone());
        }
    }
}
