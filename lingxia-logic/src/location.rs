use crate::{I18nKey, i18n::err_code_message};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{
    Location, LocationRequestConfig, PermissionKind, Permissions, ToastIcon, ToastOptions,
    ToastPosition, UserFeedback,
};
use lxapp::{LxApp, lx};
use rong::function::Optional;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

// Tracks whether this process has already observed a location permission denial
// while trying to perform a location request. Once set, subsequent calls will
// not start a new native location request and will instead surface a toast
// directing the user to enable location in system settings.
static LOCATION_PERMISSION_BLOCKED: AtomicBool = AtomicBool::new(false);

fn is_location_permission_denied(code: u32) -> bool {
    code == 3002 || code == 201
}

fn show_location_permission_toast(lxapp: &LxApp) {
    let _ = lxapp.runtime.show_toast(ToastOptions {
        title: crate::i18n::t(I18nKey::PermissionLocationDenied),
        icon: ToastIcon::Error,
        image: None,
        duration: 2.0,
        mask: false,
        position: ToastPosition::Center,
    });
}

fn location_error_message(code: u32) -> String {
    if is_location_permission_denied(code) {
        err_code_message(3002).unwrap_or_else(|| crate::i18n::t(I18nKey::PermissionLocationDenied))
    } else {
        err_code_message(code).unwrap_or_else(|| format!("Location error: {}", code))
    }
}

fn handle_location_error(lxapp: &LxApp, code: u32) -> RongJSError {
    if is_location_permission_denied(code) {
        LOCATION_PERMISSION_BLOCKED.store(true, Ordering::Relaxed);
        show_location_permission_toast(lxapp);
    }
    RongJSError::Error(location_error_message(code))
}

/// Coordinate conversion: WGS84 to GCJ02 (Mars coordinate system)
/// Reference: https://github.com/wandergis/coordtransform
fn wgs84_to_gcj02(wgs_lat: f64, wgs_lng: f64) -> (f64, f64) {
    const A: f64 = 6378245.0;
    const EE: f64 = 0.006_693_421_622_965_943;

    if out_of_china(wgs_lat, wgs_lng) {
        return (wgs_lat, wgs_lng);
    }

    let mut d_lat = transform_lat(wgs_lng - 105.0, wgs_lat - 35.0);
    let mut d_lng = transform_lng(wgs_lng - 105.0, wgs_lat - 35.0);
    let rad_lat = wgs_lat / 180.0 * std::f64::consts::PI;
    let mut magic = (1.0 - EE * rad_lat.sin() * rad_lat.sin()).sqrt();
    magic = (A * (1.0 - EE)) / (magic * magic * magic);
    d_lat = (d_lat * 180.0) / ((A * (1.0 - EE)) / (magic * magic * magic) * std::f64::consts::PI);
    d_lng = (d_lng * 180.0) / (A / magic * rad_lat.cos() * std::f64::consts::PI);

    (wgs_lat + d_lat, wgs_lng + d_lng)
}

fn out_of_china(lat: f64, lng: f64) -> bool {
    !(72.004..=137.8347).contains(&lng) || !(0.8293..=55.8271).contains(&lat)
}

fn transform_lat(lng: f64, lat: f64) -> f64 {
    let mut ret =
        -100.0 + 2.0 * lng + 3.0 * lat + 0.2 * lat * lat + 0.1 * lng * lat + 0.2 * lng.abs().sqrt();
    ret += (20.0 * (6.0 * lng * std::f64::consts::PI).sin()
        + 20.0 * (2.0 * lng * std::f64::consts::PI).sin())
        * 2.0
        / 3.0;
    ret += (20.0 * (lat * std::f64::consts::PI).sin()
        + 40.0 * ((lat / 3.0) * std::f64::consts::PI).sin())
        * 2.0
        / 3.0;
    ret += (160.0 * ((lat / 12.0) * std::f64::consts::PI).sin()
        + 320.0 * ((lat * std::f64::consts::PI) / 30.0).sin())
        * 2.0
        / 3.0;
    ret
}

fn transform_lng(lng: f64, lat: f64) -> f64 {
    let mut ret =
        300.0 + lng + 2.0 * lat + 0.1 * lng * lng + 0.1 * lng * lat + 0.1 * lng.abs().sqrt();
    ret += (20.0 * (6.0 * lng * std::f64::consts::PI).sin()
        + 20.0 * (2.0 * lng * std::f64::consts::PI).sin())
        * 2.0
        / 3.0;
    ret += (20.0 * (lng * std::f64::consts::PI).sin()
        + 40.0 * ((lng / 3.0) * std::f64::consts::PI).sin())
        * 2.0
        / 3.0;
    ret += (150.0 * ((lng / 12.0) * std::f64::consts::PI).sin()
        + 300.0 * ((lng / 30.0) * std::f64::consts::PI).sin())
        * 2.0
        / 3.0;
    ret
}

/// Location options from JavaScript
#[derive(FromJSObj)]
struct JSLocationOptions {
    #[rename = "type"]
    coordinate_type: Option<String>,
    altitude: Option<bool>,
    #[rename = "isHighAccuracy"]
    is_high_accuracy: Option<bool>,
    #[rename = "highAccuracyExpireTime"]
    high_accuracy_expire_time: Option<u32>,
}

