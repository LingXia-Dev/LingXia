//! Unified push notification APIs for platform FFI layers.

use crate::provider::{self, ProviderError};

fn normalize_token(token: String) -> Result<String, ProviderError> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(ProviderError::invalid_request("push token is empty"));
    }
    Ok(token)
}

/// Bind push token through the registered provider.
pub async fn bind_push_token(token: String) -> Result<(), ProviderError> {
    let token = normalize_token(token)?;
    provider::bind_push_token(token).await
}

/// FFI-friendly push token entrypoint.
///
/// Returns:
/// - `0` when task queued successfully
/// - `1` when validation/scheduling failed
pub fn bind_push_token_for_ffi(token: String) -> i32 {
    let token = match normalize_token(token) {
        Ok(token) => token,
        Err(err) => {
            crate::warn!("[Push] Invalid token: {}", err);
            return 1;
        }
    };

    if let Err(e) = rong::bg::spawn(async move {
        if let Err(err) = provider::bind_push_token(token).await {
            crate::warn!("[Push] bind_push_token failed: {}", err);
        }
    }) {
        crate::error!("[Push] Failed to schedule bind_push_token task: {}", e);
        return 1;
    }
    0
}
