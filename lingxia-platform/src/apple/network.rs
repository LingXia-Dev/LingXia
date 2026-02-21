use crate::error::PlatformError;
use crate::traits::network::Network;

use super::Platform;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use super::ffi;

macro_rules! apple_only {
    ($callback_id:ident, $ffi_call:expr) => {{
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            $ffi_call;
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let _ = $callback_id;
            Err(PlatformError::Platform(
                "Network APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }};
}

impl Network for Platform {
    fn get_network_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::get_network_info(callback_id))
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::add_network_change_listener(callback_id))
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(
            callback_id,
            ffi::remove_network_change_listener(callback_id)
        )
    }
}
