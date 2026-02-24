use crate::{WebViewController, WebViewError};
use jni::objects::{Global, JObject};
use jni::{jni_sig, jni_str};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot::Sender;

// Import JNI environment access from shared utils
use super::jni_env::{get_lingxia_webview_class, with_env};

// Import WebTag from the main webview module
use crate::webview::WebTag;

// Type alias for WebView senders map to reduce complexity
type WebViewSendersMap =
    Arc<Mutex<HashMap<String, Sender<Result<Arc<crate::WebView>, WebViewError>>>>>;

// Global map to store senders for WebView creation
pub(crate) static WEBVIEW_SENDERS: OnceLock<WebViewSendersMap> = OnceLock::new();

#[derive(Debug)]
pub struct WebViewInner {
    java_webview: Global<JObject<'static>>,
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

        let appid_owned = appid.to_string();
        let path_owned = path.to_string();

        // Get JNI environment via closure
        let result = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            // Get WebView class reference
            let webview_class =
                get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;

            // Create Java strings
            let appid_jstring = env.new_string(&appid_owned)?;
            let path_jstring = env.new_string(&path_owned)?;

            // Call Java static method to request WebView creation
            env.call_static_method(
                webview_class,
                jni_str!("requestWebView"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                &[(&appid_jstring).into(), (&path_jstring).into()],
            )?;

            log::info!(
                "Successfully requested WebView creation for {}-{}",
                appid_owned,
                path_owned
            );
            Ok(())
        });

        if let Err(e) = result {
            log::error!("Failed to request WebView creation: {:?}", e);
            remove_and_send_error(format!("Failed to request WebView creation: {:?}", e));
        }
    }

    /// Create WebViewInner from existing Java WebView object (called from onWebViewReady)
    pub(crate) fn from_java_object(java_webview: Global<JObject<'static>>, webtag: WebTag) -> Self {
        WebViewInner {
            java_webview,
            webtag,
        }
    }

    pub fn get_java_webview(&self) -> &Global<JObject<'static>> {
        &self.java_webview
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        let _ = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let _ = env.call_method(
                &*self.java_webview,
                jni_str!("destroy"),
                jni_sig!("()V"),
                &[],
            );
            Ok(())
        });
        log::info!(
            "[WebViewInner] Android WebViewInner dropped and destroyed ({})",
            self.webtag.as_str()
        );
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let url_string = env.new_string(&url)?;
            env.call_method(
                &*self.java_webview,
                jni_str!("loadUrl"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&url_string).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to load URL: {:?}", e)))
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let data_string = env.new_string(&data)?;
            let base_url_string = env.new_string(&base_url)?;
            let history_url_string = match history_url {
                Some(url) => env.new_string(&url)?,
                None => env.new_string(&base_url)?,
            };

            env.call_method(
                &*self.java_webview,
                jni_str!("loadHtmlData"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
                &[
                    (&data_string).into(),
                    (&base_url_string).into(),
                    (&history_url_string).into(),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to load data: {:?}", e)))
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let script_string = env.new_string(&js)?;

            env.call_method(
                &*self.java_webview,
                jni_str!("evaluateJavascript"),
                jni_sig!("(Ljava/lang/String;Landroid/webkit/ValueCallback;)V"),
                &[(&script_string).into(), (&JObject::null()).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("JavaScript evaluation failed: {:?}", e)))
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            env.call_method(
                &*self.java_webview,
                jni_str!("clearBrowsingData"),
                jni_sig!("()V"),
                &[],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to clear browsing data: {:?}", e)))
    }

    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let msg_string = env.new_string(&message)?;

            env.call_method(
                &*self.java_webview,
                jni_str!("postMessageToWebView"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&msg_string).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to post message: {:?}", e)))
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let ua_string = env.new_string(&ua)?;

            env.call_method(
                &*self.java_webview,
                jni_str!("setUserAgent"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&ua_string).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to set user agent: {:?}", e)))
    }
}
