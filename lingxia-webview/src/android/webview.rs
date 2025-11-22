use crate::{WebViewController, WebViewError};
use jni::objects::{GlobalRef, JObject, JValue};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot::Sender;

// Import JNI environment access from shared utils
use super::jni_env::{get_env, get_lingxia_webview_class};

// Import WebTag from the main webview module
use crate::webview::WebTag;

// Type alias for WebView senders map to reduce complexity
type WebViewSendersMap =
    Arc<Mutex<HashMap<String, Sender<Result<Arc<crate::WebView>, WebViewError>>>>>;

// Global map to store senders for WebView creation
pub(crate) static WEBVIEW_SENDERS: OnceLock<WebViewSendersMap> = OnceLock::new();

#[derive(Debug)]
pub struct WebViewInner {
    java_webview: GlobalRef,
    pub(crate) webtag: WebTag,
}

impl WebViewInner {
    /// Create a new WebView asynchronously by calling Java static method and storing sender
    pub(crate) fn create(
        appid: &str,
        path: &str,
        _session_id: Option<u64>,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        // Store sender in global map for callback
        let webtag = WebTag::new(appid, path, None);
        let senders = WEBVIEW_SENDERS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

        if let Ok(mut senders_map) = senders.lock() {
            senders_map.insert(webtag.to_string(), sender);
        }

        // Helper function to remove sender and send error
        let remove_and_send_error = |error_msg: String| {
            if let Ok(mut senders_map) = senders.lock()
                && let Some(sender) = senders_map.remove(&webtag.to_string())
            {
                let _ = sender.send(Err(WebViewError::WebView(error_msg)));
            }
        };

        // Get JNI environment
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                log::error!("Failed to get JNI environment");
                remove_and_send_error("Failed to get JNI environment".to_string());
                return;
            }
        };

        // Get WebView class reference
        let webview_class = match get_lingxia_webview_class() {
            Some(class) => class,
            None => {
                log::error!("LingXiaWebView class not cached");
                remove_and_send_error("LingXiaWebView class not cached".to_string());
                return;
            }
        };

        // Create Java strings - these rarely fail
        let appid_jstring = env.new_string(appid).unwrap();
        let path_jstring = env.new_string(path).unwrap();

        // Call Java static method to request WebView creation
        match env.call_static_method(
            webview_class,
            "requestWebView",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&appid_jstring),
                JValue::Object(&path_jstring),
            ],
        ) {
            Ok(_) => log::info!(
                "Successfully requested WebView creation for {}-{}",
                appid,
                path
            ),
            Err(e) => {
                log::error!("Failed to call requestWebView: {:?}", e);
                remove_and_send_error(format!("Failed to call requestWebView: {:?}", e));
            }
        }
    }

    /// Create WebViewInner from existing Java WebView object (called from onWebViewReady)
    pub(crate) fn from_java_object(java_webview: GlobalRef, webtag: WebTag) -> Self {
        WebViewInner {
            java_webview,
            webtag,
        }
    }

    pub fn get_java_webview(&self) -> &GlobalRef {
        &self.java_webview
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        if let Ok(mut env) = get_env() {
            let _ = env.call_method(self.java_webview.as_obj(), "destroy", "()V", &[]);
        }
        log::info!(
            "[WebViewInner] Android WebViewInner dropped and destroyed ({})",
            self.webtag.as_str()
        );
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        match env.new_string(&url) {
            Ok(url_string) => {
                let result = env.call_method(
                    self.java_webview.as_obj(),
                    "loadUrl",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&url_string)],
                );

                if result.is_ok() {
                    Ok(())
                } else {
                    Err(WebViewError::WebView("Failed to load URL".to_string()))
                }
            }
            Err(_) => Err(WebViewError::WebView(
                "Failed to create Java string".to_string(),
            )),
        }
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        let data_string = env.new_string(&data).unwrap();
        let base_url_string = env.new_string(&base_url).unwrap();
        let history_url_string = match history_url {
            Some(url) => env.new_string(&url).unwrap(),
            None => env.new_string(&base_url).unwrap(),
        };

        let result = env.call_method(
            self.java_webview.as_obj(),
            "loadHtmlData",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&data_string),
                JValue::Object(&base_url_string),
                JValue::Object(&history_url_string),
            ],
        );

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView("Failed to load data".to_string()))
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        let script_string = match env.new_string(&js) {
            Ok(s) => s,
            Err(_) => {
                return Err(WebViewError::WebView(
                    "Failed to create Java string".to_string(),
                ));
            }
        };

        let result = env.call_method(
            self.java_webview.as_obj(),
            "evaluateJavascript",
            "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
            &[
                JValue::Object(&script_string),
                JValue::Object(&JObject::null()),
            ],
        );

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView(
                "JavaScript evaluation failed".to_string(),
            ))
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();
        let result = env.call_method(self.java_webview.as_obj(), "clearBrowsingData", "()V", &[]);

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView(
                "Failed to clear browsing data".to_string(),
            ))
        }
    }

    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        let msg_string = match env.new_string(&message) {
            Ok(s) => s,
            Err(_) => {
                return Err(WebViewError::WebView(
                    "Failed to create Java string".to_string(),
                ));
            }
        };

        let result = env.call_method(
            self.java_webview.as_obj(),
            "postMessageToWebView",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&msg_string)],
        );

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView("Failed to post message".to_string()))
        }
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        let ua_string = match env.new_string(&ua) {
            Ok(s) => s,
            Err(_) => {
                return Err(WebViewError::WebView(
                    "Failed to create Java string".to_string(),
                ));
            }
        };

        let result = env.call_method(
            self.java_webview.as_obj(),
            "setUserAgent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&ua_string)],
        );

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView(
                "Failed to set user agent".to_string(),
            ))
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError> {
        let mut env = get_env().unwrap();

        let throttle_value = throttle_ms.unwrap_or(100); // Default to 100ms

        let result = env.call_method(
            self.java_webview.as_obj(),
            "setScrollListenerEnabled",
            "(ZJ)V",
            &[
                JValue::Bool(enabled as u8),
                JValue::Long(throttle_value as i64),
            ],
        );

        if result.is_ok() {
            Ok(())
        } else {
            Err(WebViewError::WebView(
                "Failed to set scroll listener enabled".to_string(),
            ))
        }
    }
}
