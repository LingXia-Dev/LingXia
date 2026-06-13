//! Document-created script injection for Windows WebView2.

use super::*;

pub(crate) fn install_document_scripts(webview: &ICoreWebView2) -> StdResult<()> {
    let script = r#"
        (function() {
            if (window.__LingXiaWindowsInjected) return;
            window.__LingXiaWindowsInjected = true;

            if (window.chrome && window.chrome.webview && !window.__LingXiaNativeMessageListener) {
                window.__LingXiaNativeMessageListener = true;
                window.chrome.webview.addEventListener('message', function(event) {
                    try {
                        var payload = typeof event.data === 'string' ? event.data : JSON.stringify(event.data);
                        if (typeof window.__LingXiaRecvMessage === 'function') {
                            window.__LingXiaRecvMessage(payload);
                        } else {
                            console.warn('[LingXia] __LingXiaRecvMessage not available');
                        }
                    } catch (e) {}
                });
            }

            // Windows uses WebView2's native web message channel
            // (`chrome.webview.postMessage` -> `WebMessageReceived`). WebView2
            // does not expose Android-style WebMessagePort transfer to Win32,
            // so `supportsMessagePort` intentionally stays false.
            window.LingXiaProxy = window.LingXiaProxy || {
                supportsMessagePort: function() { return false; },
                getPort: function() { return ''; },
                postMessage: function(message) {
                    window.chrome && window.chrome.webview && window.chrome.webview.postMessage(String(message));
                }
            };

            // Embedded native components: the page bridge posts component
            // messages (component.mount/update/unmount, ...) through this
            // object; they travel in a tagged envelope so the host can route
            // them separately from regular bridge traffic. A lightweight
            // scroll tracker keeps native overlays aligned with document
            // coordinates; it stays dormant until a component message is
            // actually sent.
            if (!window.NativeComponentBridge) {
                var lxNcActive = false;
                var lxNcScrollScheduled = false;
                var lxNcPost = function(payload) {
                    try {
                        window.chrome && window.chrome.webview && window.chrome.webview.postMessage(JSON.stringify({
                            __lingxia_native_component__: true,
                            payload: String(payload)
                        }));
                    } catch (e) {}
                };
                var lxNcPostScroll = function() {
                    lxNcScrollScheduled = false;
                    lxNcPost(JSON.stringify({
                        action: 'page.scroll',
                        x: window.scrollX || 0,
                        y: window.scrollY || 0
                    }));
                };
                var lxNcScheduleScroll = function() {
                    if (!lxNcActive || lxNcScrollScheduled) return;
                    lxNcScrollScheduled = true;
                    window.requestAnimationFrame(lxNcPostScroll);
                };
                window.addEventListener('scroll', lxNcScheduleScroll, { passive: true });
                window.addEventListener('resize', lxNcScheduleScroll);
                window.NativeComponentBridge = {
                    postMessage: function(message) {
                        if (!lxNcActive) {
                            lxNcActive = true;
                            lxNcPostScroll();
                        }
                        lxNcPost(message);
                    }
                };
            }

            if (window.__LingXiaConsoleInjected) return;
            window.__LingXiaConsoleInjected = true;
            ['log', 'info', 'warn', 'error', 'debug'].forEach(function(level) {
                var original = console[level];
                console[level] = function() {
                    try {
                        var msg = Array.prototype.map.call(arguments, function(arg) {
                            return typeof arg === 'object' ? JSON.stringify(arg) : String(arg);
                        }).join(' ');
                        window.chrome && window.chrome.webview && window.chrome.webview.postMessage(JSON.stringify({
                            __lingxia_console__: true,
                            level: level,
                            message: msg
                        }));
                    } catch (e) {}
                    if (original) return original.apply(console, arguments);
                };
            });
        })();
    "#;

    let webview = webview.clone();
    let script = script.to_string();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let script = CoTaskMemPWSTR::from(script.as_str());
            webview
                .AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|result, _id| result),
    )
    .map_err(map_webview2_error)?;

    Ok(())
}
