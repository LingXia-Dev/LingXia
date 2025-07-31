use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::jobject;
use lingxia_webview::get_env;
use lxapp::{AssetFileEntry, DeviceInfo, LxAppError};
use ndk_sys;
use std::ffi::CString;
use std::io::{Read, Result as IoResult};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global reference to LxApp class for worker threads
static LXAPP_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Initialize LxApp class global reference (called from JNI_OnLoad)
pub(super) fn init_lxapp_class(global_ref: GlobalRef) {
    let _ = LXAPP_CLASS.set(global_ref);
}

// App for Android
#[derive(Clone)]
pub struct App {
    asset_manager: *mut ndk_sys::AAssetManager,
    java_asset_manager: GlobalRef,
    data_dir: String,
    cache_dir: String,
    device_info: DeviceInfo,
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

/// Reader for a single asset file
struct AssetReader {
    asset: *mut ndk_sys::AAsset,
}

impl Read for AssetReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let read =
            unsafe { ndk_sys::AAsset_read(self.asset, buf.as_mut_ptr() as *mut _, buf.len()) };
        if read < 0 {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "AAsset_read failed",
            ))
        } else {
            Ok(read as usize)
        }
    }
}

impl Drop for AssetReader {
    fn drop(&mut self) {
        unsafe { ndk_sys::AAsset_close(self.asset) };
    }
}

// Using Java AssetManager.list() via JNI for directory listing because the NDK's
// AAssetManager_openDir and AAssetDir_getNextFileName have shown inconsistent behavior
// in listing subdirectories or distinguishing files from directories reliably across
// all scenarios in this project. The Java API is generally more robust for listing.
// NDK AAssetManager_open() is still used for reading files once paths are known.
struct RecursiveAssetIterator<'a> {
    app: &'a App,
    // Stack of directory paths (relative to asset root) to visit.
    dir_stack: Vec<String>,
    // Queue of discovered file paths (full paths) ready to be yielded.
    file_queue: Vec<String>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> RecursiveAssetIterator<'a> {
    fn new(app: &'a App, initial_path: &str) -> Self {
        RecursiveAssetIterator {
            app,
            dir_stack: vec![initial_path.to_string()],
            file_queue: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    // Helper function to simplify error handling for JNI operations
    fn handle_jni_error<T>(
        result: Result<T, jni::errors::Error>,
        path: &str,
    ) -> Result<T, LxAppError> {
        result.map_err(|e| {
            LxAppError::InvalidParameter(format!(
                "JNI operation failed for path '{}': {:?}",
                path, e
            ))
        })
    }

    fn list_via_jni(&self, path_to_list: &str) -> Result<Option<Vec<String>>, LxAppError> {
        let mut jni_env = get_env()
            .map_err(|e| LxAppError::InvalidParameter(format!("Failed to get JNIEnv: {}", e)))?;
        let path_jstring = Self::handle_jni_error(jni_env.new_string(path_to_list), path_to_list)?;

        let java_am_obj = self.app.java_asset_manager.as_obj();
        let jvalue = Self::handle_jni_error(
            jni_env.call_method(
                java_am_obj,
                "list",
                "(Ljava/lang/String;)[Ljava/lang/String;",
                &[JValue::from(&path_jstring)],
            ),
            path_to_list,
        )?;

        if Self::handle_jni_error(jni_env.exception_check(), path_to_list)? {
            Self::handle_jni_error(jni_env.exception_clear(), path_to_list)?;
            Ok(None)
        } else {
            let jobject_array = Self::handle_jni_error(jvalue.l(), path_to_list)?;

            if jobject_array.is_null() {
                Ok(None)
            } else {
                let jobject_array = jni::objects::JObjectArray::from(jobject_array);
                let array_len =
                    Self::handle_jni_error(jni_env.get_array_length(&jobject_array), path_to_list)?;

                if array_len == 0 {
                    return Ok(Some(Vec::new()));
                }

                let mut entries = Vec::with_capacity(array_len as usize);
                for i in 0..array_len {
                    let entry_jobject = Self::handle_jni_error(
                        jni_env.get_object_array_element(&jobject_array, i),
                        path_to_list,
                    )?;
                    let entry_jstring_wrapper: jni::objects::JString = entry_jobject.into();
                    let entry_java_str = Self::handle_jni_error(
                        jni_env.get_string(&entry_jstring_wrapper),
                        path_to_list,
                    )?;
                    entries.push(entry_java_str.into());
                }
                Ok(Some(entries))
            }
        }
    }
}

impl<'a> Iterator for RecursiveAssetIterator<'a> {
    type Item = Result<AssetFileEntry<'a>, LxAppError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(file_path_to_yield) = self.file_queue.pop() {
                let c_path = match CString::new(file_path_to_yield.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        return Some(Err(LxAppError::InvalidParameter(format!(
                            "Invalid CString for asset path '{}': {}",
                            file_path_to_yield, e
                        ))));
                    }
                };
                unsafe {
                    let asset_ptr = ndk_sys::AAssetManager_open(
                        self.app.asset_manager,
                        c_path.as_ptr(),
                        ndk_sys::AASSET_MODE_STREAMING as i32,
                    );
                    if asset_ptr.is_null() {
                        return Some(Err(LxAppError::InvalidParameter(format!(
                            "Asset '{}' was in file_queue, but NDK AAssetManager_open failed.",
                            file_path_to_yield
                        ))));
                    }
                    let reader = AssetReader { asset: asset_ptr };
                    return Some(Ok(AssetFileEntry {
                        path: file_path_to_yield,
                        reader: Box::new(reader),
                    }));
                }
            }

            if let Some(dir_to_scan) = self.dir_stack.pop() {
                match self.list_via_jni(&dir_to_scan) {
                    Ok(Some(child_entry_names)) => {
                        let mut discovered_files_in_this_scan = Vec::new();
                        for child_name in child_entry_names {
                            let full_child_path = if dir_to_scan.is_empty() {
                                child_name
                            } else {
                                format!("{}/{}", dir_to_scan, child_name)
                            };

                            let c_full_child_path = match CString::new(full_child_path.clone()) {
                                Ok(c) => c,
                                Err(_e) => continue,
                            };
                            let asset_ptr_check = unsafe {
                                ndk_sys::AAssetManager_open(
                                    self.app.asset_manager,
                                    c_full_child_path.as_ptr(),
                                    ndk_sys::AASSET_MODE_STREAMING as i32,
                                )
                            };

                            if !asset_ptr_check.is_null() {
                                unsafe { ndk_sys::AAsset_close(asset_ptr_check) };
                                discovered_files_in_this_scan.push(full_child_path);
                            } else {
                                self.dir_stack.push(full_child_path);
                            }
                        }
                        discovered_files_in_this_scan.reverse();
                        self.file_queue.append(&mut discovered_files_in_this_scan);
                        continue;
                    }
                    Ok(None) => continue,
                    Err(e) => return Some(Err(e)),
                }
            } else {
                return None;
            }
        }
    }
}

