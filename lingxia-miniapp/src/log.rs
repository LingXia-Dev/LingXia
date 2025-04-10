use crate::miniapp::{LogLevel, MiniApp};

impl MiniApp {
    fn log(&self, level: LogLevel, appid: impl AsRef<str>, message: &str) {
        self.runtime
            .log(level, &format!("[{}] {}", appid.as_ref(), message))
    }

    pub fn verbose(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Verbose, appid, &message.to_string())
    }

    pub fn debug(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Debug, appid, &message.to_string())
    }

    pub fn info(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Info, appid, &message.to_string())
    }

    pub fn warn(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Warn, appid, &message.to_string())
    }

    pub fn error(&self, appid: impl AsRef<str>, message: impl std::fmt::Display) {
        self.log(LogLevel::Error, appid, &message.to_string())
    }
}