/// Location information
#[derive(Debug, Clone, IntoJSObj)]
pub struct LocationObj {
    /// Latitude, range -90~90, negative for south
    latitude: f64,
    /// Longitude, range -180~180, negative for west
    longitude: f64,
    /// Speed in m/s
    speed: Option<f64>,
    /// Position accuracy in meters (smaller = more accurate)
    accuracy: Option<f64>,
    /// Altitude in meters
    altitude: Option<f64>,
    /// Vertical accuracy in meters
    #[rename = "verticalAccuracy"]
    vertical_accuracy: Option<f64>,
    /// Horizontal accuracy in meters
    #[rename = "horizontalAccuracy"]
    horizontal_accuracy: Option<f64>,
}

impl From<CallbackResult> for LocationObj {
    fn from(result: CallbackResult) -> Self {
        let data = match result {
            CallbackResult::Success(data) => data,
            CallbackResult::Error(_) => return default_location(),
        };

        let parsed: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => return default_location(),
        };

        let latitude = parsed
            .get("latitude")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let longitude = parsed
            .get("longitude")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let speed = parsed.get("speed").and_then(Value::as_f64);
        let accuracy = parsed.get("accuracy").and_then(Value::as_f64);
        let altitude = parsed.get("altitude").and_then(Value::as_f64);
        let vertical_accuracy = parsed.get("vertical_accuracy").and_then(Value::as_f64);
        let horizontal_accuracy = parsed.get("horizontal_accuracy").and_then(Value::as_f64);

        LocationObj {
            latitude,
            longitude,
            speed,
            accuracy,
            altitude,
            vertical_accuracy,
            horizontal_accuracy,
        }
    }
}

fn default_location() -> LocationObj {
    LocationObj {
        latitude: 0.0,
        longitude: 0.0,
        speed: None,
        accuracy: None,
        altitude: None,
        vertical_accuracy: None,
        horizontal_accuracy: None,
    }
}

/// Get location function
async fn get_location(
    ctx: JSContext,
    options: Optional<JSLocationOptions>,
) -> JSResult<LocationObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let blocked = LOCATION_PERMISSION_BLOCKED.load(Ordering::Relaxed);
    // If we already know that location permission has been denied in this
    // process, do not start a new native request. Instead, surface a unified
    // toast guiding the user to enable location in system settings.
    if blocked {
        return Err(handle_location_error(&lxapp, 3002));
    }

    // Optional permission preflight for platforms that support it (e.g. Harmony).
    // If the platform returns an error for this preflight request we treat it as
    // the user denying permission for this call and do not start the location
    // request itself.
    let (perm_callback_id, perm_receiver) = get_callback();
    let permission_preflight_supported = lxapp
        .runtime
        .request_permission(PermissionKind::Location, perm_callback_id)
        .is_ok();

    if permission_preflight_supported {
        let perm_result = perm_receiver.await.map_err(|_| {
            RongJSError::Error("Location permission request cancelled or failed".to_string())
        })?;

        match perm_result {
            CallbackResult::Error(code) => {
                return Err(handle_location_error(&lxapp, code));
            }
            CallbackResult::Success(_) => {}
        }
    }

    // Get callback ID and receiver for the actual location request
    let (callback_id, receiver) = get_callback();

    // Create location request config from options
    let config = if let Some(opts) = options.as_ref() {
        LocationRequestConfig {
            is_high_accuracy: opts.is_high_accuracy.unwrap_or(false),
            high_accuracy_expire_time: opts.high_accuracy_expire_time,
            include_altitude: opts.altitude.unwrap_or(false),
        }
    } else {
        LocationRequestConfig::default()
    };

    // Call runtime interface with callback ID and config
    match lxapp.runtime.request_location(callback_id, config) {
        Ok(()) => {
            // Wait for result from callback
            match receiver.await {
                Ok(result) => {
                    match result {
                        CallbackResult::Error(code) => {
                            return Err(handle_location_error(&lxapp, code));
                        }
                        CallbackResult::Success(data) => {
                            let mut location = LocationObj::from(CallbackResult::Success(data));

                            let requested_type = options
                                .as_ref()
                                .and_then(|opts| opts.coordinate_type.as_deref())
                                .unwrap_or("wgs84");

                            // If GCJ02 coordinate system is requested, perform coordinate conversion
                            if requested_type == "gcj02" {
                                let (gcj_lat, gcj_lng) =
                                    wgs84_to_gcj02(location.latitude, location.longitude);
                                location.latitude = gcj_lat;
                                location.longitude = gcj_lng;
                            }

                            Ok(location)
                        }
                    }
                }
                Err(_) => Err(RongJSError::Error(
                    "Location callback timeout or cancelled".to_string(),
                )),
            }
        }
        Err(e) => Err(RongJSError::Error(format!("Failed to get location: {}", e))),
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_location_func = JSFunc::new(ctx, get_location)?;
    lx::register_js_api(ctx, "getLocation", get_location_func)?;

    Ok(())
}
