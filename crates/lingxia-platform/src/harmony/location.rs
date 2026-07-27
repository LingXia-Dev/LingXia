//! Harmony platform location (GPS) implementation

use log::warn;
use serde_json::json;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::error::PlatformError;
use crate::traits::location::Location;
use lingxia_messaging::{CallbackResult, invoke_callback, register_handler, remove_callback};
use lingxia_webview::platform::harmony::tsfn;

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
    delivered: AtomicBool,
    stopping: AtomicBool,
}

unsafe impl Send for HarmonyLocationContext {}
unsafe impl Sync for HarmonyLocationContext {}

impl Drop for HarmonyLocationContext {
    fn drop(&mut self) {
        unsafe { OH_Location_DestroyRequestConfig(self.request_config) };
    }
}

static LOCATION_CONTEXTS: OnceLock<Mutex<HashMap<usize, Arc<HarmonyLocationContext>>>> =
    OnceLock::new();
static NEXT_LOCATION_TOKEN: AtomicUsize = AtomicUsize::new(1);

fn location_contexts() -> MutexGuard<'static, HashMap<usize, Arc<HarmonyLocationContext>>> {
    LOCATION_CONTEXTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn register_location_context(context: Arc<HarmonyLocationContext>) -> usize {
    let mut contexts = location_contexts();
    loop {
        let token = NEXT_LOCATION_TOKEN.fetch_add(1, Ordering::Relaxed);
        if token != 0 && !contexts.contains_key(&token) {
            contexts.insert(token, context);
            return token;
        }
    }
}

fn remove_location_context(token: usize, context: &Arc<HarmonyLocationContext>) {
    let mut contexts = location_contexts();
    if contexts
        .get(&token)
        .is_some_and(|current| Arc::ptr_eq(current, context))
    {
        contexts.remove(&token);
    }
}

unsafe extern "C" fn handle_location_update(location: *mut Location_Info, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }

    let token = user_data as usize;

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

    let context = {
        let contexts = location_contexts();
        contexts.get(&token).cloned()
    };
    let Some(context) = context else {
        return;
    };

    let callback_id =
        (!context.delivered.swap(true, Ordering::AcqRel)).then_some(context.callback_id);

    if context
        .stopping
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let stop_result = unsafe { OH_Location_StopLocating(context.request_config) };
        if stop_result == LOCATION_SUCCESS {
            remove_location_context(token, &context);
        } else {
            warn!("Failed to stop Harmony location updates: {stop_result}");
            context.stopping.store(false, Ordering::Release);
        }
    }

    let Some(callback_id) = callback_id else {
        return;
    };
    if !lingxia_messaging::invoke_callback(callback_id, Ok(payload_str)) {
        warn!(
            "Location callback {callback_id} not found",
            callback_id = callback_id
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

    async fn request_location(
        &self,
        config: crate::traits::location::LocationRequestConfig,
    ) -> Result<String, PlatformError> {
        let platform = self.clone();
        crate::rt::native_call(|callback_id| {
            let request_config = config.clone();

            let handler_id_cell = Arc::new(AtomicU64::new(0));
            let handler_id_cell_inner = handler_id_cell.clone();

            let handler_platform = platform.clone();
            let handler_config = request_config.clone();

            let handler_id = register_handler(move |result| {
                let handler_id = handler_id_cell_inner.load(Ordering::Relaxed);
                if handler_id != 0 {
                    let _ = remove_callback(handler_id);
                }

                match result {
                    CallbackResult::Success(_) => {
                        if let Err(err) =
                            handler_platform.start_locating(callback_id, handler_config.clone())
                        {
                            warn!(
                                "Harmony location: failed to start after permission granted: {err}"
                            );
                            let _ = invoke_callback(callback_id, Err(1001));
                        }
                    }
                    CallbackResult::Error(code) => {
                        let _ = invoke_callback(callback_id, Err(code));
                    }
                }
            });
            handler_id_cell.store(handler_id, Ordering::Relaxed);

            let handler_id_str = handler_id.to_string();
            if tsfn::call_arkts("requestLocationPermission", &[&handler_id_str]).is_ok() {
                return Ok(());
            }

            let _ = remove_callback(handler_id);
            platform.start_locating(callback_id, request_config)
        })
        .await
    }
}

impl Platform {
    fn start_locating(
        &self,
        callback_id: u64,
        config: crate::traits::location::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        unsafe {
            let request_config = OH_Location_CreateRequestConfig();
            if request_config.is_null() {
                let _ = invoke_callback(callback_id, Err(1001));
                return Ok(());
            }

            let interval = if config.is_high_accuracy { 1 } else { 5 };
            OH_LocationRequestConfig_SetInterval(request_config, interval);

            let context = Arc::new(HarmonyLocationContext {
                callback_id,
                request_config,
                delivered: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
            });
            let context_token = register_location_context(context.clone());
            OH_LocationRequestConfig_SetCallback(
                request_config,
                Some(handle_location_update),
                context_token as *mut c_void,
            );

            let result = OH_Location_StartLocating(request_config);
            if result != LOCATION_SUCCESS {
                remove_location_context(context_token, &context);

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
