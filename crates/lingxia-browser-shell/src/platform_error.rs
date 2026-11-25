use lingxia_platform::PlatformError;
use lxapp::LxAppError;

pub(crate) fn map_platform_error(api: &str, error: PlatformError) -> LxAppError {
    LxAppError::Coded(match error {
        PlatformError::BusinessError(number) => {
            lingxia_error::Error::business(number, format!("{api} failed"))
        }
        other => other.into(),
    })
}
