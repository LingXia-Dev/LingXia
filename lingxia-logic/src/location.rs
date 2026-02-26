use crate::i18n::{
    js_error_from_business_code, js_error_from_platform_error, js_internal_error,
    js_invalid_parameter_error, js_timeout_error,
};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::traits::location::{Location, LocationRequestConfig};
use lxapp::{LxApp, lx};
use rong::function::Optional;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;

fn handle_location_error(code: u32) -> RongJSError {
    js_error_from_business_code(code)
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

fn parse_location_payload(data: &str) -> Result<LocationObj, RongJSError> {
    let parsed: Value = serde_json::from_str(data)
        .map_err(|e| js_internal_error(format!("getLocation invalid payload: {}", e)))?;

    let latitude = parsed
        .get("latitude")
        .and_then(Value::as_f64)
        .ok_or_else(|| js_internal_error("getLocation payload missing numeric `latitude`"))?;
    let longitude = parsed
        .get("longitude")
        .and_then(Value::as_f64)
        .ok_or_else(|| js_internal_error("getLocation payload missing numeric `longitude`"))?;
    let speed = parsed.get("speed").and_then(Value::as_f64);
    let accuracy = parsed.get("accuracy").and_then(Value::as_f64);
    let altitude = parsed.get("altitude").and_then(Value::as_f64);
    let vertical_accuracy = parsed.get("vertical_accuracy").and_then(Value::as_f64);
    let horizontal_accuracy = parsed.get("horizontal_accuracy").and_then(Value::as_f64);

    Ok(LocationObj {
        latitude,
        longitude,
        speed,
        accuracy,
        altitude,
        vertical_accuracy,
        horizontal_accuracy,
    })
}

/// Get location function
async fn get_location(
    ctx: JSContext,
    options: Optional<JSLocationOptions>,
) -> JSResult<LocationObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let requested_type = options
        .as_ref()
        .and_then(|opts| opts.coordinate_type.as_deref())
        .unwrap_or("wgs84");
    if requested_type != "wgs84" && requested_type != "gcj02" {
        return Err(js_invalid_parameter_error(format!(
            "getLocation invalid type: {}",
            requested_type
        )));
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
                            return Err(handle_location_error(code));
                        }
                        CallbackResult::Success(data) => {
                            let mut location = parse_location_payload(&data)?;

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
                Err(_) => Err(js_timeout_error("Location callback timed out")),
            }
        }
        Err(e) => Err(js_error_from_platform_error(&e)),
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_location_func = JSFunc::new(ctx, get_location)?;
    lx::register_js_api(ctx, "getLocation", get_location_func)?;

    Ok(())
}
