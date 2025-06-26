use crate::android::get_env;
use jni::objects::{GlobalRef, JObject, JValue};
use miniapp::{MiniAppError, WebViewController};

#[derive(Clone, Debug)]
pub struct WebViewInner {
    java_webview: GlobalRef,
}

impl WebViewInner {
    /// Create a new WebView by calling Kotlin createWebView
    pub(crate) fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        use jni::objects::JValue;

        let mut env = get_env().unwrap();

        let miniapp_class = env
            .find_class("com/lingxia/miniapp/MiniApp")
            .map_err(|e| MiniAppError::WebView(format!("Failed to find MiniApp class: {:?}", e)))?;

        let appid_jstring = env.new_string(appid).unwrap();
        let path_jstring = env.new_string(path).unwrap();

        // Call Kotlin createWebView method
        let webview_result = env
            .call_static_method(
                &miniapp_class,
                "createWebView",
                "(Ljava/lang/String;Ljava/lang/String;)Lcom/lingxia/miniapp/WebView;",
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                ],
            )
            .map_err(|e| MiniAppError::WebView(format!("Failed to call createWebView: {:?}", e)))?;

        let java_webview = webview_result
            .l()
            .map_err(|e| MiniAppError::WebView(format!("Failed to get WebView object: {:?}", e)))?;

        if java_webview.is_null() {
            return Err(MiniAppError::WebView(
                "createWebView returned null".to_string(),
            ));
        }

        let java_webview = env
            .new_global_ref(java_webview)
            .map_err(|e| MiniAppError::WebView(format!("Failed to create global ref: {:?}", e)))?;

        Ok(WebViewInner { java_webview })
    }

    pub(crate) fn get_java_webview(&self) -> &GlobalRef {
        &self.java_webview
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        if let Ok(mut env) = get_env() {
            let _ = env.call_method(self.java_webview.as_obj(), "destroy", "()V", &[]);
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
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
                    Err(MiniAppError::WebView("Failed to load URL".to_string()))
                }
            }
            Err(_) => Err(MiniAppError::WebView(
                "Failed to create Java string".to_string(),
            )),
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        let mut env = get_env().unwrap();

        let script_string = match env.new_string(&js) {
            Ok(s) => s,
            Err(_) => {
                return Err(MiniAppError::WebView(
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
            Err(MiniAppError::WebView(
                "JavaScript evaluation failed".to_string(),
            ))
        }
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        let mut env = get_env().unwrap();
        let result = env.call_method(self.java_webview.as_obj(), "clearBrowsingData", "()V", &[]);

        if result.is_ok() {
            Ok(())
        } else {
            Err(MiniAppError::WebView(
                "Failed to clear browsing data".to_string(),
            ))
        }
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        let mut env = get_env().unwrap();

        match env.find_class("android/webkit/WebView") {
            Ok(webview_class) => {
                let result = env.call_static_method(
                    webview_class,
                    "setWebContentsDebuggingEnabled",
                    "(Z)V",
                    &[JValue::Bool(enabled as u8)],
                );

                if result.is_ok() {
                    Ok(())
                } else {
                    Err(MiniAppError::WebView("Failed to set devtools".to_string()))
                }
            }
            Err(_) => Err(MiniAppError::WebView(
                "Failed to find WebView class".to_string(),
            )),
        }
    }

    fn post_message(&self, message: String) -> Result<(), MiniAppError> {
        let mut env = get_env().unwrap();

        let msg_string = match env.new_string(&message) {
            Ok(s) => s,
            Err(_) => {
                return Err(MiniAppError::WebView(
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
            Err(MiniAppError::WebView("Failed to post message".to_string()))
        }
    }

    fn set_user_agent(&self, ua: String) -> Result<(), MiniAppError> {
        let mut env = get_env().unwrap();

        let ua_string = match env.new_string(&ua) {
            Ok(s) => s,
            Err(_) => {
                return Err(MiniAppError::WebView(
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
            Err(MiniAppError::WebView(
                "Failed to set user agent".to_string(),
            ))
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
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
            Err(MiniAppError::WebView(
                "Failed to set scroll listener enabled".to_string(),
            ))
        }
    }
}
