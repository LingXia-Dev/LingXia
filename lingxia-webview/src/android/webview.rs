use crate::android::get_env;
use jni::objects::{GlobalRef, JObject, JValue};
use log::{error, info};
use miniapp::PageController;
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;
use std::sync::OnceLock;

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

    fn evaluate_javascript(&self, script: &str) -> Result<(), Box<dyn Error>> {
        let mut env = get_env()?;
        let script_string = env.new_string(script)?;
        env.call_method(
            self.java_webview.as_obj(),
            "evaluateJavascript",
            "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
            &[
                JValue::Object(&script_string),
                JValue::Object(&JObject::null()),
            ],
        )?;
        Ok(())
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

    pub fn load_url(&self, url: String) -> Result<(), Box<dyn Error>> {
        let mut env = get_env()?;
        let url_string = env.new_string(&url)?;
        env.call_method(
            self.java_webview.as_obj(),
            "loadUrl",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&url_string)],
        )?;
        Ok(())
    }

    pub fn clear_browsing_data(&self) -> Result<(), Box<dyn Error>> {
        let mut env = get_env()?;
        env.call_method(self.java_webview.as_obj(), "clearBrowsingData", "()V", &[])?;
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
        match self.load_url(url) {
            Ok(_) => true,
            Err(e) => {
                error!("Failed to load URL: {:?}", e);
                false
            }
        }
    }

    fn setup_ua(&self, ua: &str) {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(e) => {
                error!("Failed to get JNI env: {:?}", e);
                return;
            }
        };

        if let Ok(ua_string) = env.new_string(ua) {
            let _ = env
                .call_method(
                    self.java_webview.as_obj(),
                    "setUserAgent",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&ua_string)],
                )
                .map_err(|e| error!("Failed to set user agent: {:?}", e));
        }
    }

    fn evaluate_javascript(&self, js: String) -> Option<String> {
        match self.evaluate_javascript(&js) {
            Ok(_) => Some(String::new()),
            Err(e) => {
                error!("Failed to evaluate JavaScript: {:?}", e);
                None
            }
        }
    }

    fn clear_browsing_data(&self) {
        if let Err(e) = self.clear_browsing_data() {
            error!("Failed to clear browsing data: {:?}", e);
        }
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
