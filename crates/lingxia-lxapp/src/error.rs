use lingxia_platform::PlatformError;
use lingxia_webview::WebViewError;
#[cfg(feature = "js-appservice")]
use rong::RongJSError;
#[cfg(feature = "js-appservice")]
use rong::error::{ErrorData, ErrorNumber};
use serde_json::Value;
use std::io;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LxAppError {
    /// Error when performing web operations.
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

    /// The lxapp already owns a live presentation in another shell region.
    #[error("Surface conflict: {0}")]
    SurfaceConflict(String),

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

#[cfg(feature = "js-appservice")]
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
        match error {
            PlatformError::NotSupported(message) => LxAppError::UnsupportedOperation(message),
            PlatformError::InvalidParameter(message) => LxAppError::InvalidParameter(message),
            PlatformError::AssetNotFound(message) => LxAppError::ResourceNotFound(message),
            other => LxAppError::Runtime(other.to_string()),
        }
    }
}

impl From<WebViewError> for LxAppError {
    fn from(error: WebViewError) -> Self {
        match error {
            WebViewError::WebView(detail) => LxAppError::WebView(detail),
            other => LxAppError::WebView(other.to_string()),
        }
    }
}

impl From<lingxia_update::UpdateError> for LxAppError {
    fn from(error: lingxia_update::UpdateError) -> Self {
        match error {
            lingxia_update::UpdateError::InvalidParameter(detail) => {
                LxAppError::InvalidParameter(detail)
            }
            lingxia_update::UpdateError::UnsupportedOperation(detail) => {
                LxAppError::UnsupportedOperation(detail)
            }
            lingxia_update::UpdateError::ResourceNotFound(detail) => {
                LxAppError::ResourceNotFound(detail)
            }
            lingxia_update::UpdateError::Io(detail) => LxAppError::IoError(detail),
            lingxia_update::UpdateError::Runtime(detail) => LxAppError::Runtime(detail),
        }
    }
}

impl From<lingxia_settings::SettingsError> for LxAppError {
    fn from(error: lingxia_settings::SettingsError) -> Self {
        LxAppError::Runtime(error.to_string())
    }
}

#[cfg(feature = "js-appservice")]
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

impl LxAppError {
    /// The message without the variant's prefix.
    ///
    /// Every variant's `Display` prepends or appends its own label, which reads
    /// as noise once the caller has already framed the failure: nesting them
    /// produces "Invalid parameter: iconPath: Invalid parameter: …", and the
    /// variants that append produce "directory traversal not allowed not
    /// found". Use this when embedding one error's reason inside another's
    /// sentence.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::WebView(detail)
            | Self::ResourceNotFound(detail)
            | Self::InvalidJsonFile(detail)
            | Self::InvalidParameter(detail)
            | Self::UnsupportedOperation(detail)
            | Self::IoError(detail)
            | Self::Runtime(detail) => Some(detail),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LxAppError;
    use lingxia_webview::WebViewError;

    #[test]
    fn raw_webview_errors_keep_the_source_label() {
        assert_eq!(
            LxAppError::WebView("WebView not ready".to_string()).to_string(),
            "WebView error: WebView not ready"
        );
    }

    #[test]
    fn typed_webview_errors_have_one_source_label() {
        let error = LxAppError::from(WebViewError::WebView("creation failed".to_string()));
        assert_eq!(error.to_string(), "WebView error: creation failed");

        let error = LxAppError::from(WebViewError::InvalidCreateOptions(
            "missing tag".to_string(),
        ));
        assert_eq!(
            error.to_string(),
            "WebView error: Invalid WebView create options: missing tag"
        );
    }
}
