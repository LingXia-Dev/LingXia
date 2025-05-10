use crate::miniapp::MiniApp;

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
pub(crate) enum LogTag {
    Native,                // For logs from Rust/native code
    WebViewConsole,        // For logs from WebView's JavaScript console
    MiniAppServiceConsole, // For logs from MiniApp service layer
}

impl LogTag {
    fn as_str(&self) -> &'static str {
        match self {
            LogTag::Native => "Native",
            LogTag::WebViewConsole => "JSView",
            LogTag::MiniAppServiceConsole => "JSService",
        }
    }
}

pub(crate) trait Logging {
    /// Advanced logging for mini-app framework
    /// This logs both to the local platform and can be extended
    /// to handle different log sources (WebView, native, etc.)
    /// and targets (local, remote servers, analytics, etc.)
    fn write_log(&self, path: &str, level: LogLevel, tag: LogTag, message: impl std::fmt::Display);

    fn verbose(&self, path: &str, message: impl std::fmt::Display) {
        self.write_log(path, LogLevel::Verbose, LogTag::Native, message)
    }

    fn debug(&self, path: &str, message: impl std::fmt::Display) {
        self.write_log(path, LogLevel::Debug, LogTag::Native, message)
    }

    fn info(&self, path: &str, message: impl std::fmt::Display) {
        self.write_log(path, LogLevel::Info, LogTag::Native, message)
    }

    fn warn(&self, path: &str, message: impl std::fmt::Display) {
        self.write_log(path, LogLevel::Warn, LogTag::Native, message)
    }

    fn error(&self, path: &str, message: impl std::fmt::Display) {
        self.write_log(path, LogLevel::Error, LogTag::Native, message)
    }
}

impl Logging for MiniApp {
    // Comprehensive logging system for mini-app framework
    fn write_log(
        &self,
        _path: &str,
        level: LogLevel,
        tag: LogTag,
        message: impl std::fmt::Display,
    ) {
        // Log to local platform (essential logs)
        self.controller.log(
            &self.appid,
            level,
            &format!("[{}] {}", tag.as_str(), message),
        );

        // TODO: Log to network server for remote diagnostics
    }
}
