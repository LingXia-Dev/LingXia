use lingxia_error::Error;
use std::future::Future;

pub trait Network: Send + Sync + 'static {
    fn get_network_info(&self) -> impl Future<Output = Result<String, Error>> + Send {
        async {
            Err(Error::not_supported(
                "get_network_info not implemented".to_string(),
            ))
        }
    }

    /// Add listener for network changes. Keeps callback_id for stream callbacks.
    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), Error> {
        let _ = callback_id;
        Err(Error::not_supported(
            "add_network_change_listener not implemented".to_string(),
        ))
    }

    /// Remove a previously registered network change listener.
    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), Error> {
        let _ = callback_id;
        Err(Error::not_supported(
            "remove_network_change_listener not implemented".to_string(),
        ))
    }
}
