use crate::android::get_env;
use jni::objects::{GlobalRef, JObject, JValue};
#[cfg(debug_assertions)]
use log::info;
use miniapp::{MiniAppError, WebViewController};

#[derive(Clone)]
pub struct WebView {
    #[cfg(debug_assertions)]
    appid: String,
    #[cfg(debug_assertions)]
    path: String,
    java_webview: GlobalRef,
}

impl Drop for WebView {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        info!(
            "Dropping WebView for appId: {}, path: {}",
            self.appid, self.path
        );

        let _ = self.destroy_webview();
    }
}

impl WebView {
    pub(crate) fn from_java(java_webview: JObject, _appid: String, _path: String) -> Self {
        let env = get_env().unwrap();
        let java_webview = env.new_global_ref(java_webview).unwrap();

        #[cfg(debug_assertions)]
        return WebView {
            appid: _appid,
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

impl WebViewController for WebView {
    fn load_url(&self, url: &str) -> Result<(), MiniAppError> {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

        match env.new_string(url) {
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

    fn evaluate_javascript(&self, js: &str) -> Result<(), MiniAppError> {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

        let script_string = match env.new_string(js) {
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
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

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
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

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

    fn post_message(&self, message: &str) -> Result<(), MiniAppError> {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

        let msg_string = match env.new_string(message) {
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

    fn set_user_agent(&self, ua: &str) -> Result<(), MiniAppError> {
        let mut env = match get_env() {
            Ok(env) => env,
            Err(_) => {
                return Err(MiniAppError::WebView(
                    "Failed to get JNI environment".to_string(),
                ));
            }
        };

        let ua_string = match env.new_string(ua) {
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
}
