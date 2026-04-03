use std::future::Future;

use crate::error::PlatformError;

pub trait Network: Send + Sync + 'static {
    fn get_network_info(&self) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "get_network_info not implemented".to_string(),
            ))
        }
    }

    /// Add listener for network changes. Keeps callback_id for stream callbacks.
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
