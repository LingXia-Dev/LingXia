//! Harmony platform location (GPS) implementation

use log::warn;
use serde_json::json;
use std::ffi::c_void;

use crate::error::PlatformError;
use crate::traits::Location;
use lingxia_messaging::invoke_callback;

use super::Platform;

#[allow(non_camel_case_types)]
type Location_ResultCode = i32;

const LOCATION_SUCCESS: Location_ResultCode = 0;

#[allow(non_camel_case_types)]
type Location_InfoCallback = Option<unsafe extern "C" fn(*mut Location_Info, *mut c_void)>;

#[allow(non_camel_case_types)]
#[repr(C)]
struct Location_Info {
    _private: [u8; 0],
}

#[allow(non_camel_case_types)]
#[repr(C)]
struct Location_RequestConfig {
    _private: [u8; 0],
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
#[derive(Clone, Copy)]
struct Location_BasicInfo {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    accuracy: f64,
    speed: f64,
    direction: f64,
    timeForFix: i64,
    timeSinceBoot: i64,
    altitudeAccuracy: f64,
    speedAccuracy: f64,
    directionAccuracy: f64,
    uncertaintyOfTimeSinceBoot: i64,
    locationSourceType: i32,
}

#[link(name = "location_ndk")]
unsafe extern "C" {
    fn OH_Location_IsLocatingEnabled(enabled: *mut bool) -> Location_ResultCode;
    fn OH_Location_StartLocating(
        request_config: *const Location_RequestConfig,
    ) -> Location_ResultCode;
    fn OH_Location_StopLocating(
        request_config: *const Location_RequestConfig,
    ) -> Location_ResultCode;

    fn OH_Location_CreateRequestConfig() -> *mut Location_RequestConfig;
    fn OH_Location_DestroyRequestConfig(request_config: *mut Location_RequestConfig);
    fn OH_LocationRequestConfig_SetCallback(
        request_config: *mut Location_RequestConfig,
        callback: Location_InfoCallback,
        user_data: *mut c_void,
    );
    fn OH_LocationRequestConfig_SetInterval(
        request_config: *mut Location_RequestConfig,
        interval: i32,
    );

    fn OH_LocationInfo_GetBasicInfo(location: *mut Location_Info) -> Location_BasicInfo;
}

struct HarmonyLocationContext {
    callback_id: u64,
    request_config: *mut Location_RequestConfig,
}

impl HarmonyLocationContext {
    fn new(
        callback_id: u64,
        request_config: *mut Location_RequestConfig,
    ) -> *mut HarmonyLocationContext {
        Box::into_raw(Box::new(Self {
            callback_id,
            request_config,
        }))
    }

    fn from_raw(ptr: *mut c_void) -> Box<Self> {
        unsafe { Box::from_raw(ptr as *mut Self) }
    }
}

unsafe extern "C" fn handle_location_update(location: *mut Location_Info, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }

    let ctx = HarmonyLocationContext::from_raw(user_data);

    let basic = unsafe { OH_LocationInfo_GetBasicInfo(location) };

    let payload = json!({
        "latitude": basic.latitude,
        "longitude": basic.longitude,
        "speed": basic.speed,
        "accuracy": basic.accuracy,
        "altitude": basic.altitude,
        "vertical_accuracy": basic.altitudeAccuracy,
        "horizontal_accuracy": basic.accuracy,
    });

    let payload_str = match serde_json::to_string(&payload) {
        Ok(json) => json,
        Err(e) => {
            warn!("Failed to serialize location payload: {}", e);
            "{}".to_string()
        }
    };

    if unsafe { OH_Location_StopLocating(ctx.request_config) } != LOCATION_SUCCESS {
        warn!("Failed to stop Harmony location updates");
    }
    unsafe { OH_Location_DestroyRequestConfig(ctx.request_config) };

    if !lingxia_messaging::invoke_callback(ctx.callback_id, Ok(payload_str)) {
        warn!(
            "Location callback {callback_id} not found",
            callback_id = ctx.callback_id
        );
    }
}

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        let mut enabled = false;
        let result = unsafe { OH_Location_IsLocatingEnabled(&mut enabled as *mut bool) };
        if result == LOCATION_SUCCESS {
            Ok(enabled)
        } else {
            Err(PlatformError::Platform(format!(
                "OH_Location_IsLocatingEnabled failed with code {}",
                result
            )))
        }
    }

    fn request_location(
        &self,
        callback_id: u64,
        config: crate::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        unsafe {
            let request_config = OH_Location_CreateRequestConfig();
            if request_config.is_null() {
                return Err(PlatformError::Platform(
                    "Failed to create location request config".to_string(),
                ));
            }

            // Set interval based on accuracy requirements
            let interval = if config.is_high_accuracy { 1 } else { 5 };
            OH_LocationRequestConfig_SetInterval(request_config, interval);

            let context_ptr = HarmonyLocationContext::new(callback_id, request_config);
            OH_LocationRequestConfig_SetCallback(
                request_config,
                Some(handle_location_update),
                context_ptr as *mut c_void,
            );

            let result = OH_Location_StartLocating(request_config);
            if result != LOCATION_SUCCESS {
                OH_Location_DestroyRequestConfig(request_config);
                drop(Box::from_raw(context_ptr));

                let error_code: u32 = if result == 201 {
                    3002 // Permission denied
                } else {
                    1001 // General failure
                };

                invoke_callback(callback_id, Err(error_code));
                return Ok(());
            }

            Ok(())
        }
    }
}
