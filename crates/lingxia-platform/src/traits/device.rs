use crate::error::PlatformError;
use crate::{DeviceInfo, ScreenInfo};

pub trait Device: Send + Sync + 'static {
    fn device_info(&self) -> DeviceInfo;
    fn screen_info(&self) -> ScreenInfo;
    fn vibrate(&self, long: bool) -> Result<(), PlatformError>;
    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError>;
}

pub trait DeviceHardware: Send + Sync + 'static {
    /// Get total physical memory in bytes.
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        Err(PlatformError::NotSupported(
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
        Err(PlatformError::NotSupported(
            "get_storage_total_bytes not implemented".to_string(),
        ))
    }
}
