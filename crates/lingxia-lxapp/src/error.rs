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

    /// The surface's role forbids the operation: hiding a main, or closing or
    /// moving the stable root main.
    #[error("Surface role locked: {0}")]
    SurfaceRoleLocked(String),

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

    /// A coded error from a route or a lower layer, carried to the boundary as it is.
    #[error("{0}")]
    Coded(lingxia_error::Error),
}

impl From<lingxia_error::Error> for LxAppError {
    fn from(error: lingxia_error::Error) -> Self {
        LxAppError::Coded(error)
    }
}

/// The boundary's one shape: what Logic, a WebView and a channel receive.
impl From<LxAppError> for lingxia_error::Error {
    fn from(error: LxAppError) -> Self {
        use lingxia_error::{Error, code};
        match error {
            LxAppError::Coded(error) => error,
            LxAppError::ResourceNotFound(detail) | LxAppError::PluginNotConfigured(detail) => {
                Error::not_found(detail)
            }
            LxAppError::InvalidParameter(detail) | LxAppError::ResourceExhausted(detail) => {
                Error::invalid_arg(detail)
            }
            LxAppError::UnsupportedOperation(detail) => Error::not_supported(detail),
            LxAppError::SurfaceConflict(detail) => Error::new(code::SURFACE_CONFLICT, detail),
            LxAppError::SurfaceRoleLocked(detail) => Error::new(code::SURFACE_ROLE_LOCKED, detail),
            LxAppError::IoError(detail) => Error::new(code::IO, detail),
            LxAppError::Bridge(detail) if detail == "Canceled" => Error::aborted(detail),
            LxAppError::RongJSHost {
                code,
                message,
                data,
            } => host_error(code, message, data),
            other => Error::internal(other.to_string()),
        }
    }
}

/// A Rong host error at the boundary: a numeric code (or `bizCode` beside a non-`E_` code)
/// becomes its code and reason; `reason` and `detail` in its data move to their own fields.
fn host_error(code: String, message: String, data: Option<Value>) -> lingxia_error::Error {
    let mut data = data;
    let (mut reason, mut detail, mut biz) = (None, None, None);
    if let Some(fields) = data.as_mut().and_then(Value::as_object_mut) {
        if let Some(Value::String(raised)) = fields.remove("reason") {
            reason = Some(raised);
        }
        if let Some(Value::String(text)) = fields.remove("detail") {
            detail = Some(text);
        }
        biz = fields.remove("bizCode").and_then(|value| {
            value
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        });
    }
    let message = detail.unwrap_or(message);
    let numeric = code
        .parse::<u32>()
        .ok()
        .or_else(|| biz.filter(|_| !code.starts_with("E_")));
    let mut error = match numeric {
        Some(number) => lingxia_error::Error::business(number, message),
        None => {
            let error = lingxia_error::Error::new(code, message);
            match biz.and_then(|number| lingxia_error::business_code(number).1) {
                Some(reason) => error.with_reason(reason),
                None => error,
            }
        }
    };
    if let Some(reason) = reason {
        error = error.with_reason(reason);
    }
    match data {
        Some(Value::Object(fields)) if fields.is_empty() => error,
        Some(data) => error.with_data(data),
        None => error,
    }
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
            lingxia_update::UpdateError::RequiresRuntimeUpgrade(detail) => {
                LxAppError::requires_runtime_upgrade(detail)
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

const REQUIRES_RUNTIME_UPGRADE_CODE: &str = "6002";
/// Spelled here: the crate also builds without rong.
const PERMISSION_DENIED_CODE: &str = "E_PERMISSION_DENIED";

impl LxAppError {
    /// This host's runtime is below the package's `minRuntime`.
    pub fn requires_runtime_upgrade(detail: impl Into<String>) -> Self {
        Self::RongJSHost {
            code: REQUIRES_RUNTIME_UPGRADE_CODE.to_string(),
            message: detail.into(),
            data: None,
        }
    }

    pub fn is_requires_runtime_upgrade(&self) -> bool {
        match self {
            Self::RongJSHost { code, .. } => code == REQUIRES_RUNTIME_UPGRADE_CODE,
            Self::Coded(error) => error.reason() == Some("hostUpgradeRequired"),
            _ => false,
        }
    }

    /// The host refused the call (`E_PERMISSION_DENIED`): the lxapp's policy, not a fault.
    pub fn is_refusal(&self) -> bool {
        match self {
            Self::RongJSHost { code, .. } => code == PERMISSION_DENIED_CODE,
            Self::Coded(error) => error.is(PERMISSION_DENIED_CODE),
            _ => false,
        }
    }

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
    fn a_coded_error_crosses_the_boundary_unchanged() {
        let coded = lingxia_error::Error::permission_denied("lxapp scope required")
            .with_reason("photosPermissionDenied");
        let back = lingxia_error::Error::from(LxAppError::from(coded));
        assert_eq!(back.code(), "E_PERMISSION_DENIED");
        assert_eq!(back.reason(), Some("photosPermissionDenied"));
        assert!(LxAppError::from(lingxia_error::Error::permission_denied("x")).is_refusal());
    }

    #[test]
    fn a_host_error_number_becomes_its_code_and_an_e_code_keeps_its_own() {
        let numeric = lingxia_error::Error::from(LxAppError::requires_runtime_upgrade("old"));
        assert_eq!(numeric.code(), "E_NOT_SUPPORTED");
        assert_eq!(numeric.reason(), Some("hostUpgradeRequired"));
        let coded = lingxia_error::Error::from(LxAppError::RongJSHost {
            code: "E_INVALID_ARG".to_string(),
            message: "localized".to_string(),
            data: Some(serde_json::json!({
                "bizCode": 1002,
                "detail": "duplicate route",
                "reason": "tabAlreadyOpen",
                "target": "pages/home",
            })),
        });
        assert_eq!(coded.code(), "E_INVALID_ARG");
        assert_eq!(coded.message(), "duplicate route");
        assert_eq!(coded.reason(), Some("tabAlreadyOpen"));
        assert_eq!(
            coded.data(),
            Some(&serde_json::json!({ "target": "pages/home" }))
        );
        assert_eq!(
            lingxia_error::Error::from(LxAppError::Bridge("Canceled".to_string())).code(),
            "E_ABORT"
        );
    }

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
