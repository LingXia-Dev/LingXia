use lingxia_error::{Error, code};

/// Callback wire code for an unmounted chrome presenter. Mirrored by the Android and HarmonyOS
/// SDKs, the only platforms whose chrome update can run before a presenter exists.
pub const PRESENTER_UNAVAILABLE_CODE: u32 = 1001;

/// Internal `reason` only: callers keep the patch and return `Ok`, so it never reaches the wire.
const PRESENTER_UNAVAILABLE: &str = "presenterUnavailable";

/// The lxapp's native chrome host is not mounted, so nothing painted. Not a failure: rust keeps
/// the patch and the first presenter reads it.
pub fn presenter_unavailable() -> Error {
    Error::invalid_state("page chrome presenter not mounted").with_reason(PRESENTER_UNAVAILABLE)
}

pub fn is_presenter_unavailable(error: &Error) -> bool {
    error.reason() == Some(PRESENTER_UNAVAILABLE)
}

/// The user dismissed the platform UI (business code 2000): an outcome, not a failure.
pub fn is_user_dismissed(error: &Error) -> bool {
    error.is(code::ABORT) && error.reason() == Some("userCanceled")
}

/// A native callback's numeric code, as its code and `reason`.
pub(crate) fn callback_error(number: u32) -> Error {
    Error::business(number, "platform call failed")
}

/// [`callback_error`] for a chrome update, where [`PRESENTER_UNAVAILABLE_CODE`] means a deferred
/// paint rather than an internal failure.
#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) fn chrome_callback_error(number: u32) -> Error {
    if number == PRESENTER_UNAVAILABLE_CODE {
        presenter_unavailable()
    } else {
        callback_error(number)
    }
}

pub(crate) fn callback_dropped() -> Error {
    Error::internal("platform callback dropped")
}
