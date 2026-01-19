use crate::error::PlatformError;
use crate::{DeviceInfo, ScreenInfo};

pub trait Device: Send + Sync + 'static {
    fn device_info(&self) -> DeviceInfo;
    fn screen_info(&self) -> ScreenInfo;
    fn vibrate(&self, long: bool) -> Result<(), PlatformError>;
    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError>;
}

pub trait DeviceSecureStore: Send + Sync + 'static {
    /// Read a persisted value from a secure, app-scoped store that survives reinstall where supported.
    fn secure_store_read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        Err(PlatformError::Platform(format!(
            "secure_store_read not implemented for key {}",
            key
        )))
    }

    /// Persist a value into the secure store.
    fn secure_store_write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        let _ = (key, value);
        Err(PlatformError::Platform(
            "secure_store_write not implemented".to_string(),
        ))
    }

    /// Delete a value from the secure store.
    fn secure_store_delete(&self, key: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(format!(
            "secure_store_delete not implemented for key {}",
            key
        )))
    }
}

pub trait DeviceHardware: Send + Sync + 'static {
    /// Get total physical memory in bytes.
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        Err(PlatformError::Platform(
            "get_memory_info not implemented".to_string(),
        ))
    }

    /// Get the number of logical CPU cores available.
    fn get_cpu_count(&self) -> usize {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
    }

    /// Get total ROM storage in bytes.
    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        Err(PlatformError::Platform(
            "get_storage_total_bytes not implemented".to_string(),
        ))
    }
}
