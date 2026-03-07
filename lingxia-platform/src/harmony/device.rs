//! Harmony platform device implementation
use crate::error::PlatformError;
use crate::traits::device::{Device, DeviceHardware};
use crate::traits::secure_store::SecureStore;
use crate::{DeviceInfo, ScreenInfo};
use log::warn;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::sync::Once;

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

// Harmony Asset Store (secure persistence)
#[allow(non_camel_case_types)]
type Asset_Tag = u32;

const ASSET_TAG_SECRET: Asset_Tag = (3 << 28) | 0x01;
const ASSET_TAG_ALIAS: Asset_Tag = (3 << 28) | 0x02;
const ASSET_TAG_SYNC_TYPE: Asset_Tag = (2 << 28) | 0x10;
const ASSET_TAG_IS_PERSISTENT: Asset_Tag = (1 << 28) | 0x11;
const ASSET_TAG_RETURN_TYPE: Asset_Tag = (2 << 28) | 0x40;
const ASSET_TAG_CONFLICT_RESOLUTION: Asset_Tag = (2 << 28) | 0x44;
const ASSET_TAG_ACCESSIBILITY: Asset_Tag = (2 << 28) | 0x03;
const ASSET_TAG_AUTH_TYPE: Asset_Tag = (2 << 28) | 0x05;

const ASSET_SYNC_TYPE_THIS_DEVICE: u32 = 1;
const ASSET_RETURN_ALL: u32 = 0;
const ASSET_CONFLICT_OVERWRITE: u32 = 0;
const ASSET_ACCESSIBILITY_DEVICE_FIRST_UNLOCKED: u32 = 1;
const ASSET_AUTH_TYPE_NONE: u32 = 0;

const ASSET_SUCCESS: i32 = 0;
const ASSET_PERMISSION_DENIED: i32 = 201;
const ASSET_NOT_FOUND: i32 = 24000002;
const ASSET_DUPLICATED: i32 = 24000003;

