use crate::error::PlatformError;
use crate::traits::MediaKind;
use crate::{AppRuntime, AssetFileEntry, MediaRuntime};
use log::warn;
use napi_ohos::JsValue;
use napi_ohos::bindgen_prelude::{Env, Object};
use ohos_raw_sys::*;
use std::collections::HashMap;
use std::ffi::{CString, c_void};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, mpsc};
use std::time::Duration;

pub struct Platform {
    pub data_dir: String,
    pub cache_dir: String,
    pub locale: String,
    resource_manager: Option<*mut NativeResourceManager>,
    // Store the original napi values for cloning
    env: Option<napi_ohos::sys::napi_env>,
    js_resource_manager: Option<napi_ohos::sys::napi_value>,
}

impl Clone for Platform {
    fn clone(&self) -> Self {
        // For cloning, we can recreate the ResourceManager if we have the original values
        let resource_manager = if let (Some(env), Some(js_rm)) =
            (self.env, self.js_resource_manager)
        {
            let native_mgr = unsafe { OH_ResourceManager_InitNativeResourceManager(env, js_rm) };
            if native_mgr.is_null() {
                None
            } else {
                Some(native_mgr)
            }
        } else {
            None
        };

        Platform {
            data_dir: self.data_dir.clone(),
            cache_dir: self.cache_dir.clone(),
            locale: self.locale.clone(),
            resource_manager,
            env: self.env,
            js_resource_manager: self.js_resource_manager,
        }
    }
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

const ZERO_REQUEST_ID: &str = "00000000-0000-0000-0000-000000000000";

struct RequestState {
    sender: Option<mpsc::Sender<i32>>,
    result: Option<i32>,
}

static REQUEST_CHANNELS: LazyLock<Mutex<HashMap<String, RequestState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

unsafe extern "C" fn on_media_copy_prepared(result: i32, request_id: ffi::MediaLibrary_RequestId) {
    let request_key = request_id_to_string(&request_id);
    let mut map = REQUEST_CHANNELS.lock().expect("REQUEST_CHANNELS poisoned");
    let mut remove_entry = false;

    if let Some(state) = map.get_mut(&request_key) {
        if let Some(sender) = state.sender.take() {
            let _ = sender.send(result);
            remove_entry = true;
        } else {
            state.result = Some(result);
        }
    } else {
        map.insert(
            request_key.clone(),
            RequestState {
                sender: None,
                result: Some(result),
            },
        );
    }

    if remove_entry {
        map.remove(&request_key);
    }
}

fn request_id_to_string(id: &ffi::MediaLibrary_RequestId) -> String {
    let mut bytes = Vec::new();
    for &ch in &id.request_id {
        if ch == 0 {
            break;
        }
        bytes.push(ch as u8);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn insert_request_channel(request_key: &str, sender: mpsc::Sender<i32>) {
    let mut map = REQUEST_CHANNELS.lock().expect("REQUEST_CHANNELS poisoned");

    if let Some(state) = map.get_mut(request_key) {
        if let Some(result) = state.result.take() {
            map.remove(request_key);
            drop(map);
            let _ = sender.send(result);
        } else {
            state.sender = Some(sender);
        }
    } else {
        map.insert(
            request_key.to_string(),
            RequestState {
                sender: Some(sender),
                result: None,
            },
        );
    }
}

fn cleanup_request_channel(request_key: &str) {
    let _ = REQUEST_CHANNELS
        .lock()
        .expect("REQUEST_CHANNELS poisoned")
        .remove(request_key);
}

struct ManagerGuard(*mut ffi::OH_MediaAssetManager);

impl Drop for ManagerGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                let code = ffi::OH_MediaAssetManager_Release(self.0);
                if code != ffi::MEDIA_LIBRARY_OK {
                    warn!("Failed to release OH_MediaAssetManager, code={}", code);
                }
            }
        }
    }
}

