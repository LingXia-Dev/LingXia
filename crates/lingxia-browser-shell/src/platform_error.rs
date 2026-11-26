use lingxia_platform::PlatformError;

pub(crate) fn map_platform_error(api: &str, error: PlatformError) -> lingxia_error::Error {
    match error {
        PlatformError::BusinessError(number) => {
            lingxia_error::Error::business(number, format!("{api} failed"))
        }
        other => other.into(),
    }
}
