use std::future::Future;

use crate::error::PlatformError;

pub const DEFAULT_LOCATION_TIMEOUT_MS: u32 = 10_000;

#[derive(Debug, Clone, Default)]
pub struct LocationRequestConfig {
    pub is_high_accuracy: bool,
    pub high_accuracy_expire_time: Option<u32>,
    pub include_altitude: bool,
}

impl LocationRequestConfig {
    pub fn effective_timeout_ms(&self) -> u32 {
        self.high_accuracy_expire_time
            .unwrap_or(DEFAULT_LOCATION_TIMEOUT_MS)
    }
}

pub trait Location: Send + Sync + 'static {
    fn is_location_enabled(&self) -> Result<bool, PlatformError>;

    fn request_location(
        &self,
        config: LocationRequestConfig,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_timeout_defaults_to_ten_seconds() {
        assert_eq!(
            LocationRequestConfig::default().effective_timeout_ms(),
            10_000
        );
    }

    #[test]
    fn location_timeout_preserves_explicit_value() {
        let config = LocationRequestConfig {
            high_accuracy_expire_time: Some(2_500),
            ..LocationRequestConfig::default()
        };
        assert_eq!(config.effective_timeout_ms(), 2_500);
    }
}
