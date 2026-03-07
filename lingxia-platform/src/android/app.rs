use crate::AssetFileEntry;
use crate::error::PlatformError;
use crate::traits::app_runtime::AppRuntime;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::sys::jobject;
use jni::{Env, jni_sig, jni_str};
use lingxia_webview::with_env;
use std::ffi::CString;
use std::io::{Read, Result as IoResult};
use std::path::{Path, PathBuf};

use super::{CachedClass, get_cached_class};

// Platform for Android
pub struct Platform {
    asset_manager: *mut ndk_sys::AAssetManager,
    java_asset_manager: Global<JObject<'static>>,
    data_dir: String,
    cache_dir: String,
    locale: String,
}

impl Clone for Platform {
    fn clone(&self) -> Self {
        let java_asset_manager = with_env(
            |env| -> Result<Global<JObject<'static>>, Box<dyn std::error::Error>> {
                Ok(env.new_global_ref(self.java_asset_manager.as_ref())?)
            },
        )
        .expect("Failed to clone Platform java_asset_manager");

        Platform {
            asset_manager: self.asset_manager,
            java_asset_manager,
            data_dir: self.data_dir.clone(),
            cache_dir: self.cache_dir.clone(),
            locale: self.locale.clone(),
        }
    }
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

impl crate::traits::update::UpdateService for Platform {
    fn show_download_progress(&self) -> Result<(), PlatformError> {
        super::update::show_download_progress().map_err(|e| {
            PlatformError::Platform(format!("Failed to show download progress: {}", e))
        })
    }

    fn update_download_progress(&self, progress: i32) -> Result<(), PlatformError> {
        super::update::update_download_progress(progress).map_err(|e| {
            PlatformError::Platform(format!("Failed to update download progress: {}", e))
        })
    }

    fn dismiss_download_progress(&self) -> Result<(), PlatformError> {
        super::update::dismiss_download_progress().map_err(|e| {
            PlatformError::Platform(format!("Failed to dismiss download progress: {}", e))
        })
    }

    fn show_update_prompt(
        &self,
        callback_id: u64,
        update_info_json: Option<&str>,
    ) -> Result<(), PlatformError> {
        super::update::show_update_prompt(callback_id, update_info_json)
            .map_err(|e| PlatformError::Platform(format!("Failed to show update prompt: {}", e)))
    }

