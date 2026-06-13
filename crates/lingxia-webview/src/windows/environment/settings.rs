//! WebView2 controller and settings setup.

use super::*;

pub(crate) fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> StdResult<ICoreWebView2Controller> {
    let env = env.clone();
    let (tx, rx) = mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, controller| {
            result?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Controller callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Controller creation failed: {err}")))
}

pub(crate) fn configure_controller(controller: &ICoreWebView2Controller) -> StdResult<()> {
    unsafe {
        controller
            .SetBounds(RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            })
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        controller
            .SetIsVisible(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn configure_settings(
    webview: &ICoreWebView2,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<()> {
    let relaxed_profile = effective_options.profile == SecurityProfile::BrowserRelaxed;
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        settings
            .SetIsScriptEnabled(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsScriptEnabled failed: {err}")))?;
        settings
            .SetAreDefaultScriptDialogsEnabled(relaxed_profile)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultScriptDialogsEnabled failed: {err}"))
            })?;
        settings
            .SetAreDefaultContextMenusEnabled(relaxed_profile)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultContextMenusEnabled failed: {err}"))
            })?;
        settings.SetIsWebMessageEnabled(true).map_err(|err| {
            WebViewError::WebView(format!("SetIsWebMessageEnabled failed: {err}"))
        })?;
        settings
            .SetIsStatusBarEnabled(false)
            .map_err(|err| WebViewError::WebView(format!("SetIsStatusBarEnabled failed: {err}")))?;
        settings
            .SetAreDevToolsEnabled(webview_devtools_enabled())
            .map_err(|err| WebViewError::WebView(format!("SetAreDevToolsEnabled failed: {err}")))?;
    }
    Ok(())
}
