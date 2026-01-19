use crate::error::PlatformError;
use crate::traits::wifi::{Wifi, WifiConnectRequest, WifiGetConnectedRequest};
use std::os::raw::c_int;

use super::Platform;

mod ffi {
    use super::*;

    pub type WifiResultCode = c_int;
    pub const WIFI_SUCCESS: WifiResultCode = 0;

    #[link(name = "wifi_ndk")]
    unsafe extern "C" {
        pub fn OH_Wifi_IsWifiEnabled(enabled: *mut bool) -> WifiResultCode;
    }
}

impl Wifi for Platform {
    fn start_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("startWifi", &[&callback_id_str])
            .map_err(|e| PlatformError::Platform(format!("Failed to start WiFi: {}", e)))
    }

    fn stop_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("stopWifi", &[&callback_id_str])
            .map_err(|e| PlatformError::Platform(format!("Failed to stop WiFi: {}", e)))
    }

    fn connect_wifi(&self, request: WifiConnectRequest) -> Result<(), PlatformError> {
        let callback_id_str = request.callback_id.to_string();
        let password = request.password.unwrap_or_default();

        lingxia_webview::tsfn::call_arkts(
            "connectWifi",
            &[&callback_id_str, &request.ssid, &password],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to connect WiFi: {}", e)))
    }

    fn get_wifi_list(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("getWifiList", &[&callback_id_str])
            .map_err(|e| PlatformError::Platform(format!("Failed to get WiFi list: {}", e)))
    }

    fn get_connected_wifi(&self, request: WifiGetConnectedRequest) -> Result<(), PlatformError> {
        let callback_id_str = request.callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("getConnectedWifi", &[&callback_id_str])
            .map_err(|e| PlatformError::Platform(format!("Failed to get connected WiFi: {}", e)))
    }

    fn is_wifi_enabled(&self) -> Result<bool, PlatformError> {
        let mut enabled = false;
        let result = unsafe { ffi::OH_Wifi_IsWifiEnabled(&mut enabled) };

        if result == ffi::WIFI_SUCCESS {
            Ok(enabled)
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to check WiFi status: {}",
                result
            )))
        }
    }

    fn add_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("addWifiStateListener", &[&callback_id_str]).map_err(
            |e| PlatformError::Platform(format!("Failed to add WiFi state listener: {}", e)),
        )
    }

    fn remove_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let callback_id_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("removeWifiStateListener", &[&callback_id_str]).map_err(
            |e| PlatformError::Platform(format!("Failed to remove WiFi state listener: {}", e)),
        )
    }
}
