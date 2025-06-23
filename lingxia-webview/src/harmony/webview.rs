use crate::harmony::ffi::CALLBACK_TSFN;
use crate::harmony::schemehandler::set_webview_scheme_handler;
use miniapp::{MiniAppError, WebViewController};
use napi_ohos::{Result as NapiResult, Status, threadsafe_function::ThreadsafeFunctionCallMode};
use ohos_web_sys::*;
use std::ffi::CString;

#[derive(Debug)]
pub struct WebViewInner {
    webtag: String,
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    /// Create a new WebView instance for HarmonyOS
    pub fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        let webtag = format!("{}:{}", appid, path);

        // Call ArkTS to create WebView controller first
        match create_webview_controller(appid, path) {
            Ok(_) => {
                // Set scheme handler for this WebView
                if let Err(e) = set_webview_scheme_handler(&webtag) {
                    return Err(MiniAppError::WebView(format!(
                        "Failed to set scheme handler: {}",
                        e
                    )));
                }

                Ok(WebViewInner { webtag })
            }
            Err(e) => Err(MiniAppError::WebView(format!(
                "Failed to create WebView: {}",
                e
            ))),
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
        log::info!(
            "WebViewController::load_url called for {}: {}",
            self.webtag,
            url
        );

        unsafe {
            let web_tag_cstr = CString::new(self.webtag.clone()).unwrap();

            // Use HTML redirect approach
            let html = format!(
                r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
</head>
<body>
    <script>
        window.location.href = '{}';
    </script>
</body>
</html>
"#,
                url
            );

            let data_cstr = CString::new(html).unwrap();

            // Use HTML content with redirect
            let base_url_cstr = CString::new(url.clone()).unwrap();
            let result = OH_NativeArkWeb_LoadData(
                web_tag_cstr.as_ptr(),
                data_cstr.as_ptr(),
                c"text/html".as_ptr(),
                c"UTF-8".as_ptr(),
                base_url_cstr.as_ptr(),
                c"".as_ptr(),
            );

            if result == 0 {
                log::info!("Successfully loaded URL {} in WebView {}", url, self.webtag);
                Ok(())
            } else {
                log::error!(
                    "Failed to load URL {} in WebView {}, error: {}",
                    url,
                    self.webtag,
                    result
                );
                Err(MiniAppError::WebView(format!(
                    "Failed to load URL: error code {}",
                    result
                )))
            }
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        log::info!(
            "WebViewController::evaluate_javascript called for {}: {}",
            self.webtag,
            js
        );

        unsafe {
            let web_tag_cstr = CString::new(self.webtag.clone()).unwrap();
            let js_cstr = CString::new(js.clone()).unwrap();

            // No callback needed since we don't want the execution result
            OH_NativeArkWeb_RunJavaScript(web_tag_cstr.as_ptr(), js_cstr.as_ptr(), None);

            log::info!(
                "Successfully submitted JavaScript for evaluation in WebView {}",
                self.webtag
            );
            Ok(())
        }
    }

    fn set_devtools(&self, _enabled: bool) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_user_agent(&self, _ua: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_scroll_listener_enabled(
        &self,
        _enabled: bool,
        _throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        Ok(())
    }
}

/// Create WebView controller in ArkTS
pub fn create_webview_controller(app_id: &str, path: &str) -> NapiResult<()> {
    if let Some(tsfn) = CALLBACK_TSFN.get() {
        // Node-API ThreadSafe Function limitation: can only pass single string
        // Format: "function_name:arg1:arg2:..."
        let data = format!("createWebViewController:{}:{}", app_id, path);
        let status = tsfn.call(data, ThreadsafeFunctionCallMode::Blocking);
        if status == Status::Ok {
            log::info!(
                "Successfully created WebView controller for {}:{}",
                app_id,
                path
            );
            Ok(())
        } else {
            log::error!("Failed to create WebView controller: {:?}", status);
            Err(napi_ohos::Error::new(
                Status::GenericFailure,
                format!("Failed to create WebView controller: {:?}", status),
            ))
        }
    } else {
        log::error!("No callback available");
        Err(napi_ohos::Error::new(
            Status::GenericFailure,
            "No callback available".to_string(),
        ))
    }
}

/// Destroy WebView controller in ArkTS
pub fn destroy_webview_controller(appid: &str, path: &str) -> NapiResult<()> {
    if let Some(tsfn) = CALLBACK_TSFN.get() {
        // Node-API ThreadSafe Function limitation: can only pass single string
        // Format: "function_name:arg1:arg2:..."
        let data = format!("destroyWebViewController:{}:{}", appid, path);
        let status = tsfn.call(data, ThreadsafeFunctionCallMode::Blocking);
        if status == Status::Ok {
            log::info!(
                "Successfully destroyed WebView controller: {}:{}",
                appid,
                path
            );
            Ok(())
        } else {
            log::error!("Failed to destroy WebView controller: {:?}", status);
            Err(napi_ohos::Error::new(
                Status::GenericFailure,
                format!("Failed to destroy WebView controller: {:?}", status),
            ))
        }
    } else {
        log::error!("No callback available");
        Err(napi_ohos::Error::new(
            Status::GenericFailure,
            "No callback available".to_string(),
        ))
    }
}
