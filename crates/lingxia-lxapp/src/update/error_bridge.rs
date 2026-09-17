use crate::error::LxAppError;
use lingxia_update::UpdateError;

pub(super) fn lxapp_error_to_update_error(error: LxAppError) -> UpdateError {
    let is_runtime_gate = error.is_requires_runtime_upgrade();
    match error {
        LxAppError::InvalidParameter(detail) => UpdateError::invalid_parameter(detail),
        LxAppError::UnsupportedOperation(detail) => UpdateError::unsupported(detail),
        LxAppError::ResourceNotFound(detail) => UpdateError::not_found(detail),
        LxAppError::IoError(detail) => UpdateError::io(detail),
        LxAppError::WebView(detail)
        | LxAppError::Runtime(detail)
        | LxAppError::ChannelError(detail)
        | LxAppError::ResourceExhausted(detail)
        | LxAppError::SurfaceConflict(detail)
        | LxAppError::Bridge(detail)
        | LxAppError::RongJS(detail)
        | LxAppError::PluginNotConfigured(detail)
        | LxAppError::PluginDownloadFailed(detail)
        | LxAppError::InvalidJsonFile(detail) => UpdateError::runtime(detail),
        LxAppError::RongJSHost { message, .. } if is_runtime_gate => {
            UpdateError::requires_runtime_upgrade(message)
        }
        LxAppError::RongJSHost { code, message, .. } => {
            UpdateError::runtime(format!("{code}: {message}"))
        }
    }
}
