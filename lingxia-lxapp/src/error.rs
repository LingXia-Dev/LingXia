use lingxia_platform::PlatformError;
use rong::RongJSError;
use rong::error::{ErrorData, ErrorNumber};
use serde_json::Value;
use std::io;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LxAppError {
    /// Error when performing web operations
    #[error("WebView error: {0}")]
    WebView(String),

    #[error("{0} not found")]
    ResourceNotFound(String),

    #[error("{0} is not valid JSON file")]
    InvalidJsonFile(String),

    /// Error for invalid parameters
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Error for unsupported operations
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    /// Error for I/O operations (file access, network, etc.)
    #[error("I/O error: {0}")]
    IoError(String),

    /// Error for runtime operations
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// Error channel operations
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Error when resource is exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Error when bridge error
    #[error("Bridge error: {0}")]
    Bridge(String),

    /// Error for Rong runtime
    #[error("Rong Error: {0}")]
    RongJS(String),

    /// Structured Rong host error that preserves code/data metadata
    #[error("{code}: {message}")]
    RongJSHost {
        code: String,
        message: String,
        data: Option<Value>,
    },

    /// Error when plugin is not configured in lxapp.json
    #[error("Plugin not configured: {0}")]
    PluginNotConfigured(String),

    /// Error when plugin download fails
    #[error("Plugin download failed: {0}")]
    PluginDownloadFailed(String),
}

impl From<io::Error> for LxAppError {
    fn from(error: io::Error) -> Self {
        LxAppError::IoError(error.to_string())
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for LxAppError {
    fn from(error: std::sync::mpsc::SendError<T>) -> Self {
        LxAppError::ChannelError(error.to_string())
    }
}

impl From<serde_json::Error> for LxAppError {
    fn from(error: serde_json::Error) -> Self {
        LxAppError::Bridge(format!("JSON Processing Error: {}", error))
    }
}

impl From<RongJSError> for LxAppError {
    fn from(error: RongJSError) -> Self {
        if let Some(host) = error.as_host_error() {
            let data = host.data.as_ref().map(error_data_to_json);
            return LxAppError::RongJSHost {
                code: host.code.to_string(),
                message: host.message.clone(),
                data,
            };
        }
        LxAppError::RongJS(error.to_string())
    }
}

impl From<PlatformError> for LxAppError {
    fn from(error: PlatformError) -> Self {
        LxAppError::Runtime(error.to_string())
    }
}

fn error_data_to_json(data: &ErrorData) -> Value {
    match data {
        ErrorData::Null => Value::Null,
        ErrorData::Bool(v) => Value::Bool(*v),
        ErrorData::String(v) => Value::String(v.clone()),
        ErrorData::Number(n) => match n {
            ErrorNumber::I64(v) => Value::Number(serde_json::Number::from(*v)),
            ErrorNumber::U64(v) => Value::Number(serde_json::Number::from(*v)),
            ErrorNumber::F64(bits) => {
                let num = f64::from_bits(*bits);
                match serde_json::Number::from_f64(num) {
                    Some(value) => Value::Number(value),
                    None => Value::String(num.to_string()),
                }
            }
        },
        ErrorData::Array(items) => Value::Array(items.iter().map(error_data_to_json).collect()),
        ErrorData::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), error_data_to_json(v)))
                .collect(),
        ),
    }
}
