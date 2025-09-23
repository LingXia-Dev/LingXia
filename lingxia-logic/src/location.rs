use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::Location;
use rong::function::Optional;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;
use std::sync::Arc;

/// Coordinate conversion: WGS84 to GCJ02 (Mars coordinate system)
/// Reference: https://github.com/wandergis/coordtransform
fn wgs84_to_gcj02(wgs_lat: f64, wgs_lng: f64) -> (f64, f64) {
    const A: f64 = 6378245.0;
    const EE: f64 = 0.00669342162296594323;

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
    lng < 72.004 || lng > 137.8347 || lat < 0.8293 || lat > 55.8271
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
        if !result.success {
            return default_location();
        }

        let parsed: Value = match serde_json::from_str(&result.data) {
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
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    // Get callback ID and receiver
    let (callback_id, receiver) = get_callback();

    // Call runtime interface with callback ID
    match lxapp.runtime.request_location(callback_id) {
        Ok(()) => {
            // Wait for result from callback
            match receiver.await {
                Ok(result) => {
                    let mut location = LocationObj::from(result);

                    // Handle coordinate system conversion
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
