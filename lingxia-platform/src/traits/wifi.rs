use crate::error::PlatformError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiInfo {
    /// SSID of the access point.
    pub ssid: String,
    /// BSSID of the access point (MAC address format: "aa:bb:cc:dd:ee:ff").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bssid: Option<String>,
    /// Whether the access point requires authentication.
    pub secure: bool,
    /// Signal strength in the range [0, 100], where 100 is strongest.
    pub signal_strength: u8,
    /// Center frequency in MHz (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct WifiConnectRequest {
    /// SSID of the access point.
    pub ssid: String,
    /// Password for the access point.
    pub password: Option<String>,
    /// Callback ID for success/failure.
    pub callback_id: u64,
}

#[derive(Debug, Clone)]
pub struct WifiGetConnectedRequest {
    /// Callback ID for success/failure.
    pub callback_id: u64,
}

pub trait Wifi: Send + Sync + 'static {
    /// Initialize Wi-Fi module (wx.startWifi).
    ///
    /// # Platform Requirements
    /// - Android: Requires ACCESS_WIFI_STATE and CHANGE_WIFI_STATE permissions
    /// - iOS: Requires NEHotspotConfiguration API (iOS 11+)
    /// - HarmonyOS: Requires ohos.permission.GET_WIFI_INFO
    fn start_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "start_wifi not implemented".to_string(),
        ))
    }

    /// Stop Wi-Fi module (wx.stopWifi).
    fn stop_wifi(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "stop_wifi not implemented".to_string(),
        ))
    }

    /// Connect to a Wi-Fi access point (wx.connectWifi).
    ///
    /// # Platform Limitations
    /// - iOS: Only works for pre-configured networks or hotspot networks
    /// - Android: May require location permissions on Android 6.0+
    fn connect_wifi(&self, request: WifiConnectRequest) -> Result<(), PlatformError> {
        let _ = request;
        Err(PlatformError::NotSupported(
            "connect_wifi not implemented".to_string(),
        ))
    }

    /// Request a Wi-Fi scan and return results via callback (wx.getWifiList).
    ///
    /// Unlike WeChat's event-driven approach, this directly returns scan results
    /// via the callback for Stage 1 simplicity.
    ///
    /// Callback receives: JSON array of WifiInfo objects
    /// Example: [{"ssid":"MyWiFi","bssid":"aa:bb:cc:dd:ee:ff","secure":true,"signalStrength":80,"frequency":2412}]
    fn get_wifi_list(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "get_wifi_list not implemented".to_string(),
        ))
    }

    /// Get current connected Wi-Fi (wx.getConnectedWifi).
    ///
    /// Always returns full WiFi information (SSID, BSSID, secure, signalStrength).
    fn get_connected_wifi(&self, request: WifiGetConnectedRequest) -> Result<(), PlatformError> {
        let _ = request;
        Err(PlatformError::NotSupported(
            "get_connected_wifi not implemented".to_string(),
        ))
    }

    /// Check if WiFi is currently enabled on the device (synchronous).
    ///
    /// Returns true if WiFi hardware is active and available for use.
    ///
    /// # Platform Notes
    /// - Android: Checks WifiManager.isWifiEnabled()
    /// - iOS: Always returns true (WiFi state not accessible)
    /// - HarmonyOS: Checks OH_Wifi_IsWifiEnabled()
    fn is_wifi_enabled(&self) -> Result<bool, PlatformError> {
        Err(PlatformError::NotSupported(
            "is_wifi_enabled not implemented".to_string(),
        ))
    }

    /// Add a listener for WiFi connection state changes.
    ///
    /// Multiple listeners can be registered (supports multiple LxApp instances).
    /// Platform should invoke the callback with connected WiFi info JSON payload
    /// each time WiFi connection state changes.
    ///
    /// # Multi-LxApp Support
    /// - Platform maintains a set of callback_ids
    /// - First listener triggers system WiFi state monitoring
    /// - Each new listener immediately receives current WiFi state
    fn add_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Ok(())
    }

    /// Remove a previously registered WiFi state listener.
    ///
    /// # Multi-LxApp Support
    /// - Removes the specific callback_id from the listener set
    /// - Last listener stops system WiFi state monitoring
    fn remove_wifi_state_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Ok(())
    }
}
