//! Unified push notification APIs for platform FFI layers.

use lxapp::provider;

fn normalize_token(token: String) -> crate::Result<String> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(crate::Error::invalid_request("push token is empty"));
    }
    Ok(token)
}

/// FFI-friendly push token entrypoint.
///
/// Returns:
/// - `0` when task queued successfully
/// - `1` when validation failed
#[cfg_attr(
    not(any(target_os = "ios", target_os = "macos", target_env = "ohos")),
    allow(dead_code)
)]
pub(crate) fn bind_push_token_for_ffi(token: String) -> i32 {
    let token = match normalize_token(token) {
        Ok(token) => token,
        Err(err) => {
            log::warn!("[Push] Invalid token: {}", err);
            return 1;
        }
    };

    std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
        if let Err(err) = provider::bind_push_token(token).await {
            log::warn!("[Push] bind_push_token failed: {}", err);
        }
    }));
    0
}
