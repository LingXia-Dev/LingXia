//! Document-created script injection for Windows WebView2.

use super::*;

/// Platform/selection baseline injected for lxapp pages (not browser tabs).
///
/// The runtime owns the selection/copy policy per platform so apps don't bake
/// a build-time assumption into their CSS. Windows is a desktop platform, so the
/// baseline mirrors the macOS desktop policy: text is selectable by default and
/// apps opt out per element with `.no-select` / `[data-lx-no-select]`. Tags the
/// document with `lx-desktop` + `data-lx-platform="windows"` at document-start.
/// The CSS contains no single quotes or newlines, so single-quoting is safe.
const PLATFORM_BASELINE_SCRIPT: &str = concat!(
    "(function(){try{",
    "var el=document.documentElement;",
    "el.classList.add('lx-desktop');",
    "el.setAttribute('data-lx-platform','windows');",
    "var s=document.createElement('style');",
    "s.setAttribute('data-lingxia-base','');",
    "s.textContent='",
    "html.lx-desktop,html.lx-desktop body{-webkit-user-select:text;user-select:text;}",
    "html.lx-desktop .no-select,html.lx-desktop [data-lx-no-select]{-webkit-user-select:none;user-select:none;}",
    "';(document.head||el).appendChild(s);",
    "}catch(e){}})();"
);

pub(crate) fn install_document_scripts(
    webview: &ICoreWebView2,
    inject_platform_baseline: bool,
) -> StdResult<()> {
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
    // Prepend the selection baseline (its own IIFE) for lxapp pages so it runs
    // at document-start ahead of the bridge plumbing below.
    let script = if inject_platform_baseline {
        format!("{PLATFORM_BASELINE_SCRIPT}{script}")
    } else {
        script.to_string()
    };
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
