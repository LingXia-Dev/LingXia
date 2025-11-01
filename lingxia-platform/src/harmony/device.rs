//! Harmony platform device implementation
use crate::error::PlatformError;
use crate::traits::Device;
use crate::{DeviceInfo, ScreenInfo};
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

// Harmony Display Manager C API
#[link(name = "native_display_manager")]
unsafe extern "C" {
    fn OH_NativeDisplayManager_GetDefaultDisplayWidth(displayWidth: *mut i32) -> i32;
    fn OH_NativeDisplayManager_GetDefaultDisplayHeight(displayHeight: *mut i32) -> i32;
    fn OH_NativeDisplayManager_GetDefaultDisplayDensityPixels(densityPixels: *mut f32) -> i32;
}

// Harmony DeviceInfo C API
#[link(name = "deviceinfo_ndk.z")]
#[allow(dead_code)]
unsafe extern "C" {
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
        let market_name = call_cstr(OH_GetMarketName).unwrap_or_else(|| model.clone());
        let system = call_cstr(OH_GetOSFullName).unwrap_or_else(|| "Unknown".to_string());
        DeviceInfo {
            brand,
            model,
            market_name,
            system,
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        // Harmony: Use C Display Manager API to synchronously get display metrics
        // width/height are physical pixels, densityPixels is the virtual pixel ratio (like Android density)
        let mut width_px: i32 = 0;
        let mut height_px: i32 = 0;
        let mut density_pixels: f32 = 1.0;

        unsafe {
            // Ignore error codes here and fall back to defaults if calls fail
            let _ = OH_NativeDisplayManager_GetDefaultDisplayWidth(&mut width_px as *mut i32);
            let _ = OH_NativeDisplayManager_GetDefaultDisplayHeight(&mut height_px as *mut i32);
            let _ = OH_NativeDisplayManager_GetDefaultDisplayDensityPixels(
                &mut density_pixels as *mut f32,
            );
        }

        let density = if density_pixels > 0.0 {
            density_pixels as f64
        } else {
            1.0
        };
        let width = ((width_px as f64) / density).round();
        let height = ((height_px as f64) / density).round();

        ScreenInfo {
            width,
            height,
            scale: density,
        }
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
