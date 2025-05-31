use crate::webview::WebViewCmd;
use miniapp::{MiniAppError, WebViewController};

/// WebView message handler responsible for processing WebView commands from the UI thread
pub(crate) fn handle_webview_cmd(cmd: WebViewCmd) -> Result<(), MiniAppError> {
    match cmd {
        WebViewCmd::LoadUrl {
            webview,
            url,
            responder,
        } => {
            let result = webview.inner().load_url(url);
            let _ = responder.send(result);
            Ok(())
        }
        WebViewCmd::EvaluateJavascript {
            webview,
            script,
            responder,
        } => {
            let result = webview.inner().evaluate_javascript(script);
            let _ = responder.send(result);
            Ok(())
        }
        WebViewCmd::PostMessage {
            webview,
            message,
            responder,
        } => {
            let result = webview.inner().post_message(message);
            let _ = responder.send(result);
            Ok(())
        }
        WebViewCmd::SetDevtools {
            webview,
            enabled,
            responder,
        } => {
            let result = webview.inner().set_devtools(enabled);
            let _ = responder.send(result);
            Ok(())
        }
        WebViewCmd::ClearBrowsingData { webview, responder } => {
            let result = webview.inner().clear_browsing_data();
            let _ = responder.send(result);
            Ok(())
        }
        WebViewCmd::SetUserAgent {
            webview,
            ua,
            responder,
        } => {
            let result = webview.inner().set_user_agent(ua);
            let _ = responder.send(result);
            Ok(())
        }
    }
}
