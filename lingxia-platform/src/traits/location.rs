use crate::error::PlatformError;

#[derive(Debug, Clone, Default)]
pub struct LocationRequestConfig {
    pub is_high_accuracy: bool,
    pub high_accuracy_expire_time: Option<u32>,
    pub include_altitude: bool,
}

pub trait Location: Send + Sync + 'static {
    fn is_location_enabled(&self) -> Result<bool, PlatformError>;
    fn request_location(
        &self,
        callback_id: u64,
        config: LocationRequestConfig,
    ) -> Result<(), PlatformError>;
}
