//! Geolocation APIs for native Rust code.
//!
//! Use [`request`] to request one location fix from the host platform. The host
//! owns permission prompts and provider-specific behavior; this module returns a
//! typed result and maps platform failures into [`crate::Error`].
//!
//! # Example
//!
//! ```ignore
//! use lingxia::location::{self, LocationOptions};
//!
//! let fix = location::request(LocationOptions::high_accuracy()).await?;
//! log::info!("at {}, {}", fix.latitude, fix.longitude);
//! ```

use lingxia_platform::traits::location::{Location as PlatformLocation, LocationRequestConfig};
use serde::{Deserialize, Serialize};

/// Options for a single location request.
///
/// `high_accuracy` asks the platform for its more precise location mode and
/// requests altitude when available.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationOptions {
    #[serde(default)]
    pub high_accuracy: bool,
}

impl LocationOptions {
    /// Creates a request using the platform default accuracy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a request for the platform's more precise location mode.
    pub fn high_accuracy() -> Self {
        Self {
            high_accuracy: true,
        }
    }
}

/// A single resolved location fix.
///
/// Accuracy-related fields default to `0.0` when the host omits them.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub accuracy: f64,
    #[serde(default)]
    pub altitude: f64,
    #[serde(default)]
    pub speed: f64,
    #[serde(default)]
    pub vertical_accuracy: f64,
    #[serde(default)]
    pub horizontal_accuracy: f64,
}

/// Internal deserialization shape for platform payloads.
#[derive(Deserialize)]
struct RawLocation {
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    accuracy: f64,
    #[serde(default)]
    altitude: f64,
    #[serde(default)]
    speed: f64,
    #[serde(default)]
    vertical_accuracy: f64,
    #[serde(default)]
    horizontal_accuracy: f64,
}

impl From<RawLocation> for Location {
    fn from(raw: RawLocation) -> Self {
        Self {
            latitude: raw.latitude,
            longitude: raw.longitude,
            accuracy: raw.accuracy,
            altitude: raw.altitude,
            speed: raw.speed,
            vertical_accuracy: raw.vertical_accuracy,
            horizontal_accuracy: raw.horizontal_accuracy,
        }
    }
}

/// Requests one location fix from the host platform.
pub async fn request(options: LocationOptions) -> crate::Result<Location> {
    let config = LocationRequestConfig {
        is_high_accuracy: options.high_accuracy,
        high_accuracy_expire_time: None,
        include_altitude: options.high_accuracy,
    };
    let raw = crate::runtime::platform()?
        .request_location(config)
        .await
        .map_err(crate::Error::from)?;
    let parsed: RawLocation = serde_json::from_str(&raw)
        .map_err(|err| crate::Error::platform(format!("parse location: {err} (raw: {raw})")))?;
    Ok(parsed.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_platform_location_payload() {
        let raw = r#"{"latitude":37.33,"longitude":-122.03,"speed":1.5,"accuracy":5.0,"altitude":12.0,"vertical_accuracy":3.0,"horizontal_accuracy":5.0}"#;
        let parsed: RawLocation = serde_json::from_str(raw).unwrap();
        let dto: Location = parsed.into();
        assert_eq!(dto.latitude, 37.33);
        assert_eq!(dto.longitude, -122.03);
        assert_eq!(dto.accuracy, 5.0);
        assert_eq!(dto.altitude, 12.0);
    }

    #[test]
    fn tolerates_minimal_payload() {
        let raw = r#"{"latitude":1.0,"longitude":2.0}"#;
        let parsed: RawLocation = serde_json::from_str(raw).unwrap();
        let dto: Location = parsed.into();
        assert_eq!(dto.accuracy, 0.0);
        assert_eq!(dto.speed, 0.0);
    }
}
