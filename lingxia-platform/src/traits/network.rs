use crate::error::PlatformError;

pub trait Network: Send + Sync + 'static {
    /// Get current network info via callback.
    ///
    /// Callback payload example:
    /// `{ "isConnected": true, "networkType": "wifi", "ipv4": ["192.168.1.12"], "ipv6": ["240e:3a1:..."] }`
    fn get_network_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "get_network_info not implemented".to_string(),
        ))
    }

    /// Add listener for network changes.
    ///
    /// Platform should push callback payload:
    /// `{ "isConnected": boolean, "networkType": "none|unknown|wifi|2g|3g|4g|5g|ethernet", "ipv4": string[], "ipv6": string[] }`
    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "add_network_change_listener not implemented".to_string(),
        ))
    }

    /// Remove a previously registered network change listener.
    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        let _ = callback_id;
        Err(PlatformError::NotSupported(
            "remove_network_change_listener not implemented".to_string(),
        ))
    }
}
