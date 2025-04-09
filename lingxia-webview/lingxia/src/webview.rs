use crate::JAVA_VM;
use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;
use log::{error, info};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;
use std::sync::OnceLock;

const CLASS_MINIAPP: &str = "com/lingxia/miniapp/MiniApp";

static WEBVIEWS: OnceLock<Mutex<HashMap<String, Vec<WebView>>>> = OnceLock::new();

const DOCUMENT_START_SCRIPT: &str = r#"
    (function() {
        if (!window.lingxia) {
            window.lingxia = {
                postMessage: function(message) {
                    MiniApp.postMessage(message);
                }
            };
            console.log('MiniApp bridge initialized');
            window.lingxia.postMessage('{"type":"BRIDGE_READY"}');
            return true;
        }
        console.log('MiniApp bridge already exists');
        return false;
    })();
"#;

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
    fn new(app_id: String, path: String, java_webview: JObject) -> Result<WebView, Box<dyn Error>> {
        let env = get_env()?;
        let webview = WebView {
            app_id: app_id.clone(),
            path: path.clone(),
            java_webview: env.new_global_ref(java_webview)?,
        };
        Ok(webview)
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

    fn setup(&self) -> Result<(), Box<dyn Error>> {
        let mut env = get_env()?;
        // 设置基本参数
        let _ua_string = env.new_string("MiniApp/1.0")?;
        /*        env.call_method(
            self.java_webview.as_obj(),
            "setUserAgent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&ua_string)]
        )?;
        */

        // 在build模式下启用开发者工具
        #[cfg(debug_assertions)]
        self.set_devtools(true)?;

        //DEBUG ONLY: 根据 app_id 和 path 确定要加载的 URL
        let url = if self.app_id == "demo" {
            let path_str = if self.path.is_empty() {
                "index.html"
            } else {
                &self.path
            };
            format!("lingxia://demo/{}", path_str)
        } else if self.app_id == "baidu" {
            "https://www.bing.com".to_string()
        } else {
            "about:blank".to_string()
        };

        info!("Loading URL: {}", url);
        self.load_url(url)?;

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

    fn inject_bridge_script(&self) -> Result<(), Box<dyn Error>> {
        self.evaluate_javascript(DOCUMENT_START_SCRIPT)
    }

    /// Destroy this WebView instance and remove it from the global WebViews map
    fn destroy_webview(&self) -> Result<(), Box<dyn Error>> {
        // First destroy the Java WebView
        if let Ok(mut env) = get_env() {
            let _ = env.call_method(self.java_webview.as_obj(), "destroy", "()V", &[]);
        }

        // Then remove from global map
        if let Some(webviews) = WEBVIEWS.get() {
            let mut webviews = webviews.lock().unwrap();
            if let Some(app_webviews) = webviews.get_mut(&self.app_id) {
                if let Some(index) = app_webviews.iter().position(|w| w.path == self.path) {
                    app_webviews.remove(index);

                    // If this was the last WebView for this app_id, remove the app entry
                    if app_webviews.is_empty() {
                        webviews.remove(&self.app_id);
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct WebViewManager;

impl WebViewManager {
    fn find_webview<'a>(
        webviews: &'a HashMap<String, Vec<WebView>>,
        app_id: &str,
        path: &str,
    ) -> Option<&'a WebView> {
        webviews
            .get(app_id)
            .and_then(|views| views.iter().find(|view| view.path == path))
    }

    fn find_webview_mut<'a>(
        webviews: &'a mut HashMap<String, Vec<WebView>>,
        app_id: &str,
        path: &str,
    ) -> Option<&'a mut WebView> {
        webviews
            .get_mut(app_id)
            .and_then(|views| views.iter_mut().find(|view| view.path == path))
    }

    pub fn on_webview_created(
        app_id: String,
        path: String,
        java_webview: JObject,
    ) -> Result<(), Box<dyn Error>> {
        info!("Creating new WebView for appId: {}, path: {}", app_id, path);

        let webviews = WEBVIEWS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut webviews = webviews.lock().unwrap();

        let webview = WebView::new(app_id.clone(), path.clone(), java_webview)?;
        webview.setup()?;

        if let Some(app_webviews) = webviews.get_mut(&app_id) {
            info!("Adding WebView to existing app entry for appId: {}", app_id);
            app_webviews.push(webview);
        } else {
            info!("Creating new app entry for appId: {}", app_id);
            webviews.insert(app_id, vec![webview]);
        }

        Ok(())
    }

    pub fn destroy_all_webviews() -> Result<(), Box<dyn Error>> {
        info!("Destroying all WebViews");
        if let Some(webviews) = WEBVIEWS.get() {
            let mut webviews = webviews.lock().unwrap();
            webviews.clear();
        }
        Ok(())
    }

    pub fn on_page_started(app_id: String, path: String) -> Result<(), Box<dyn Error>> {
        info!("Page started loading for appId: {}, path: {}", app_id, path);
        if let Some(webviews) = WEBVIEWS.get() {
            let webviews = webviews.lock().unwrap();
            if let Some(webview) = WebViewManager::find_webview(&webviews, &app_id, &path) {
                webview.inject_bridge_script()?;
            }
        }
        Ok(())
    }

    pub fn on_page_finished(app_id: String, path: String) -> Result<(), Box<dyn Error>> {
        info!(
            "Page finished loading for appId: {}, path: {}",
            app_id, path
        );
        if let Some(webviews) = WEBVIEWS.get() {
            let webviews = webviews.lock().unwrap();
            if let Some(webview) = WebViewManager::find_webview(&webviews, &app_id, &path) {
                webview.inject_bridge_script()?;
            }
        }
        Ok(())
    }

    pub fn should_override_url_loading(
        app_id: String,
        url: String,
    ) -> Result<bool, Box<dyn Error>> {
        info!(
            "Should override URL loading for appId: {}, URL: {}",
            app_id, url
        );
        // 这里可以根据需要处理 URL 重定向
        Ok(false)
    }

    pub fn handle_post_message(
        app_id: String,
        path: String,
        message_str: String,
    ) -> Result<(), Box<dyn Error>> {
        info!(
            "Handling message for WebView with appId {}: {}",
            app_id, message_str
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

    pub fn get_existing_webview(
        app_id: &str,
        path: &str,
    ) -> Result<Option<JObject<'static>>, Box<dyn Error>> {
        let mut env = get_env()?;
        if let Some(webviews) = WEBVIEWS.get() {
            let webviews = webviews.lock().unwrap();
            if let Some(webview) = Self::find_webview(&webviews, app_id, path) {
                let local_ref = env.new_local_ref(webview.java_webview.as_obj())?;
                unsafe { Ok(Some(JObject::from_raw(local_ref.into_raw()))) }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
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

// Helper function to get JNIEnv for current thread
fn get_env() -> Result<JNIEnv<'static>, Box<dyn Error>> {
    crate::get_env()
}
