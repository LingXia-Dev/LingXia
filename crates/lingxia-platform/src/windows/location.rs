//! Windows location services via WinRT `Windows.Devices.Geolocation`.
//!
//! Only one-shot lookups are implemented; there is no watch/subscribe API in
//! the [`Location`] trait, so `Geolocator.PositionChanged` streaming is not
//! needed. WinRT activation is safe from any thread: windows-rs ensures the
//! MTA on first factory activation, and `IAsyncOperation` integrates with the
//! executor via `IntoFuture` (no blocking waits), so the futures returned
//! here are async-safe.

use std::future::Future;
use std::time::Duration;

use serde_json::json;
use windows::Devices::Geolocation::{
    GeolocationAccessStatus, Geolocator, Geoposition, PositionAccuracy, PositionStatus,
};
use windows::Win32::Foundation::E_ACCESSDENIED;

use super::Platform;
use crate::error::PlatformError;
use crate::traits::location::{Location, LocationRequestConfig};

// Business error codes shared with the other backends (see apple/location.rs).
const CODE_PERMISSION_DENIED: u32 = 3002;
const CODE_SERVICES_DISABLED: u32 = 4001;
const CODE_TIMEOUT: u32 = 5002;

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        let geolocator = Geolocator::new().map_err(|err| {
            PlatformError::Platform(format!("failed to create Geolocator: {err}"))
        })?;
        let status = geolocator.LocationStatus().map_err(|err| {
            PlatformError::Platform(format!("failed to query location status: {err}"))
        })?;
        // Before the first position request the status is NotInitialized
        // (unknown); only a definitive "switched off"/"no hardware" answer
        // maps to false.
        Ok(status != PositionStatus::Disabled && status != PositionStatus::NotAvailable)
    }

    fn request_location(
        &self,
        config: LocationRequestConfig,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async move {
            match config.high_accuracy_expire_time {
                Some(timeout_ms) if timeout_ms > 0 => tokio::time::timeout(
                    Duration::from_millis(u64::from(timeout_ms)),
                    request_location_once(config),
                )
                .await
                .map_err(|_| PlatformError::BusinessError(CODE_TIMEOUT))?,
                _ => request_location_once(config).await,
            }
        }
    }
}

async fn request_location_once(config: LocationRequestConfig) -> Result<String, PlatformError> {
    // For unpackaged desktop apps RequestAccessAsync cannot show a consent
    // prompt; it reflects the system-wide location privacy switch. A hard
    // Denied is authoritative, but transient failures fall through to the
    // position request, which reports E_ACCESSDENIED itself when blocked.
    match Geolocator::RequestAccessAsync() {
        Ok(operation) => match operation.await {
            Ok(status) if status == GeolocationAccessStatus::Denied => {
                return Err(PlatformError::BusinessError(CODE_PERMISSION_DENIED));
            }
            Ok(_) => {}
            Err(err) => log::warn!(
                "Windows Location: RequestAccessAsync failed ({err}); attempting lookup anyway"
            ),
        },
        Err(err) => log::warn!(
            "Windows Location: RequestAccessAsync unavailable ({err}); attempting lookup anyway"
        ),
    }

    let geolocator = Geolocator::new()
        .map_err(|err| PlatformError::Platform(format!("failed to create Geolocator: {err}")))?;
    let accuracy = if config.is_high_accuracy {
        PositionAccuracy::High
    } else {
        PositionAccuracy::Default
    };
    if let Err(err) = geolocator.SetDesiredAccuracy(accuracy) {
        log::warn!("Windows Location: SetDesiredAccuracy failed: {err}");
    }
    if matches!(geolocator.LocationStatus(), Ok(status) if status == PositionStatus::Disabled) {
        return Err(PlatformError::BusinessError(CODE_SERVICES_DISABLED));
    }

    let operation = geolocator
        .GetGeopositionAsync()
        .map_err(map_geoposition_error)?;
    let position = operation.await.map_err(map_geoposition_error)?;
    build_location_payload(&position, config.include_altitude)
}

fn map_geoposition_error(err: windows::core::Error) -> PlatformError {
    if err.code() == E_ACCESSDENIED {
        PlatformError::BusinessError(CODE_PERMISSION_DENIED)
    } else {
        PlatformError::Platform(format!("location request failed: {err}"))
    }
}

/// Payload shape matches the other backends (see apple/location.rs).
fn build_location_payload(
    position: &Geoposition,
    include_altitude: bool,
) -> Result<String, PlatformError> {
    let coordinate = position.Coordinate().map_err(|err| {
        PlatformError::Platform(format!("location result missing coordinate: {err}"))
    })?;
    let latitude = coordinate
        .Latitude()
        .map_err(|err| PlatformError::Platform(format!("failed to read latitude: {err}")))?;
    let longitude = coordinate
        .Longitude()
        .map_err(|err| PlatformError::Platform(format!("failed to read longitude: {err}")))?;
    // Accuracy/AltitudeAccuracy/Speed/Altitude are nullable IReference values;
    // `unwrap_or` covers both null and call failure.
    let horizontal_accuracy = sanitize_measurement(coordinate.Accuracy().unwrap_or(0.0));
    let vertical_accuracy = sanitize_measurement(coordinate.AltitudeAccuracy().unwrap_or(0.0));
    let speed = sanitize_measurement(coordinate.Speed().unwrap_or(0.0));
    let altitude = if include_altitude {
        sanitize_measurement(coordinate.Altitude().unwrap_or(0.0))
    } else {
        0.0
    };

    Ok(json!({
        "latitude": latitude,
        "longitude": longitude,
        "speed": speed,
        "accuracy": horizontal_accuracy,
        "altitude": altitude,
        "vertical_accuracy": vertical_accuracy,
        "horizontal_accuracy": horizontal_accuracy,
    })
    .to_string())
}

fn sanitize_measurement(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}
