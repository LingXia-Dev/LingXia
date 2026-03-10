use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, WebTag,
    WebViewCreateSender, WebViewCreateStage,
};
use crate::{LoadDataRequest, WebViewController, WebViewError};
use jni::objects::{Global, JObject};
use jni::{jni_sig, jni_str};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// Import JNI environment access from shared utils
use super::jni_env::{get_lingxia_webview_class, with_env};

fn encode_options_token(options: &EffectiveWebViewCreateOptions) -> Result<String, WebViewError> {
    let json = serde_json::to_vec(options).map_err(|e| {
        WebViewError::InvalidCreateOptions(format!("Serialize options failed: {e}"))
    })?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

// Type alias for WebView senders map to reduce complexity
pub(crate) struct PendingWebViewCreation {
    pub sender: WebViewCreateSender,
    pub effective_options: EffectiveWebViewCreateOptions,
}

type WebViewSendersMap = Arc<Mutex<HashMap<String, PendingWebViewCreation>>>;

// Global map to store senders for WebView creation
pub(crate) static WEBVIEW_SENDERS: OnceLock<WebViewSendersMap> = OnceLock::new();

pub(crate) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    let host = config.map(|cfg| cfg.host.as_str());
    let port = config.map(|cfg| cfg.port as i32).unwrap_or(0);
    let bypass = config.map(|cfg| cfg.bypass.clone()).unwrap_or_default();

    with_env(
        |env| -> Result<ProxyApplyReport, Box<dyn std::error::Error>> {
            let webview_class =
                get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
            let host_obj = match host {
                Some(value) => JObject::from(env.new_string(value)?),
                None => JObject::null(),
            };

            let bypass_array = env.new_object_array(
                bypass.len() as i32,
                jni_str!("java/lang/String"),
                JObject::null(),
            )?;
            for (idx, rule) in bypass.iter().enumerate() {
                let rule_string = env.new_string(rule)?;
                bypass_array.set_element(env, idx, &rule_string)?;
            }
            let bypass_arg = JObject::from(bypass_array);

            let result = env.call_static_method(
                webview_class,
                jni_str!("applyHttpProxy"),
                jni_sig!("(Ljava/lang/String;I[Ljava/lang/String;)Ljava/lang/String;"),
                &[(&host_obj).into(), port.into(), (&bypass_arg).into()],
            )?;

            let error_obj = result.l()?;
            if !error_obj.is_null() {
                let error_jstring = jni::objects::JString::cast_local(env, error_obj)?;
                let error = error_jstring.try_to_string(env)?;
                if let Some(detail) = error.strip_prefix("UNSUPPORTED:") {
                    return Ok(ProxyApplyReport::unsupported(detail.trim()));
                }
                return Err(format!("Android proxy apply failed: {}", error).into());
            }

            let report = if config.is_some() {
                ProxyApplyReport::applied(ProxyActivation::EffectiveNow)
            } else {
                ProxyApplyReport::cleared(ProxyActivation::EffectiveNow)
            };
            Ok(report)
        },
    )
    .map_err(|e| WebViewError::WebView(format!("Failed to apply Android proxy: {:?}", e)))
}

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
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        // Store sender in global map for callback
        let webtag = WebTag::new(appid, path, session_id);
        let senders = WEBVIEW_SENDERS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

        if let Ok(mut senders_map) = senders.lock() {
            senders_map.insert(
                webtag.to_string(),
                PendingWebViewCreation {
                    sender,
                    effective_options: effective_options.clone(),
                },
            );
        }

        // Helper function to remove sender and send error
        let remove_and_send_error = |error_msg: String| {
            if let Ok(mut senders_map) = senders.lock()
                && let Some(pending) = senders_map.remove(&webtag.to_string())
            {
                pending.sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(error_msg),
                );
            }
        };

        let appid_owned = appid.to_string();
        let path_owned = path.to_string();
        let options_token = match encode_options_token(&effective_options) {
            Ok(token) => token,
            Err(e) => {
                remove_and_send_error(format!("Failed to encode create options token: {e}"));
                return;
            }
        };

        // Get JNI environment via closure
        let result = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            // Get WebView class reference
            let webview_class =
                get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;

            // Create Java strings
            let appid_jstring = env.new_string(&appid_owned)?;
            let path_jstring = env.new_string(&path_owned)?;
            let session = session_id.unwrap_or_default() as i64;
            let options_jstring = env.new_string(&options_token)?;

            // Require the new API with options token.
            // If this fails, Java/Rust artifacts are mismatched and must be rebuilt together.
            env.call_static_method(
                webview_class,
                jni_str!("requestWebView"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;)V"),
                &[
                    (&appid_jstring).into(),
                    (&path_jstring).into(),
                    session.into(),
                    (&options_jstring).into(),
                ],
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
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
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

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let data_string = env.new_string(request.data)?;
            let base_url_string = env.new_string(request.base_url)?;
            let history_url_string = match request.history_url {
                Some(url) => env.new_string(&url)?,
                None => env.new_string(request.base_url)?,
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

    fn evaluate_javascript(&self, js: &str) -> Result<(), WebViewError> {
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

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
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

    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError> {
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
