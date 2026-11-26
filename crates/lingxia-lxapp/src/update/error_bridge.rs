use lingxia_error::code;
use lingxia_update::UpdateError;

pub(super) fn update_error(error: lingxia_error::Error) -> UpdateError {
    if crate::error::is_requires_runtime_upgrade(&error) {
        return UpdateError::requires_runtime_upgrade(error.message());
    }
    let message = error.message().to_string();
    match error.code() {
        code::INVALID_ARG => UpdateError::invalid_parameter(message),
        code::NOT_SUPPORTED | code::SURFACE_ROLE_LOCKED => UpdateError::unsupported(message),
        code::NOT_FOUND => UpdateError::not_found(message),
        code::IO => UpdateError::io(message),
        _ => UpdateError::runtime(message),
    }
}