impl App {
    pub(crate) fn from_java(
        jni_env: &mut jni::JNIEnv,
        java_asset_manager_obj: jobject,
        data_dir: String,
        cache_dir: String,
    ) -> Result<Self, String> {
        // Get the native asset manager pointer from the Java AssetManager
        let asset_manager_ptr =
            unsafe { ndk_sys::AAssetManager_fromJava(jni_env.get_raw(), java_asset_manager_obj) };

        if asset_manager_ptr.is_null() {
            return Err("Failed to get native AssetManager".to_string());
        }

        // Create a global reference to the Java AssetManager for later use
        let java_asset_manager = jni_env
            .new_global_ref(unsafe { JObject::from_raw(java_asset_manager_obj) })
            .map_err(|e| format!("Failed to create global reference: {:?}", e))?;

        // Get device information using getprop commands
        let device_brand = std::process::Command::new("getprop")
            .arg("ro.product.brand")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        let device_model = std::process::Command::new("getprop")
            .arg("ro.product.model")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        let system = std::process::Command::new("getprop")
            .arg("ro.build.version.release")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        // Combine OS name with version
        let system = format!("{} {}", "Android", system);

        let device_info = DeviceInfo {
            brand: device_brand,
            model: device_model,
            system,
        };

        Ok(App {
            asset_manager: asset_manager_ptr,
            java_asset_manager,
            data_dir,
            cache_dir,
            device_info,
        })
    }

    /// Read asset file from platform-specific location as a streaming reader
    pub(crate) fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, LxAppError> {
        unsafe {
            // Convert path to CString to ensure proper null-termination
            let c_path = std::ffi::CString::new(path)
                .map_err(|e| LxAppError::InvalidParameter(format!("Invalid path: {}", e)))?;

            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_STREAMING as i32,
            );

            if asset.is_null() {
                return Err(LxAppError::ResourceNotFound(format!(
                    "Failed to open asset: {}",
                    path
                )));
            }

            // Return a reader instead of reading the entire asset into memory
            Ok(Box::new(AssetReader { asset }))
        }
    }

    /// Iterate over files in an asset directory
    pub(crate) fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, LxAppError>> + 'a> {
        Box::new(RecursiveAssetIterator::new(self, asset_dir))
    }

    /// Get data directory path
    pub(crate) fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    /// Get cache directory path
    pub(crate) fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    /// Get device information
    pub(crate) fn device_info(&self) -> DeviceInfo {
        self.device_info.clone()
    }

    pub(crate) fn open_lxapp(&self, appid: &str, path: &str) -> Result<(), LxAppError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;

            let lxapp_class: &JClass = LXAPP_CLASS
                .get()
                .ok_or("Global LxApp class reference not available")?
                .as_obj()
                .into();
            let appid_jstring = env.new_string(appid)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                lxapp_class,
                "openLxApp",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                ],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(LxAppError::WebView(format!(
                "Failed to open lxapp: {}",
                e
            ))),
        }
    }

    pub(crate) fn close_lxapp(&self, appid: &str) -> Result<(), LxAppError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;

            let lxapp_class: &JClass = LXAPP_CLASS
                .get()
                .ok_or("Global LxApp class reference not available")?
                .as_obj()
                .into();

            let appid_jstring = env.new_string(appid)?;

            env.call_static_method(
                lxapp_class,
                "closeLxApp",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&appid_jstring)],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(LxAppError::WebView(format!(
                "Failed to close lxapp: {}",
                e
            ))),
        }
    }

    pub(crate) fn switch_page(&self, appid: &str, path: &str) -> Result<(), LxAppError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;

            let lxapp_class: &JClass = LXAPP_CLASS
                .get()
                .ok_or("Global LxApp class reference not available")?
                .as_obj()
                .into();

            let appid_jstring = env.new_string(appid)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                lxapp_class,
                "switchPage",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                ],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(LxAppError::WebView(format!("Failed to switch page: {}", e))),
        }
    }
}
