use crate::android::get_env;
use jni::objects::{GlobalRef, JObject, JValue};
use log::info;
use miniapp::PageController;
use std::any::Any;
use std::error::Error;

const CLASS_MINIAPP: &str = "com/lingxia/miniapp/MiniApp";

pub struct WebView {
    #[cfg(debug_assertions)]
    app_id: String,
    #[cfg(debug_assertions)]
    path: String,
    java_webview: GlobalRef,
}

impl Drop for WebView {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        info!(
            "Dropping WebView for appId: {}, path: {}",
            self.app_id, self.path
        );

        let _ = self.destroy_webview();
    }
}

impl WebView {
    pub(crate) fn from_java(java_webview: JObject, _app_id: String, _path: String) -> Self {
        let env = get_env().unwrap();
        let java_webview = env.new_global_ref(java_webview).unwrap();

        #[cfg(debug_assertions)]
        return WebView {
            app_id: _app_id,
            path: _path,
            java_webview,
        };

        #[cfg(not(debug_assertions))]
        return WebView { java_webview };
    }

    pub(crate) fn get_java_webview(&self) -> &GlobalRef {
        &self.java_webview
    }

    fn destroy_webview(&self) {
        if let Ok(mut env) = get_env() {
            let _ = env.call_method(self.java_webview.as_obj(), "destroy", "()V", &[]);
        }
    }
}

impl PageController for WebView {
    fn load_url(&self, url: String) -> bool {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => return false,
        };

        match env.new_string(&url) {
            Ok(url_string) => env
                .call_method(
                    self.java_webview.as_obj(),
                    "loadUrl",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&url_string)],
                )
                .is_ok(),
            Err(_) => false,
        }
    }

    fn setup_ua(&self, ua: &str) {
        let Ok(mut env) = get_env() else { return };

        if let Ok(ua_string) = env.new_string(ua) {
            let _ = env.call_method(
                self.java_webview.as_obj(),
                "setUserAgent",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&ua_string)],
            );
        }
    }

    fn evaluate_javascript(&self, js: String) -> Option<String> {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => return None,
        };

        match env.new_string(&js) {
            Ok(script_string) => {
                match env.call_method(
                    self.java_webview.as_obj(),
                    "evaluateJavascript",
                    "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
                    &[
                        JValue::Object(&script_string),
                        JValue::Object(&JObject::null()),
                    ],
                ) {
                    Ok(_) => Some(String::new()),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    fn clear_browsing_data(&self) {
        let Ok(mut env) = get_env() else { return };

        let _ = env.call_method(self.java_webview.as_obj(), "clearBrowsingData", "()V", &[]);
    }

    fn set_devtools(&self, enabled: bool) -> bool {
        let Ok(mut env) = get_env() else { return false };

        match env.find_class("android/webkit/WebView") {
            Ok(webview_class) => env
                .call_static_method(
                    webview_class,
                    "setWebContentsDebuggingEnabled",
                    "(Z)V",
                    &[JValue::Bool(enabled as u8)],
                )
                .is_ok(),
            Err(_) => false,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct WebViewManager;

impl WebViewManager {
    /// Opens a mini app in a new activity
    pub fn open_mini_app(app_id: &str, path: &str) -> Result<(), Box<dyn Error>> {
        info!("Opening mini app with appId: {}, path: {}", app_id, path);
        let mut env = get_env()?;

        let miniapp_class = env.find_class(CLASS_MINIAPP)?;
        let app_id_jstring = env.new_string(app_id)?.into();
        let path_jstring = env.new_string(path)?.into();

        env.call_static_method(
            miniapp_class,
            "openMiniAppInNewActivity",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&app_id_jstring),
                JValue::Object(&path_jstring),
            ],
        )?;

        Ok(())
    }
}
