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
pub enum LogTag {
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

pub trait Logging {
    fn log(&self, path: &str, level: LogLevel, tag: LogTag, message: impl std::fmt::Display);

    fn verbose(&self, path: &str, message: impl std::fmt::Display) {
        self.log(path, LogLevel::Verbose, LogTag::Native, message)
    }

    fn debug(&self, path: &str, message: impl std::fmt::Display) {
        self.log(path, LogLevel::Debug, LogTag::Native, message)
    }

    fn info(&self, path: &str, message: impl std::fmt::Display) {
        self.log(path, LogLevel::Info, LogTag::Native, message)
    }

    fn warn(&self, path: &str, message: impl std::fmt::Display) {
        self.log(path, LogLevel::Warn, LogTag::Native, message)
    }

    fn error(&self, path: &str, message: impl std::fmt::Display) {
        self.log(path, LogLevel::Error, LogTag::Native, message)
    }
}

impl Logging for MiniApp {
    // TODO: send log to network server
    fn log(&self, _path: &str, level: LogLevel, tag: LogTag, message: impl std::fmt::Display) {
        self.controller.log(
            &self.appid,
            level,
            &format!("[{}] {}", tag.as_str(), message),
        )
    }
}
