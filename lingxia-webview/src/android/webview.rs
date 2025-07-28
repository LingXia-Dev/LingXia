use crate::android::{LXAPP_CLASS, get_env};
use jni::objects::{GlobalRef, JClass, JObject, JValue};
use miniapp::{LxAppError, WebViewController};

#[derive(Debug)]
pub struct WebViewInner {
    java_webview: GlobalRef,
}

impl WebViewInner {
    /// Create a new WebView by calling Kotlin createWebView
    pub(crate) fn create(appid: &str, path: &str) -> Result<Self, LxAppError> {
        use jni::objects::JValue;

        let mut env = get_env().unwrap();

        let lxapp_class: &JClass = LXAPP_CLASS
            .get()
            .ok_or_else(|| {
                LxAppError::WebView("Global LxApp class reference not available".to_string())
            })?
            .as_obj()
            .into();

        let appid_jstring = env.new_string(appid).unwrap();
        let path_jstring = env.new_string(path).unwrap();

        // Call Kotlin createWebView method
        let webview_result = env
            .call_static_method(
                lxapp_class,
                "createWebView",
                "(Ljava/lang/String;Ljava/lang/String;)Lcom/lingxia/lxapp/WebView;",
                &[
                    JValue::Object(&appid_jstring),
                    JValue::Object(&path_jstring),
                ],
            )
            .map_err(|e| LxAppError::WebView(format!("Failed to call createWebView: {:?}", e)))?;

        let java_webview = webview_result
            .l()
            .map_err(|e| LxAppError::WebView(format!("Failed to get WebView object: {:?}", e)))?;

        if java_webview.is_null() {
            return Err(LxAppError::WebView(
                "createWebView returned null".to_string(),
            ));
        }

        let java_webview = env
            .new_global_ref(java_webview)
            .map_err(|e| LxAppError::WebView(format!("Failed to create global ref: {:?}", e)))?;

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
    fn load_url(&self, url: String) -> Result<(), LxAppError> {
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
                    Err(LxAppError::WebView("Failed to load URL".to_string()))
                }
            }
            Err(_) => Err(LxAppError::WebView(
                "Failed to create Java string".to_string(),
            )),
        }
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), LxAppError> {
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
            Err(LxAppError::WebView("Failed to load data".to_string()))
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), LxAppError> {
        let mut env = get_env().unwrap();

        let script_string = match env.new_string(&js) {
            Ok(s) => s,
            Err(_) => {
                return Err(LxAppError::WebView(
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
            Err(LxAppError::WebView(
                "JavaScript evaluation failed".to_string(),
            ))
        }
    }

    fn clear_browsing_data(&self) -> Result<(), LxAppError> {
        let mut env = get_env().unwrap();
        let result = env.call_method(self.java_webview.as_obj(), "clearBrowsingData", "()V", &[]);

        if result.is_ok() {
            Ok(())
        } else {
            Err(LxAppError::WebView(
                "Failed to clear browsing data".to_string(),
            ))
        }
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), LxAppError> {
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
                    Err(LxAppError::WebView("Failed to set devtools".to_string()))
                }
            }
            Err(_) => Err(LxAppError::WebView(
                "Failed to find WebView class".to_string(),
            )),
        }
    }

    fn post_message(&self, message: String) -> Result<(), LxAppError> {
        let mut env = get_env().unwrap();

        let msg_string = match env.new_string(&message) {
            Ok(s) => s,
            Err(_) => {
                return Err(LxAppError::WebView(
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
            Err(LxAppError::WebView("Failed to post message".to_string()))
        }
    }

    fn set_user_agent(&self, ua: String) -> Result<(), LxAppError> {
        let mut env = get_env().unwrap();

        let ua_string = match env.new_string(&ua) {
            Ok(s) => s,
            Err(_) => {
                return Err(LxAppError::WebView(
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
            Err(LxAppError::WebView("Failed to set user agent".to_string()))
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), LxAppError> {
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
            Err(LxAppError::WebView(
                "Failed to set scroll listener enabled".to_string(),
            ))
        }
    }
}