impl Platform {
    /// Create a new Platform instance
    pub fn new(
        data_dir: String,
        cache_dir: String,
        env: Env,
        resource_manager: Option<Object>,
        locale: String,
    ) -> Result<Self, PlatformError> {
        let (resource_manager_ptr, env_raw, js_rm_raw) =
            if let Some(resource_manager) = resource_manager {
                let env_raw = env.raw();
                let js_rm_raw = resource_manager.raw();

                // Extract the native ResourceManager pointer from the JS object
                let native_mgr =
                    unsafe { OH_ResourceManager_InitNativeResourceManager(env_raw, js_rm_raw) };

                if native_mgr.is_null() {
                    return Err(PlatformError::Platform(
                        "Failed to initialize NativeResourceManager".to_string(),
                    ));
                }

                (Some(native_mgr), Some(env_raw), Some(js_rm_raw))
            } else {
                (None, None, None)
            };

        Ok(Platform {
            data_dir,
            cache_dir,
            locale,
            resource_manager: resource_manager_ptr,
            env: env_raw,
            js_resource_manager: js_rm_raw,
        })
    }

    /// Recursively collect all files from a directory
    fn collect_files_recursively<'a>(
        &'a self,
        dir_path: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, PlatformError>> {
        let mut all_files = Vec::new();

        if let Some(resource_manager) = self.resource_manager {
            // Use ResourceManager to list rawfile directory
            self.collect_files_from_rawfile(resource_manager, dir_path, &mut all_files);
        } else {
            all_files.push(Err(PlatformError::AssetNotFound(
                "ResourceManager not available".to_string(),
            )));
        }

        all_files
    }

