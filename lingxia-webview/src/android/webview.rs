use crate::android::get_env;
use jni::objects::{GlobalRef, JObject, JValue};
use log::{error, info};
use miniapp::PageController;
use serde_json::Value;
use std::any::Any;
use std::error::Error;

const CLASS_MINIAPP: &str = "com/lingxia/miniapp/MiniApp";

pub struct WebView {
    app_id: String,
    path: String,
    java_webview: GlobalRef,
}

impl Drop for WebView {
    fn drop(&mut self) {
        info!(
            "Dropping WebView for appId: {}, path: {}",
            self.app_id, self.path
        );
        let _ = self.destroy_webview();
    }
}

impl WebView {
    pub(crate) fn from_java(java_webview: JObject) -> Self {
        let env = get_env().unwrap();
        let java_webview = env.new_global_ref(java_webview).unwrap();
        WebView {
            app_id: String::new(),
            path: String::new(),
            java_webview,
        }
    }

    pub(crate) fn get_java_webview(&self) -> &GlobalRef {
        &self.java_webview
    }

    pub fn set_devtools(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        let mut env = get_env()?;
        let webview_class = env.find_class("android/webkit/WebView")?;
        env.call_static_method(
            webview_class,
            "setWebContentsDebuggingEnabled",
            "(Z)V",
            &[JValue::Bool(enabled as u8)],
        )?;
        Ok(())
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct WebViewManager;

impl WebViewManager {
    pub fn handle_post_message(
        appid: String,
        _path: String,
        message_str: String,
    ) -> Result<(), Box<dyn Error>> {
        info!(
            "Handling message for WebView with appId {}: {}",
            appid, message_str
        );

        let message: Value = serde_json::from_str(&message_str)?;
        let message_type = message.get("type").and_then(Value::as_str);

        match message_type {
            Some("OPEN_MINIAPP") => {
                info!("Handling OPEN_MINIAPP message");
                if let Some(data) = message.get("data") {
                    if let Some(app_id) = data.get("appId").and_then(Value::as_str) {
                        let path = data.get("path").and_then(Value::as_str).unwrap_or("");
                        WebViewManager::open_mini_app(app_id, path)?;
                    }
                }
                Ok(())
            }
            _ => {
                error!("Unknown message type: {:?}", message_type);
                Ok(())
            }
        }
    }

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