    fn install_update(&self, apk_path: &Path) -> Result<(), PlatformError> {
        let update_manager_class: &JClass =
            super::get_cached_class(super::CachedClass::UpdateManager)
                .map_err(|e| PlatformError::Platform(e.to_string()))?;

        with_env(|env| -> Result<(), PlatformError> {
            let path_str = apk_path
                .to_str()
                .ok_or_else(|| PlatformError::Platform("Invalid APK path".to_string()))?;
            let path_jstring = env.new_string(path_str)?;

            let result = env.call_static_method(
                update_manager_class,
                jni_str!("installUpdate"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&path_jstring)],
            )?;
            if !result.z()? {
                return Err(PlatformError::Platform(
                    "installUpdate returned false".to_string(),
                ));
            }
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to install update: {}", e)))
    }
}

/// Reader for a single asset file
struct AssetReader {
    asset: *mut ndk_sys::AAsset,
}

impl Read for AssetReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let read =
            unsafe { ndk_sys::AAsset_read(self.asset, buf.as_mut_ptr() as *mut _, buf.len()) };
        if read < 0 {
            Err(std::io::Error::other("AAsset_read failed"))
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
    app: &'a Platform,
    // Stack of directory paths (relative to asset root) to visit.
    dir_stack: Vec<String>,
    // Queue of discovered file paths (full paths) ready to be yielded.
    file_queue: Vec<String>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> RecursiveAssetIterator<'a> {
    fn new(app: &'a Platform, initial_path: &str) -> Self {
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
    ) -> Result<T, PlatformError> {
        result.map_err(|e| {
            PlatformError::Platform(format!("JNI operation failed for path '{}': {:?}", path, e))
        })
    }

    fn list_via_jni(&self, path_to_list: &str) -> Result<Option<Vec<String>>, PlatformError> {
        with_env(|env| -> Result<Option<Vec<String>>, PlatformError> {
            let path_jstring = Self::handle_jni_error(env.new_string(path_to_list), path_to_list)?;

            let java_am_obj = self.app.java_asset_manager.as_ref();
            let jvalue = Self::handle_jni_error(
                env.call_method(
                    java_am_obj,
                    jni_str!("list"),
                    jni_sig!("(Ljava/lang/String;)[Ljava/lang/String;"),
                    &[JValue::from(&path_jstring)],
                ),
                path_to_list,
            )?;

            if env.exception_check() {
                env.exception_clear();
                Ok(None)
            } else {
                let jobject_array = Self::handle_jni_error(jvalue.l(), path_to_list)?;

                if jobject_array.is_null() {
                    Ok(None)
                } else {
                    let jobject_array = unsafe {
                        jni::objects::JObjectArray::<JObject>::from_raw(
                            env,
                            jobject_array.into_raw() as _,
                        )
                    };
                    let array_len = jobject_array.len(env)?;

                    if array_len == 0 {
                        return Ok(Some(Vec::new()));
                    }

                    let mut entries = Vec::with_capacity(array_len);
                    for i in 0..array_len {
                        let entry_jobject: JObject = jobject_array.get_element(env, i)?;
                        let entry_jstring_wrapper = unsafe {
                            jni::objects::JString::from_raw(env, entry_jobject.into_raw() as _)
                        };
                        let entry_str = Self::handle_jni_error(
                            entry_jstring_wrapper.try_to_string(env),
                            path_to_list,
                        )?;
                        entries.push(entry_str);
                    }
                    Ok(Some(entries))
                }
            }
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to get JNIEnv: {}", e)))
    }
}

impl<'a> Iterator for RecursiveAssetIterator<'a> {
    type Item = Result<AssetFileEntry<'a>, PlatformError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(file_path_to_yield) = self.file_queue.pop() {
                let c_path = match CString::new(file_path_to_yield.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        return Some(Err(PlatformError::Platform(format!(
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
                        return Some(Err(PlatformError::Platform(format!(
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

impl Platform {
    /// # Safety
    /// Caller must ensure `java_asset_manager_obj` is a valid `android.content.res.AssetManager`.
    pub unsafe fn from_java(
        jni_env: &mut Env,
        java_asset_manager_obj: jobject,
        data_dir: String,
        cache_dir: String,
        locale: String,
    ) -> Result<Self, String> {
        // Get the native asset manager pointer from the Java AssetManager
        let asset_manager_ptr = unsafe {
            ndk_sys::AAssetManager_fromJava(
                jni_env.get_raw() as *mut _,
                java_asset_manager_obj as *mut _,
            )
        };

        if asset_manager_ptr.is_null() {
            return Err("Failed to get native AssetManager".to_string());
        }

        // Create a global reference to the Java AssetManager for later use
        let java_asset_manager = jni_env
            .new_global_ref(unsafe { JObject::from_raw(jni_env, java_asset_manager_obj) })
            .map_err(|e| format!("Failed to create global reference: {:?}", e))?;

        Ok(Platform {
            asset_manager: asset_manager_ptr,
            java_asset_manager,
            data_dir,
            cache_dir,
            locale,
        })
    }

    fn resolve_app_identifier(jni_env: &mut Env) -> Result<String, PlatformError> {
        // Use cached LxApp class to obtain the application context and package name.
        let lxapp_class: &JClass = get_cached_class(CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        let context = jni_env
            .call_static_method(
                lxapp_class,
                jni_str!("getApplicationContext"),
                jni_sig!("()Landroid/content/Context;"),
                &[],
            )
            .and_then(|val| val.l())
            .map_err(|e| {
                PlatformError::Platform(format!("Failed to get application context: {:?}", e))
            })?;
        if context.is_null() {
            return Err(PlatformError::Platform(
                "Application context is null".to_string(),
            ));
        }

        let package_obj = jni_env
            .call_method(
                context,
                jni_str!("getPackageName"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )
            .and_then(|val| val.l())
            .map_err(|e| PlatformError::Platform(format!("Failed to get package name: {:?}", e)))?;
        if package_obj.is_null() {
            return Err(PlatformError::Platform("Package name is null".to_string()));
        }

        let package_jstring = unsafe { JString::from_raw(jni_env, package_obj.into_raw() as _) };
        let package_name = package_jstring.try_to_string(jni_env).map_err(|e| {
            PlatformError::Platform(format!("Failed to read package name: {:?}", e))
        })?;

        Ok(package_name)
    }
}

impl AppRuntime for Platform {
    /// Read asset file from platform-specific location as a streaming reader
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        unsafe {
            // Convert path to CString to ensure proper null-termination
            let c_path = std::ffi::CString::new(path)
                .map_err(|e| PlatformError::Platform(format!("Invalid path: {}", e)))?;

            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_STREAMING as i32,
            );

            if asset.is_null() {
                return Err(PlatformError::AssetNotFound(format!(
                    "Failed to open asset: {}",
                    path
                )));
            }

            // Return a reader instead of reading the entire asset into memory
            Ok(Box::new(AssetReader { asset }))
        }
    }

    /// Iterate over files in an asset directory
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        Box::new(RecursiveAssetIterator::new(self, asset_dir))
    }

    /// Get data directory path
    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    /// Get cache directory path
    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn get_app_identifier(&self) -> Result<String, PlatformError> {
        with_env(Platform::resolve_app_identifier)
            .map_err(|e| PlatformError::Platform(format!("Failed to get app identifier: {}", e)))
    }

    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        crate::traits::media_runtime::MediaRuntime::copy_album_media_to_file(
            self, uri, dest_path, kind,
        )
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn show_lxapp(
        &self,
        appid: String,
        path: String,
        session_id: u64,
    ) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            let path_jstring = env.new_string(&path)?;

            env.call_static_method(
                lxapp_class,
                jni_str!("openLxApp"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;J)V"),
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                    JValue::Long(session_id as i64),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to show lxapp: {}", e)))
    }

    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            env.call_static_method(
                lxapp_class,
                jni_str!("closeLxApp"),
                jni_sig!("(Ljava/lang/String;J)V"),
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Long(session_id as i64),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to hide lxapp: {}", e)))
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: crate::traits::app_runtime::AnimationType,
    ) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            let path_jstring = env.new_string(&path)?;
            let anim_type_int = animation_type as i32;

            let result = env.call_static_method(
                lxapp_class,
                jni_str!("navigate"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)Z"),
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                    JValue::Int(anim_type_int),
                ],
            )?;
            if !result.z()? {
                return Err(PlatformError::Platform(format!(
                    "Navigation returned false: appid={}, path={}, animation_type={:?}",
                    appid, path, animation_type
                )));
            }
            Ok(())
        })
        .map_err(|_| {
            PlatformError::Platform(format!(
                "Failed to navigate: appid={}, path={}, animation_type={:?}",
                appid, path, animation_type
            ))
        })
    }

    fn open_url(
        &self,
        req: crate::traits::app_runtime::OpenUrlRequest,
    ) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        let target_str = if req.target == crate::traits::app_runtime::OpenUrlTarget::SelfTarget {
            "self"
        } else {
            "external"
        };

        with_env(|env| -> Result<(), PlatformError> {
            let url_jstring = env.new_string(req.url)?;
            let target_jstring = env.new_string(target_str)?;
            env.call_static_method(
                lxapp_class,
                jni_str!("launchWithUrl"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                &[
                    JValue::Object(&url_jstring),
                    JValue::Object(&target_jstring),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to open_url: {}", e)))
    }

    fn get_capsule_rect(&self, callback_id: u64) -> Result<(), PlatformError> {
        with_env(|env| -> Result<(), PlatformError> {
            let capsule_class: &JClass = super::get_cached_class(super::CachedClass::LxAppCapsule)
                .map_err(|e| {
                    PlatformError::Platform(format!("Failed to get LxAppCapsule class: {}", e))
                })?;

            env.call_static_method(
                capsule_class,
                jni_str!("getCapsuleRect"),
                jni_sig!("(J)V"),
                &[JValue::Long(callback_id as i64)],
            )
            .map_err(|e| {
                log::error!("[Android] getCapsuleRect JNI call failed: {}", e);
                PlatformError::Platform(format!("Failed to get capsule rect: {}", e))
            })?;

            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to get JNI env: {}", e)))
    }
}