    /// Collect files from rawfile using ResourceManager
    fn collect_files_from_rawfile<'a>(
        &'a self,
        resource_manager: *mut NativeResourceManager,
        dir_path: &str,
        all_files: &mut Vec<Result<AssetFileEntry<'a>, PlatformError>>,
    ) {
        let mut dirs_to_process = vec![dir_path.to_string()];

        while let Some(current_dir) = dirs_to_process.pop() {
            // Open directory
            let c_dir_path = match CString::new(current_dir.as_str()) {
                Ok(path) => path,
                Err(_) => continue,
            };

            let raw_dir =
                unsafe { OH_ResourceManager_OpenRawDir(resource_manager, c_dir_path.as_ptr()) };

            if raw_dir.is_null() {
                continue;
            }

            // Get directory count
            let count = unsafe { OH_ResourceManager_GetRawFileCount(raw_dir) };

            for i in 0..count {
                // Get file name
                let file_name_ptr = unsafe { OH_ResourceManager_GetRawFileName(raw_dir, i) };
                if file_name_ptr.is_null() {
                    continue;
                }

                let file_name = unsafe {
                    std::ffi::CStr::from_ptr(file_name_ptr)
                        .to_string_lossy()
                        .to_string()
                };

                if file_name.is_empty() {
                    continue;
                }

                let full_path = if current_dir.is_empty() || current_dir == "/" {
                    file_name.clone()
                } else {
                    format!("{}/{}", current_dir.trim_end_matches('/'), file_name)
                };

                // Check if it's a directory
                let is_directory = unsafe {
                    let c_full_path = CString::new(full_path.as_str()).unwrap_or_default();
                    OH_ResourceManager_IsRawDir(resource_manager, c_full_path.as_ptr())
                };

                if is_directory {
                    // It's a directory, add it to processing queue for recursive processing
                    dirs_to_process.push(full_path);
                } else {
                    // It's a file, try to read it
                    match self.read_asset(&full_path) {
                        Ok(reader) => {
                            all_files.push(Ok(AssetFileEntry {
                                path: full_path,
                                reader,
                            }));
                        }
                        Err(_) => {
                            // Skip files that can't be read
                        }
                    }
                }
            }

            // Close directory
            unsafe { OH_ResourceManager_CloseRawDir(raw_dir) };
        }
    }
    /// Open a raw file using the ResourceManager
    fn open_raw_file(&self, path: &str) -> Option<*mut RawFile> {
        if let Some(resource_manager) = self.resource_manager {
            let c_path = CString::new(path).ok()?;
            let raw_file =
                unsafe { OH_ResourceManager_OpenRawFile(resource_manager, c_path.as_ptr()) };
            if raw_file.is_null() {
                None
            } else {
                Some(raw_file)
            }
        } else {
            None
        }
    }

    pub(crate) fn copy_album_media_to_file_impl(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        if uri.is_empty() {
            return Err(PlatformError::Platform("URI must not be empty".to_string()));
        }

        if dest_path.as_os_str().is_empty() {
            return Err(PlatformError::Platform(
                "Destination path must not be empty".to_string(),
            ));
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to create destination directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let manager_ptr = unsafe { ffi::OH_MediaAssetManager_Create() };
        if manager_ptr.is_null() {
            return Err(PlatformError::Platform(
                "Failed to create OH_MediaAssetManager".to_string(),
            ));
        }

        let manager = ManagerGuard(manager_ptr);

        let uri_cstr = CString::new(uri)
            .map_err(|_| PlatformError::Platform("URI contains interior null byte".to_string()))?;
        let dest_string = dest_path.to_string_lossy();
        let dest_cstr = CString::new(dest_string.as_bytes()).map_err(|_| {
            PlatformError::Platform("Destination path contains interior null byte".to_string())
        })?;

        let request_options = ffi::MediaLibrary_RequestOptions {
            deliveryMode: ffi::MEDIA_LIBRARY_HIGH_QUALITY_MODE,
        };

        let (tx, rx) = mpsc::channel();

        let request_id = unsafe {
            match kind {
                MediaKind::Video => ffi::OH_MediaAssetManager_RequestVideoForPath(
                    manager.0,
                    uri_cstr.as_ptr(),
                    request_options,
                    dest_cstr.as_ptr(),
                    Some(on_media_copy_prepared),
                ),
                _ => ffi::OH_MediaAssetManager_RequestImageForPath(
                    manager.0,
                    uri_cstr.as_ptr(),
                    request_options,
                    dest_cstr.as_ptr(),
                    Some(on_media_copy_prepared),
                ),
            }
        };

        let request_key = request_id_to_string(&request_id);
        if request_key.is_empty() || request_key == ZERO_REQUEST_ID {
            return Err(PlatformError::Platform(
                "Media request failed to start".to_string(),
            ));
        }

        insert_request_channel(&request_key, tx.clone());

        let result_code = match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(code) => code,
            Err(_) => {
                cleanup_request_channel(&request_key);
                return Err(PlatformError::Platform(
                    "Timed out waiting for media request".to_string(),
                ));
            }
        };

        if result_code != ffi::MEDIA_LIBRARY_OK {
            cleanup_request_channel(&request_key);
            return Err(PlatformError::Platform(format!(
                "Media request failed with code {}",
                result_code
            )));
        }

        cleanup_request_channel(&request_key);

        if !dest_path.exists() {
            return Err(PlatformError::Platform(
                "Destination file was not created".to_string(),
            ));
        }

        Ok(())
    }
}

impl AppRuntime for Platform {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        if let Some(raw_file) = self.open_raw_file(path) {
            let file_size = unsafe { OH_ResourceManager_GetRawFileSize(raw_file) };

            if file_size <= 0 {
                unsafe { OH_ResourceManager_CloseRawFile(raw_file) };
                return Err(PlatformError::AssetNotFound(format!(
                    "Asset '{}' is empty or not found",
                    path
                )));
            }

            // Read the entire file content as bytes
            let mut buffer = vec![0u8; file_size as usize];
            let bytes_read = unsafe {
                OH_ResourceManager_ReadRawFile(
                    raw_file,
                    buffer.as_mut_ptr() as *mut c_void,
                    file_size as usize,
                )
            };

