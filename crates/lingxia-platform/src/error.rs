use thiserror::Error;

/// Platform-specific error types
#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Business error: code {0}")]
    BusinessError(u32),

    #[error("Callback dropped")]
    CallbackDropped,

    /// The lxapp's native chrome host is not mounted, so nothing painted.
    /// Not a failure: rust keeps the patch and the first presenter reads it.
    #[error("Page chrome presenter not mounted")]
    PresenterUnavailable,
}

/// Callback wire code for [`PlatformError::PresenterUnavailable`]. Mirrored by
/// the Android and HarmonyOS SDKs, the only platforms whose chrome update can
/// run before a presenter exists; the rest always have one or answer inline.
pub const PRESENTER_UNAVAILABLE_CODE: u32 = 1001;

/// A chrome update that ran before its presenter existed answers with the
/// shared wire code; recover the variant so callers can tell a deferred paint
/// from one that genuinely failed.
#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) fn unmounted_presenter_or(error: PlatformError) -> PlatformError {
    match error {
        PlatformError::BusinessError(PRESENTER_UNAVAILABLE_CODE) => {
            PlatformError::PresenterUnavailable
        }
        other => other,
    }
}

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for PlatformError {
    fn from(value: jni::errors::Error) -> Self {
        PlatformError::Platform(format!("JNI error: {}", value))
    }
}
