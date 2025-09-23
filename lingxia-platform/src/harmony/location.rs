//! Harmony platform location (GPS) implementation

use log::{info, warn};
use serde_json::json;
use std::ffi::c_void;

use crate::error::PlatformError;
use crate::traits::Location;

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

    unsafe fn from_raw(ptr: *mut c_void) -> Box<Self> {
        Box::from_raw(ptr as *mut Self)
    }
}

unsafe extern "C" fn handle_location_update(location: *mut Location_Info, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }

    let ctx = unsafe { HarmonyLocationContext::from_raw(user_data) };

    let basic = unsafe { OH_LocationInfo_GetBasicInfo(location) };

    // 打印 HarmonyOS 原生位置数据用于调试
    info!("HarmonyOS Location Basic Info:");
    info!("  latitude: {}", basic.latitude);
    info!("  longitude: {}", basic.longitude);
    info!("  speed: {}", basic.speed);
    info!("  accuracy: {}", basic.accuracy);
    info!("  altitude: {}", basic.altitude);
    info!("  altitudeAccuracy: {}", basic.altitudeAccuracy);

    let payload = json!({
        "latitude": basic.latitude,
        "longitude": basic.longitude,
        "speed": basic.speed,
        "accuracy": basic.accuracy,
        "altitude": basic.altitude,
        "vertical_accuracy": basic.altitudeAccuracy,
        "horizontal_accuracy": basic.accuracy,
        // HarmonyOS 原生返回 WGS84 坐标，不需要硬编码 coordinate_system
        // 让上层 logic 根据用户请求的 type 参数决定是否需要坐标转换
    });

    let payload_str = match serde_json::to_string(&payload) {
        Ok(json) => json,
        Err(e) => {
            warn!("Failed to serialize location payload: {}", e);
            "{}".to_string()
        }
    };

    info!("Generated JSON payload: {}", payload_str);

    if unsafe { OH_Location_StopLocating(ctx.request_config) } != LOCATION_SUCCESS {
        warn!("Failed to stop Harmony location updates");
    }
    unsafe { OH_Location_DestroyRequestConfig(ctx.request_config) };

    if !lingxia_messaging::invoke_callback(ctx.callback_id, true, payload_str) {
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

    fn request_location(&self, callback_id: u64) -> Result<(), PlatformError> {
        info!(
            "Starting location request with callback_id: {}",
            callback_id
        );

        unsafe {
            let request_config = OH_Location_CreateRequestConfig();
            if request_config.is_null() {
                return Err(PlatformError::Platform(
                    "Failed to create location request config".to_string(),
                ));
            }
            info!("Location request config created successfully");

            OH_LocationRequestConfig_SetInterval(request_config, 1);
            info!("Location request interval set to 1 second");

            let context_ptr = HarmonyLocationContext::new(callback_id, request_config);
            OH_LocationRequestConfig_SetCallback(
                request_config,
                Some(handle_location_update),
                context_ptr as *mut c_void,
            );
            info!("Location request callback set");

            info!("Calling OH_Location_StartLocating...");
            let result = OH_Location_StartLocating(request_config);
            info!("OH_Location_StartLocating returned code: {}", result);

            if result != LOCATION_SUCCESS {
                OH_Location_DestroyRequestConfig(request_config);
                drop(Box::from_raw(context_ptr));
                return Err(PlatformError::Platform(format!(
                    "OH_Location_StartLocating failed with code {}",
                    result
                )));
            }

            info!("Location request started successfully");
            Ok(())
        }
    }
}
