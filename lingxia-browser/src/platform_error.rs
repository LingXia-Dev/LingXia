use lingxia_platform::PlatformError;
use lxapp::LxAppError;

pub(crate) fn map_platform_error(api: &str, error: PlatformError) -> LxAppError {
    let (code, message) = match error {
        PlatformError::NotSupported(message) => ("E_NOT_SUPPORTED", message),
        PlatformError::InvalidParameter(message) => ("E_INVALID_PARAMETER", message),
        PlatformError::AssetNotFound(message) => ("E_NOT_FOUND", message),
        PlatformError::Platform(message) => ("E_PLATFORM", message),
        PlatformError::BusinessError(code) => {
            return LxAppError::RongJSHost {
                code: code.to_string(),
                message: format!("{api} failed"),
                data: Some(serde_json::json!({ "bizCode": code })),
            };
        }
        PlatformError::CallbackDropped => ("E_CALLBACK_DROPPED", "Callback dropped".to_string()),
    };

    LxAppError::RongJSHost {
        code: code.to_string(),
        message,
        data: None,
    }
}
