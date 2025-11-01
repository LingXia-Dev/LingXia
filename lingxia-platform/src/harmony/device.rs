//! Harmony platform device implementation

use crate::DeviceInfo;
use crate::error::PlatformError;
use crate::traits::Device;
use std::os::raw::c_char;

use super::Platform;

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy)]
struct Vibrator_Attribute {
    vibrator_id: i32,
    usage: i32,
}

#[link(name = "ohvibrator.z")]
unsafe extern "C" {
    fn OH_Vibrator_PlayVibration(duration: i32, attribute: Vibrator_Attribute) -> i32;
}

// Harmony DeviceInfo C API
#[link(name = "deviceinfo_ndk.z")]
#[allow(dead_code)]
unsafe extern "C" {
    fn OH_GetDeviceType() -> *const c_char;
    fn OH_GetManufacture() -> *const c_char;
    fn OH_GetBrand() -> *const c_char;
    fn OH_GetMarketName() -> *const c_char;
    fn OH_GetProductSeries() -> *const c_char;
    fn OH_GetProductModel() -> *const c_char;
    fn OH_GetSoftwareModel() -> *const c_char;
    fn OH_GetHardwareModel() -> *const c_char;
    fn OH_GetBootloaderVersion() -> *const c_char;
    fn OH_GetAbiList() -> *const c_char;
    fn OH_GetSecurityPatchTag() -> *const c_char;
    fn OH_GetDisplayVersion() -> *const c_char;
    fn OH_GetIncrementalVersion() -> *const c_char;
    fn OH_GetOsReleaseType() -> *const c_char;
    fn OH_GetOSFullName() -> *const c_char;
    fn OH_GetVersionId() -> *const c_char;
    fn OH_GetBuildType() -> *const c_char;
    fn OH_GetBuildUser() -> *const c_char;
    fn OH_GetBuildHost() -> *const c_char;
    fn OH_GetBuildTime() -> *const c_char;
    fn OH_GetBuildRootHash() -> *const c_char;
    fn OH_GetDistributionOSName() -> *const c_char;
    fn OH_GetDistributionOSVersion() -> *const c_char;
    fn OH_GetDistributionOSReleaseType() -> *const c_char;
}

const VIBRATION_DURATION_SHORT_MS: i32 = 15;
const VIBRATION_DURATION_LONG_MS: i32 = 400;
const DEFAULT_VIBRATOR_ID: i32 = 0;
const VIBRATOR_USAGE_ALARM: i32 = 1;

/// Convert C const char* to Rust String
fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        let s = std::ffi::CStr::from_ptr(ptr)
            .to_string_lossy()
            .trim()
            .to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

/// Call a 0-arg C function that returns const char* and convert to String
fn call_cstr(f: unsafe extern "C" fn() -> *const c_char) -> Option<String> {
    let p = unsafe { f() };
    cstr_to_string(p)
}

// Platform Device trait implementation - direct implementation without delegation
impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        // Use Harmony C DeviceInfo API (market name preferred for model)
        let brand = call_cstr(OH_GetBrand).unwrap_or_else(|| "Unknown".to_string());
        let model = call_cstr(OH_GetProductModel).unwrap_or_else(|| "Unknown".to_string());
        let system = call_cstr(OH_GetOSFullName).unwrap_or_else(|| "Unknown".to_string());

        DeviceInfo {
            brand,
            model,
            system,
        }
    }

    fn screen_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("getScreenInfo", &[&callback_id.to_string()]).map_err(
            |e| {
                // Send error via callback
                lingxia_messaging::invoke_callback(
                    callback_id,
                    false,
                    format!("Failed to get screen info: {}", e),
                );
                PlatformError::Platform(format!("Failed to get screen info: {}", e))
            },
        )
    }

    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        let duration = if long {
            VIBRATION_DURATION_LONG_MS
        } else {
            VIBRATION_DURATION_SHORT_MS
        };

        let attribute = Vibrator_Attribute {
            vibrator_id: DEFAULT_VIBRATOR_ID,
            usage: VIBRATOR_USAGE_ALARM,
        };

        let result = unsafe { OH_Vibrator_PlayVibration(duration, attribute) };
        if result == 0 {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to vibrate via OH_Vibrator_PlayVibration: error code {}",
                result
            )))
        }
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("makePhoneCall", &[phone_number])
            .map_err(|e| PlatformError::Platform(format!("Failed to make phone call: {}", e)))
    }
}
