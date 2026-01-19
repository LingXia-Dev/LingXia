use crate::error::PlatformError;
use crate::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest};

use super::Platform;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use super::ffi;

// Macro to reduce cfg block duplication for Apple-only functions
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
                "WiFi APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }};
}

impl Wifi for Platform {
    fn start_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::start_wifi(callback_id))
    }

    fn stop_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::stop_wifi(callback_id))
    }

    fn connect_wifi(&self, request: WifiConnectRequest) -> Result<(), PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            ffi::connect_wifi(
                request.callback_id,
                &request.ssid,
                request.password.as_deref(),
            );
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "WiFi APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }

    fn get_wifi_list(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::get_wifi_list(callback_id))
    }

    fn get_connected_wifi(&self, request: WifiGetConnectedRequest) -> Result<(), PlatformError> {
        let callback_id = request.callback_id;
        apple_only!(callback_id, ffi::get_connected_wifi(callback_id))
    }

    fn is_wifi_enabled(&self) -> Result<bool, PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            Ok(ffi::is_wifi_enabled())
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            Err(PlatformError::Platform(
                "WiFi APIs are only supported on iOS/macOS".to_string(),
            ))
        }
    }

    fn add_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::add_wifi_state_listener(callback_id))
    }

    fn remove_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        apple_only!(callback_id, ffi::remove_wifi_state_listener(callback_id))
    }
}
