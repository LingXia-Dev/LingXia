use crate::error::PlatformError;
use crate::traits::network::Network;

use super::Platform;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use super::ffi;

impl Network for Platform {
    async fn get_network_info(&self) -> Result<String, PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            crate::bg_runtime::await_callback(|callback_id| {
                ffi::get_network_info(callback_id);
                Ok(())
            })
            .await
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            Err(PlatformError::NotSupported(
                "get_network_info is only supported on iOS/macOS".to_string(),
            ))
        }
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            ffi::add_network_change_listener(callback_id);
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let _ = callback_id;
            Err(PlatformError::NotSupported(
                "Network APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            ffi::remove_network_change_listener(callback_id);
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let _ = callback_id;
            Err(PlatformError::NotSupported(
                "Network APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }
}