fn warn_non_persistent_asset_once() {
    static WARN_ONCE: Once = Once::new();
    WARN_ONCE.call_once(|| {
        warn!(
            "Asset persistent storage is not allowed (permission denied). Falling back to non-persistent mode; data may be cleared on uninstall."
        );
    });
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
struct Asset_Blob {
    size: u32,
    data: *mut u8,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
union Asset_Value {
    boolean: bool,
    u32_: u32,
    blob: Asset_Blob,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
struct Asset_Attr {
    tag: u32,
    value: Asset_Value,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct Asset_Result {
    count: u32,
    attrs: *mut Asset_Attr,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct Asset_ResultSet {
    count: u32,
    results: *mut Asset_Result,
}

#[link(name = "asset_ndk.z")]
unsafe extern "C" {
    fn OH_Asset_Add(attributes: *const Asset_Attr, attr_cnt: u32) -> i32;
    fn OH_Asset_Remove(query: *const Asset_Attr, query_cnt: u32) -> i32;
    fn OH_Asset_Update(
        query: *const Asset_Attr,
        query_cnt: u32,
        attributes_to_update: *const Asset_Attr,
        update_cnt: u32,
    ) -> i32;
    fn OH_Asset_Query(
        query: *const Asset_Attr,
        query_cnt: u32,
        result_set: *mut Asset_ResultSet,
    ) -> i32;
    fn OH_Asset_ParseAttr(result: *const Asset_Result, tag: Asset_Tag) -> *const Asset_Attr;
    fn OH_Asset_FreeResultSet(result_set: *mut Asset_ResultSet);
}

fn asset_attr_bytes(tag: Asset_Tag, bytes: &[u8]) -> Asset_Attr {
    Asset_Attr {
        tag,
        value: Asset_Value {
            blob: Asset_Blob {
                size: bytes.len() as u32,
                data: bytes.as_ptr() as *mut u8,
            },
        },
    }
}

fn asset_attr_bool(tag: Asset_Tag, value: bool) -> Asset_Attr {
    Asset_Attr {
        tag,
        value: Asset_Value { boolean: value },
    }
}

fn asset_attr_u32(tag: Asset_Tag, value: u32) -> Asset_Attr {
    Asset_Attr {
        tag,
        value: Asset_Value { u32_: value },
    }
}

fn asset_attrs_for_value(
    alias: &str,
    value: &[u8],
    persistent: bool,
    include_alias: bool,
    include_conflict: bool,
) -> Vec<Asset_Attr> {
    let mut attrs = Vec::with_capacity(6);
    if include_alias {
        attrs.push(asset_attr_bytes(ASSET_TAG_ALIAS, alias.as_bytes()));
    }
    attrs.push(asset_attr_bytes(ASSET_TAG_SECRET, value));
    attrs.push(asset_attr_u32(
        ASSET_TAG_ACCESSIBILITY,
        ASSET_ACCESSIBILITY_DEVICE_FIRST_UNLOCKED,
    ));
    attrs.push(asset_attr_u32(ASSET_TAG_AUTH_TYPE, ASSET_AUTH_TYPE_NONE));
    attrs.push(asset_attr_u32(
        ASSET_TAG_SYNC_TYPE,
        ASSET_SYNC_TYPE_THIS_DEVICE,
    ));
    if include_conflict {
        attrs.push(asset_attr_u32(
            ASSET_TAG_CONFLICT_RESOLUTION,
            ASSET_CONFLICT_OVERWRITE,
        ));
    }
    if persistent {
        attrs.push(asset_attr_bool(ASSET_TAG_IS_PERSISTENT, true));
    }
    attrs
}

struct AssetResultSetGuard {
    inner: Asset_ResultSet,
}

impl Drop for AssetResultSetGuard {
    fn drop(&mut self) {
        unsafe {
            OH_Asset_FreeResultSet(&mut self.inner as *mut Asset_ResultSet);
        }
    }
}

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

fn parse_openharmony_version_from_full_name(full_name: &str) -> Option<String> {
    let s = full_name.trim();
    let start = s
        .char_indices()
        .find_map(|(idx, ch)| if ch.is_ascii_digit() { Some(idx) } else { None })?;
    let version = s[start..].trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

// Platform Device trait implementation - direct implementation without delegation
impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        // Use Harmony C DeviceInfo API (market name preferred for model)
        let brand = call_cstr(OH_GetBrand).unwrap_or_else(|| "Unknown".to_string());
        let model = call_cstr(OH_GetProductModel).unwrap_or_else(|| "Unknown".to_string());
        let market_name = call_cstr(OH_GetMarketName).unwrap_or_else(|| model.clone());
        let os_name = "OpenHarmony".to_string();
        let distribution_os_version = call_cstr(OH_GetDistributionOSVersion);
        let os_full_name = call_cstr(OH_GetOSFullName);
        let os_version = os_full_name
            .as_deref()
            .and_then(parse_openharmony_version_from_full_name)
            .or(distribution_os_version.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        DeviceInfo {
            brand,
            model,
            market_name,
            os_name,
            os_version,
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

impl SecureStore for Platform {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let alias_attr = asset_attr_bytes(ASSET_TAG_ALIAS, key.as_bytes());
        let return_type_attr = asset_attr_u32(ASSET_TAG_RETURN_TYPE, ASSET_RETURN_ALL);
        let query = [alias_attr, return_type_attr];

        let mut result_set = MaybeUninit::<Asset_ResultSet>::zeroed();
        let code =
            unsafe { OH_Asset_Query(query.as_ptr(), query.len() as u32, result_set.as_mut_ptr()) };

        if code == ASSET_NOT_FOUND {
            return Ok(None);
        }
        if code != ASSET_SUCCESS {
            return Err(PlatformError::Platform(format!(
                "OH_Asset_Query failed for key {}: code {}",
                key, code
            )));
        }

        let result_set = unsafe { result_set.assume_init() };
        let guard = AssetResultSetGuard { inner: result_set };

        if guard.inner.count == 0 || guard.inner.results.is_null() {
            return Ok(None);
        }

        // Take the first result.
        let first_result = unsafe { guard.inner.results.as_ref() }
            .ok_or_else(|| PlatformError::Platform("Asset result pointer is null".to_string()))?;

        let attr_ptr =
            unsafe { OH_Asset_ParseAttr(first_result as *const Asset_Result, ASSET_TAG_SECRET) };
        if attr_ptr.is_null() {
            return Ok(None);
        }

        let blob = unsafe { (*attr_ptr).value.blob };
        if blob.size == 0 || blob.data.is_null() {
            return Ok(Some(Vec::new()));
        }

        let data =
            unsafe { std::slice::from_raw_parts(blob.data as *const u8, blob.size as usize) };
        Ok(Some(data.to_vec()))
    }

    fn write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        let query = [asset_attr_bytes(ASSET_TAG_ALIAS, key.as_bytes())];
        let mut used_non_persistent_mode = false;
        let success_with_warning = |used_non_persistent_mode: bool| {
            if used_non_persistent_mode {
                warn_non_persistent_asset_once();
            }
            Ok(())
        };

        // 1) Try update with minimal attrs allowed by update API.
        // Keep update payload limited to SECRET to avoid parameter validation failures (401).
        let update_attrs = [asset_attr_bytes(ASSET_TAG_SECRET, value)];
        let update_code = unsafe {
            OH_Asset_Update(
                query.as_ptr(),
                query.len() as u32,
                update_attrs.as_ptr(),
                update_attrs.len() as u32,
            )
        };

        let mut persistence_allowed = true;
        if update_code == ASSET_PERMISSION_DENIED {
            persistence_allowed = false;
            used_non_persistent_mode = true;
        }

        if update_code == ASSET_SUCCESS {
            return Ok(());
        }
        if update_code != ASSET_NOT_FOUND && update_code != ASSET_PERMISSION_DENIED {
            warn!(
                "OH_Asset_Update failed for key {}: code {}",
                key, update_code
            );
        }

        // 2) Try add (persistent if allowed)
        let mut add_attrs = asset_attrs_for_value(key, value, persistence_allowed, true, true);
        let mut add_code = unsafe { OH_Asset_Add(add_attrs.as_ptr(), add_attrs.len() as u32) };

        if add_code == ASSET_PERMISSION_DENIED && persistence_allowed {
            // Retry without persistence if permission denied
            used_non_persistent_mode = true;
            add_attrs = asset_attrs_for_value(key, value, false, true, true);
            add_code = unsafe { OH_Asset_Add(add_attrs.as_ptr(), add_attrs.len() as u32) };
        }

        if add_code == ASSET_SUCCESS {
            return success_with_warning(used_non_persistent_mode);
        }

        if add_code == ASSET_DUPLICATED {
            // Duplicate: try update again (respecting persistence_allowed)
            let update_attrs_retry = [asset_attr_bytes(ASSET_TAG_SECRET, value)];
            let retry_code = unsafe {
                OH_Asset_Update(
                    query.as_ptr(),
                    query.len() as u32,
                    update_attrs_retry.as_ptr(),
                    update_attrs_retry.len() as u32,
                )
            };
            if retry_code == ASSET_SUCCESS {
                return success_with_warning(used_non_persistent_mode);
            }
            return Err(PlatformError::Platform(format!(
                "OH_Asset_Update after duplicate failed for key {}: code {}",
                key, retry_code
            )));
        }

        Err(PlatformError::Platform(format!(
            "OH_Asset_Add failed for key {}: code {}",
            key, add_code
        )))
    }

    fn delete(&self, key: &str) -> Result<(), PlatformError> {
        let query = [asset_attr_bytes(ASSET_TAG_ALIAS, key.as_bytes())];
        let code = unsafe { OH_Asset_Remove(query.as_ptr(), query.len() as u32) };
        if code == ASSET_SUCCESS || code == ASSET_NOT_FOUND {
            return Ok(());
        }

        Err(PlatformError::Platform(format!(
            "OH_Asset_Remove failed for key {}: code {}",
            key, code
        )))
    }
}

impl DeviceHardware for Platform {
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if pages <= 0 || page_size <= 0 {
            return Err(PlatformError::Platform(
                "sysconf failed to read memory info".to_string(),
            ));
        }
        Ok((pages as u64) * (page_size as u64))
    }

    fn get_cpu_count(&self) -> usize {
        // Use _SC_NPROCESSORS_CONF to get total configured processors (including offline ones)
        let count = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_CONF) };
        if count > 0 {
            count as usize
        } else {
            // Fallback to 1 for stability
            1
        }
    }

    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        let total_bytes = get_total_storage_bytes(&self.data_dir)
            .or_else(|| get_total_storage_bytes("/"))
            .ok_or_else(|| {
                PlatformError::Platform("statvfs failed to read storage size".to_string())
            })?;
        Ok(total_bytes)
    }
}

fn get_total_storage_bytes(path: &str) -> Option<u64> {
    let c_path = CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let code = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat as *mut libc::statvfs) };
    if code != 0 {
        return None;
    }
    Some(stat.f_blocks as u64 * stat.f_frsize as u64)
}
