use crate::MiniAppPlatform;
use crate::android::{CLASS_MINIAPP, get_env};
use jni::objects::JValue;
use jni::sys::{JNIEnv, jobject};
use log::info;
use miniapp::MiniAppError;
use miniapp::log::LogLevel;
use ndk_sys;
use std::ffi::{CStr, CString};
use std::io::{Read, Result as IoResult};
use std::path::PathBuf;

// App for Android
#[derive(Clone)]
pub struct App {
    asset_manager: *mut ndk_sys::AAssetManager,
    data_dir: String,
    cache_dir: String,
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

/// Reader for a single asset file
pub struct AssetReader {
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

/// Iterator over files in an asset directory
pub struct AssetDirIterator<'a> {
    asset_manager: *mut ndk_sys::AAssetManager,
    dir: *mut ndk_sys::AAssetDir,
    dir_path: String,
    finished: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Iterator for AssetDirIterator<'a> {
    type Item = Result<crate::AssetFileEntry<'a>, MiniAppError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        unsafe {
            let filename_ptr = ndk_sys::AAssetDir_getNextFileName(self.dir);
            if filename_ptr.is_null() {
                self.finished = true;
                return None;
            }
            let filename = CStr::from_ptr(filename_ptr).to_string_lossy().into_owned();
            let full_path = if self.dir_path.is_empty() {
                filename.clone()
            } else {
                format!("{}/{}", self.dir_path, filename)
            };
            // Open asset for reading
            let c_path = match CString::new(full_path.clone()) {
                Ok(c) => c,
                Err(e) => return Some(Err(MiniAppError::IoError(format!("Invalid path: {}", e)))),
            };
            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_STREAMING as i32,
            );
            if asset.is_null() {
                // Directory might contain subdirectories, skip items that cannot be opened
                return self.next();
            }
            let reader = AssetReader { asset };
            Some(Ok(crate::AssetFileEntry {
                path: full_path,
                reader: Box::new(reader),
            }))
        }
    }
}

impl<'a> Drop for AssetDirIterator<'a> {
    fn drop(&mut self) {
        unsafe { ndk_sys::AAssetDir_close(self.dir) };
    }
}

impl App {
    pub(crate) fn from_java(
        env: *mut JNIEnv,
        asset_manager: jobject,
        data_dir: String,
        cache_dir: String,
    ) -> Result<Self, &'static str> {
        unsafe {
            let ptr = ndk_sys::AAssetManager_fromJava(env, asset_manager);
            if ptr.is_null() {
                Err("Failed to get AAssetManager from Java object")
            } else {
                Ok(App {
                    asset_manager: ptr,
                    data_dir,
                    cache_dir,
                })
            }
        }
    }

    pub fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<crate::AssetFileEntry<'a>, MiniAppError>> + 'a> {
        let c_dir = match CString::new(asset_dir) {
            Ok(c) => c,
            Err(e) => {
                return Box::new(std::iter::once(Err(MiniAppError::IoError(format!(
                    "Invalid directory path: {}",
                    e
                )))));
            }
        };
        unsafe {
            let dir = ndk_sys::AAssetManager_openDir(self.asset_manager, c_dir.as_ptr());
            if dir.is_null() {
                return Box::new(std::iter::once(Err(MiniAppError::IoError(format!(
                    "Failed to open asset directory: {}",
                    asset_dir
                )))));
            }
            Box::new(AssetDirIterator {
                asset_manager: self.asset_manager,
                dir,
                dir_path: asset_dir.to_string(),
                finished: false,
                _marker: std::marker::PhantomData,
            })
        }
    }
}

impl MiniAppPlatform for App {
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError> {
        unsafe {
            // Convert path to CString to ensure proper null-termination
            let c_path = std::ffi::CString::new(path)
                .map_err(|e| MiniAppError::IoError(format!("Invalid path: {}", e)))?;

            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );

            if asset.is_null() {
                return Err(MiniAppError::IoError(format!(
                    "Failed to open asset: {}",
                    path
                )));
            }

            let length = ndk_sys::AAsset_getLength64(asset) as usize;
            let mut buffer = vec![0u8; length];
            let read = ndk_sys::AAsset_read(asset, buffer.as_mut_ptr() as *mut _, length) as i32;

            ndk_sys::AAsset_close(asset);

            if read == length as i32 {
                Ok(buffer)
            } else {
                Err(MiniAppError::IoError(format!(
                    "Failed to read asset completely: {}",
                    path
                )))
            }
        }
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<crate::AssetFileEntry<'a>, MiniAppError>> + 'a> {
        self.asset_dir_iter(asset_dir)
    }

    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn log(&self, appid: &str, level: LogLevel, message: &str) {
        // Use Android's logging system
        let log_msg = format!("[{}] {}", appid, message);
        match level {
            LogLevel::Verbose => log::trace!("{}", log_msg),
            LogLevel::Debug => log::debug!("{}", log_msg),
            LogLevel::Info => log::info!("{}", log_msg),
            LogLevel::Warn => log::warn!("{}", log_msg),
            LogLevel::Error => log::error!("{}", log_msg),
        }
    }

    fn open_miniapp(&self, appid: &str, path: &str) -> Result<(), MiniAppError> {
        info!("Opening mini app with appId: {}, path: {}", appid, path);

        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env().unwrap();

            let miniapp_class = env.find_class(CLASS_MINIAPP)?;
            let appid_jstring = env.new_string(appid)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                miniapp_class,
                "openMiniApp",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                ],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(MiniAppError::WebView(format!(
                "Failed to open miniapp: {}",
                e
            ))),
        }
    }

    fn switch_page(&self, appid: &str, path: &str) -> Result<(), MiniAppError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env().unwrap();

            let miniapp_class = env.find_class(CLASS_MINIAPP)?;
            let appid_jstring = env.new_string(appid)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                miniapp_class,
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
            Err(e) => Err(MiniAppError::WebView(format!(
                "Failed to switch page: {}",
                e
            ))),
        }
    }
}
