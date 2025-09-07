use crate::error::PlatformError;
use crate::{AppRuntime, AssetFileEntry, DeviceInfo, NavigationType};
use napi_ohos::JsValue;
use napi_ohos::bindgen_prelude::{Env, Object};
use ohos_raw_sys::*;
use std::ffi::{CString, c_void};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::process::Command;

pub struct Platform {
    pub data_dir: String,
    pub cache_dir: String,
    resource_manager: Option<*mut NativeResourceManager>,
    device_info: DeviceInfo,
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
            resource_manager,
            device_info: self.device_info.clone(),
            env: self.env,
            js_resource_manager: self.js_resource_manager,
        }
    }
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

/// Get system parameter using param command
fn get_system_param(param_name: &str) -> Option<String> {
    match Command::new("param").arg("get").arg(param_name).output() {
        Ok(output) => {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !result.is_empty() {
                    Some(result)
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

impl Platform {
    /// Create a new Platform instance
    pub fn new(
        data_dir: String,
        cache_dir: String,
        env: Env,
        resource_manager: Option<Object>,
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

        // Get device information using param commands during initialization
        let brand = get_system_param("const.product.brand").unwrap_or_else(|| "HUAWEI".to_string());

        let model =
            get_system_param("const.product.model").unwrap_or_else(|| "Unknown".to_string());

        let os_version = get_system_param("const.product.os.dist.version")
            .unwrap_or_else(|| "Unknown".to_string());

        // Construct system string with HarmonyOS version
        let system = format!("HarmonyOS {}", os_version);

        let device_info = DeviceInfo {
            brand,
            model,
            system,
        };

        Ok(Platform {
            data_dir,
            cache_dir,
            resource_manager: resource_manager_ptr,
            device_info,
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

    fn device_info(&self) -> DeviceInfo {
        self.device_info.clone()
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
        navigation_type: NavigationType,
    ) -> Result<(), PlatformError> {
        let nav_type_int = navigation_type as i32;
        lingxia_webview::tsfn::call_arkts("navigate", &[&appid, &path, &nav_type_int.to_string()])
            .map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to navigate: appid={}, path={}, navigation_type={:?}",
                    appid, path, navigation_type
                ))
            })
    }

    fn launch_with_url(&self, url: String) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("launchWithUrl", &[&url])
            .map_err(|e| PlatformError::Platform(format!("Failed to launch with url: {}", e)))
    }
}
