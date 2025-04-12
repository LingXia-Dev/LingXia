use super::webview::WebView;
use crate::android::{CLASS_MINIAPP, get_env};
use jni::objects::JValue;
use jni::sys::{JNIEnv, jobject};
use log::info;
use miniapp::MiniAppRuntime;
use miniapp::PageController;
use miniapp::log::LogLevel;
use ndk_sys;

// Platform implementation for Android
pub struct Platform {
    asset_manager: *mut ndk_sys::AAssetManager,
    data_dir: String,
    cache_dir: String,
}

// Implement Send and Sync for Platform since we handle the raw pointer safely
unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

impl Platform {
    pub fn from_java(
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
                Ok(Platform {
                    asset_manager: ptr,
                    data_dir,
                    cache_dir,
                })
            }
        }
    }
}

impl MiniAppRuntime for Platform {
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        unsafe {
            // Convert path to CString to ensure proper null-termination
            let c_path = std::ffi::CString::new(path)?;
            let asset = ndk_sys::AAssetManager_open(
                self.asset_manager,
                c_path.as_ptr(),
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );

            if asset.is_null() {
                return Err("Failed to open asset".into());
            }

            let length = ndk_sys::AAsset_getLength64(asset) as usize;
            let mut buffer = vec![0u8; length];
            let read = ndk_sys::AAsset_read(asset, buffer.as_mut_ptr() as *mut _, length) as i32;

            ndk_sys::AAsset_close(asset);

            if read == length as i32 {
                Ok(buffer)
            } else {
                Err("Failed to read asset completely".into())
            }
        }
    }

    fn open_miniapp(
        &self,
        app_id: &str,
        path: &str,
        tab_bar_config: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Opening mini app with appId: {}, path: {}", app_id, path);
        let mut env = get_env()?;

        let miniapp_class = env.find_class(CLASS_MINIAPP)?;
        let app_id_jstring = env.new_string(app_id)?;
        let path_jstring = env.new_string(path)?;

        // Convert tab_bar_config to JString if present
        let tab_bar_config_jstring = match tab_bar_config {
            Some(config) => env.new_string(config)?,
            None => unsafe { jni::objects::JString::from_raw(std::ptr::null_mut()) },
        };

        env.call_static_method(
            miniapp_class,
            "openMiniApp",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&app_id_jstring),
                JValue::Object(&path_jstring),
                JValue::Object(&tab_bar_config_jstring),
            ],
        )?;

        Ok(())
    }

    fn log(&self, level: LogLevel, message: &str) {
        // Use Android's logging system
        match level {
            LogLevel::Verbose => log::trace!("{}", message),
            LogLevel::Debug => log::debug!("{}", message),
            LogLevel::Info => log::info!("{}", message),
            LogLevel::Warn => log::warn!("{}", message),
            LogLevel::Error => log::error!("{}", message),
        }
    }

    fn get_data_dir(&self) -> Option<String> {
        Some(self.data_dir.clone())
    }

    fn get_cache_dir(&self) -> Option<String> {
        Some(self.cache_dir.clone())
    }

    fn post_message(
        &self,
        controller: &dyn PageController,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(webview) = controller.as_any().downcast_ref::<WebView>() {
            let mut env = get_env()?;
            env.call_method(
                webview.get_java_webview().as_obj(),
                "postMessageToWebView",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&env.new_string(message)?.into())],
            )?;
            Ok(())
        } else {
            Err("Controller is not a WebView".into())
        }
    }

    fn close_miniapp(&self, app_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("Closing mini app with appId: {}", app_id);
        let mut env = get_env()?;

        let miniapp_class = env.find_class(CLASS_MINIAPP)?;
        let app_id_jstring = env.new_string(app_id)?;

        env.call_static_method(
            miniapp_class,
            "closeMiniApp",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&app_id_jstring)],
        )?;

        Ok(())
    }
}
