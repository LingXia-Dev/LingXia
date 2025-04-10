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
    fn log(
        &self,
        level: LogLevel,
        appid: impl AsRef<str>,
        tag: LogTag,
        message: impl std::fmt::Display,
    );

    fn verbose(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Verbose, appid, LogTag::Native, message)
    }

    fn debug(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Debug, appid, LogTag::Native, message)
    }

    fn info(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Info, appid, LogTag::Native, message)
    }

    fn warn(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Warn, appid, LogTag::Native, message)
    }

    fn error(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Error, appid, LogTag::Native, message)
    }
}

impl Logging for MiniApp {
    fn log(
        &self,
        level: LogLevel,
        appid: impl AsRef<str>,
        tag: LogTag,
        message: impl std::fmt::Display,
    ) {
        self.runtime.log(
            level,
            &format!("[{}][{}] {}", appid.as_ref(), tag.as_str(), message),
        )
    }
}
