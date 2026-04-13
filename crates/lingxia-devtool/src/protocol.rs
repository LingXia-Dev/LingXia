use lingxia::log::{LogLevel, LogMessage, LogTag};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsPeerRole {
    Devtool,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsLogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for DevtoolsLogLevel {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Verbose => Self::Verbose,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevtoolsLogSource {
    Native,
    WebViewConsole,
    LxAppServiceConsole,
}

impl From<LogTag> for DevtoolsLogSource {
    fn from(value: LogTag) -> Self {
        match value {
            LogTag::Native => Self::Native,
            LogTag::WebViewConsole => Self::WebViewConsole,
            LogTag::LxAppServiceConsole => Self::LxAppServiceConsole,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsLogMessage {
    pub timestamp_ms: u64,
    #[serde(alias = "tag")]
    pub source: DevtoolsLogSource,
    pub level: DevtoolsLogLevel,
    pub appid: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

impl From<&LogMessage> for DevtoolsLogMessage {
    fn from(value: &LogMessage) -> Self {
        Self {
            timestamp_ms: value.timestamp_ms,
            source: value.tag.into(),
            level: value.level.into(),
            appid: value.appid.clone(),
            path: value.path.clone(),
            message: value.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DevtoolsWireMessage {
    Hello {
        role: DevtoolsPeerRole,
    },
    LogBatch {
        logs: Vec<DevtoolsLogMessage>,
    },
    Command {
        command_id: String,
        handler: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    Result {
        command_id: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}