            // Close the file
            unsafe { OH_ResourceManager_CloseRawFile(raw_file) };

            if bytes_read != file_size as i32 {
                return Err(PlatformError::AssetNotFound(format!(
                    "Failed to read complete asset '{}': expected {} bytes, got {} bytes",
                    path, file_size, bytes_read
                )));
            }

            // Truncate buffer to actual bytes read
            buffer.truncate(bytes_read as usize);
            Ok(Box::new(Cursor::new(buffer)))
        } else {
            Err(PlatformError::AssetNotFound(format!(
                "Asset '{}' not found or ResourceManager not available",
                path
            )))
        }
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        // Collect all files recursively from the directory
        let files = self.collect_files_recursively(asset_dir);
        Box::new(files.into_iter())
    }

    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
    }

    fn exit_app(&self) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("exitApp", &[])
            .map_err(|e| PlatformError::Platform(format!("Failed to exit app: {}", e)))
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn open_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("openLxApp", &[&appid, &path])
            .map_err(|e| PlatformError::Platform(format!("Failed to open lxapp: {}", e)))
    }

    fn close_lxapp(&self, appid: String) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("closeLxApp", &[&appid])
            .map_err(|e| PlatformError::Platform(format!("Failed to close lxapp: {}", e)))
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: crate::traits::AnimationType,
    ) -> Result<(), PlatformError> {
        let anim_type_int = animation_type as i32;
        lingxia_webview::tsfn::call_arkts("navigate", &[&appid, &path, &anim_type_int.to_string()])
            .map_err(|_| {
                PlatformError::Platform(format!(
                    "Failed to navigate: appid={}, path={}, animation_type={:?}",
                    appid, path, animation_type
                ))
            })
    }

    fn launch_with_url(&self, url: String) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("launchWithUrl", &[&url])
            .map_err(|e| PlatformError::Platform(format!("Failed to launch with url: {}", e)))
    }
}

#[allow(non_camel_case_types)]
mod ffi {
    use std::os::raw::{c_char, c_int};

    #[repr(C)]
    pub struct OH_MediaAssetManager {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct MediaLibrary_RequestId {
        pub request_id: [c_char; 37],
    }

    pub type MediaLibrary_ErrorCode = c_int;

    pub const MEDIA_LIBRARY_OK: MediaLibrary_ErrorCode = 0;

    pub const MEDIA_LIBRARY_HIGH_QUALITY_MODE: c_int = 1;

    #[allow(non_snake_case)]
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct MediaLibrary_RequestOptions {
        pub deliveryMode: c_int,
    }

    pub type OH_MediaLibrary_OnDataPrepared =
        Option<unsafe extern "C" fn(result: c_int, request_id: MediaLibrary_RequestId)>;

    #[link(name = "media_asset_manager")]
    unsafe extern "C" {
        pub fn OH_MediaAssetManager_Create() -> *mut OH_MediaAssetManager;

        pub fn OH_MediaAssetManager_RequestImageForPath(
            manager: *mut OH_MediaAssetManager,
            uri: *const c_char,
            requestOptions: MediaLibrary_RequestOptions,
            destPath: *const c_char,
            callback: OH_MediaLibrary_OnDataPrepared,
        ) -> MediaLibrary_RequestId;

        pub fn OH_MediaAssetManager_RequestVideoForPath(
            manager: *mut OH_MediaAssetManager,
            uri: *const c_char,
            requestOptions: MediaLibrary_RequestOptions,
            destPath: *const c_char,
            callback: OH_MediaLibrary_OnDataPrepared,
        ) -> MediaLibrary_RequestId;

        pub fn OH_MediaAssetManager_Release(
            manager: *mut OH_MediaAssetManager,
        ) -> MediaLibrary_ErrorCode;
    }
}
