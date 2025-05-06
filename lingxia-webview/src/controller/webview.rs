use crate::WebView;
use miniapp::{MiniAppError, WebViewCmd, WebViewController};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// WebView message handler responsible for processing WebView commands from the UI thread
pub(crate) fn handle_webview_cmd(
    webviews: &Mutex<HashMap<(String, String), Arc<WebView>>>,
    cmd: WebViewCmd,
) -> bool {
    match cmd {
        WebViewCmd::LoadUrl {
            appid,
            path,
            url,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    webviews
                        .get(&(appid.clone(), path.clone()))
                        .ok_or_else(|| MiniAppError::WebView("WebView not found".to_string()))
                        .and_then(|webview| webview.load_url(&url))
                });

            let _ = responder.send(result);
            true // Continue processing requests
        }
        WebViewCmd::EvaluateJavascript {
            appid,
            path,
            script,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    webviews
                        .get(&(appid.clone(), path.clone()))
                        .ok_or_else(|| MiniAppError::WebView("WebView not found".to_string()))
                        .and_then(|webview| {
                            webview.evaluate_javascript(&script).map(|_| String::new())
                        })
                });

            // Convert Result<String, MiniAppError> to Result<(), MiniAppError>
            let adapted_result = result.map(|_| ());

            let _ = responder.send(adapted_result);
            true
        }
        WebViewCmd::PostMessage {
            appid,
            path,
            message,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    webviews
                        .get(&(appid.clone(), path.clone()))
                        .ok_or_else(|| MiniAppError::WebView("WebView not found".to_string()))
                        .and_then(|webview| webview.post_message(&message))
                });

            let _ = responder.send(result);
            true
        }
        WebViewCmd::SetDevtools {
            appid,
            enabled,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    let key = webviews.keys().find(|(id, _)| id == &appid);
                    key.and_then(|k| webviews.get(k))
                        .ok_or_else(|| {
                            MiniAppError::WebView("No WebView found for appid".to_string())
                        })
                        .and_then(|webview| webview.set_devtools(enabled))
                });

            let _ = responder.send(result);
            true
        }
        WebViewCmd::ClearBrowsingData {
            appid,
            path,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    webviews
                        .get(&(appid.clone(), path.clone()))
                        .ok_or_else(|| MiniAppError::WebView("WebView not found".to_string()))
                        .and_then(|webview| webview.clear_browsing_data())
                });

            let _ = responder.send(result);
            true
        }
        WebViewCmd::SetUserAgent {
            appid,
            ua,
            responder,
        } => {
            let result = webviews
                .lock()
                .map_err(|_| MiniAppError::WebView("Failed to lock webviews".to_string()))
                .and_then(|webviews| {
                    let found = false;
                    let mut result = Err(MiniAppError::WebView(
                        "No WebView found for appid".to_string(),
                    ));

                    // Find all webviews for this app and set UA
                    for ((id, _), webview) in webviews.iter().filter(|((id, _), _)| id == &appid) {
                        result = webview.set_user_agent(&ua);
                        if result.is_ok() {
                            return Ok(());
                        }
                    }

                    result
                });

            let _ = responder.send(result);
            true
        }
    }
}
