use crate::MiniAppPlatform;
use crate::android::{CLASS_MINIAPP, get_env};
use jni::objects::JValue;
use jni::sys::{JNIEnv, jobject};
use log::info;
use miniapp::MiniAppError;
use miniapp::log::LogLevel;
use ndk_sys;
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
}

// Implement the MiniAppPlatform trait for Android App
impl MiniAppPlatform for App {
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError> {
        unsafe {
            // Convert path to CString to ensure proper null-termination
            let c_path = std::ffi::CString::new(path)
                .map_err(|e| MiniAppError::WebView(format!("Invalid path: {}", e)))?;

            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );

            if asset.is_null() {
                return Err(MiniAppError::WebView(format!(
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
                Err(MiniAppError::WebView(format!(
                    "Failed to read asset completely: {}",
                    path
                )))
            }
        }
    }

    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn log(&self, level: LogLevel, app_id: &str, message: &str) {
        // Use Android's logging system
        let log_msg = format!("[{}] {}", app_id, message);
        match level {
            LogLevel::Verbose => log::trace!("{}", log_msg),
            LogLevel::Debug => log::debug!("{}", log_msg),
            LogLevel::Info => log::info!("{}", log_msg),
            LogLevel::Warn => log::warn!("{}", log_msg),
            LogLevel::Error => log::error!("{}", log_msg),
        }
    }

    fn open_miniapp(&self, app_id: &str, path: &str) -> Result<(), MiniAppError> {
        info!("Opening mini app with appId: {}, path: {}", app_id, path);

        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;
            let miniapp_class = env.find_class(CLASS_MINIAPP)?;
            let app_id_jstring = env.new_string(app_id)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                miniapp_class,
                "openMiniApp",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&app_id_jstring),
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

    fn switch_page(&self, app_id: &str, path: &str) -> Result<(), MiniAppError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;
            let miniapp_class = env.find_class(CLASS_MINIAPP)?;
            let app_id_jstring = env.new_string(app_id)?;
            let path_jstring = env.new_string(path)?;

            env.call_static_method(
                miniapp_class,
                "switchPage",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&app_id_jstring),
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
