package com.lingxia.webview;

import android.util.Log;
import android.graphics.Bitmap;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.os.Message;
import android.webkit.ConsoleMessage;
import android.webkit.JsPromptResult;
import android.webkit.JsResult;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import java.lang.ref.WeakReference;

/**
 * WebChromeClient implementation for LingXia WebView
 */
public class LingXiaWebChromeClient extends WebChromeClient {
    private static final String TAG = "LingXiaWebChromeClient";
    private static final Handler MAIN_HANDLER = new Handler(Looper.getMainLooper());
    private final WeakReference<LingXiaWebView> webViewRef;
    private final boolean allowJsDialogs;

    public LingXiaWebChromeClient(LingXiaWebView webView, boolean allowJsDialogs) {
        this.webViewRef = new WeakReference<>(webView);
        this.allowJsDialogs = allowJsDialogs;
    }

    @Override
    public boolean onConsoleMessage(ConsoleMessage message) {
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            int level = getLogLevel(message.messageLevel());
            webView.onConsoleMessage(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId(),
                level,
                message.message()
            );
        }
        return true;
    }

    @Override
    public void onProgressChanged(WebView view, int newProgress) {
        super.onProgressChanged(view, newProgress);
        Log.d(TAG, "Loading progress: " + newProgress + "%");
    }

    @Override
    public void onReceivedTitle(WebView view, String title) {
        super.onReceivedTitle(view, title);
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.pushWebViewState();
        }
    }

    @Override
    public void onReceivedIcon(WebView view, Bitmap icon) {
        super.onReceivedIcon(view, icon);
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.pushFavicon(icon);
        }
    }

    @Override
    public boolean onCreateWindow(
        WebView view,
        boolean isDialog,
        boolean isUserGesture,
        Message resultMsg
    ) {
        LingXiaWebView webView = webViewRef.get();
        if (webView == null || !webView.supportsNewWindows()) {
            return false;
        }

        String hitUrl = null;
        try {
            WebView.HitTestResult hit = view != null ? view.getHitTestResult() : null;
            hitUrl = hit != null ? hit.getExtra() : null;
        } catch (Throwable t) {
            Log.w(TAG, "Failed to read new-window hit-test URL", t);
        }
        if (isUsableNewWindowUrl(hitUrl)) {
            webView.handleNewWindowRequest(hitUrl);
            return false;
        }

        if (view == null || resultMsg == null || !(resultMsg.obj instanceof WebView.WebViewTransport)) {
            return false;
        }

        final boolean[] handled = new boolean[] { false };
        final WebView popup = new WebView(view.getContext());
        popup.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView child, WebResourceRequest request) {
                if (request == null || request.getUrl() == null) {
                    return false;
                }
                return captureTarget(child, request.getUrl().toString());
            }

            @Override
            @SuppressWarnings("deprecation")
            public boolean shouldOverrideUrlLoading(WebView child, String url) {
                return captureTarget(child, url);
            }

            @Override
            public void onPageStarted(WebView child, String url, Bitmap favicon) {
                if (captureTarget(child, url)) {
                    return;
                }
                super.onPageStarted(child, url, favicon);
            }

            private boolean captureTarget(WebView child, String rawUrl) {
                if (handled[0] || !isUsableNewWindowUrl(rawUrl)) {
                    return false;
                }
                handled[0] = true;
                webView.handleNewWindowRequest(rawUrl);
                destroyProbe(child);
                return true;
            }
        });

        MAIN_HANDLER.postDelayed(new Runnable() {
            @Override
            public void run() {
                if (!handled[0]) {
                    handled[0] = true;
                    destroyProbe(popup);
                }
            }
        }, 5000L);

        WebView.WebViewTransport transport = (WebView.WebViewTransport) resultMsg.obj;
        transport.setWebView(popup);
        resultMsg.sendToTarget();
        return true;
    }

    @Override
    public boolean onJsAlert(WebView view, String url, String message, JsResult result) {
        if (allowJsDialogs) {
            return super.onJsAlert(view, url, message, result);
        }

        Log.i(TAG, "Suppressed JavaScript alert in strict profile: " + url);
        if (result != null) {
            result.confirm();
        }
        return true;
    }

    @Override
    public boolean onJsConfirm(WebView view, String url, String message, JsResult result) {
        if (allowJsDialogs) {
            return super.onJsConfirm(view, url, message, result);
        }

        Log.i(TAG, "Suppressed JavaScript confirm in strict profile: " + url);
        if (result != null) {
            result.cancel();
        }
        return true;
    }

    @Override
    public boolean onJsPrompt(
        WebView view,
        String url,
        String message,
        String defaultValue,
        JsPromptResult result
    ) {
        if (allowJsDialogs) {
            return super.onJsPrompt(view, url, message, defaultValue, result);
        }

        Log.i(TAG, "Suppressed JavaScript prompt in strict profile: " + url);
        if (result != null) {
            result.cancel();
        }
        return true;
    }

    @Override
    public boolean onShowFileChooser(
        WebView view,
        ValueCallback<Uri[]> filePathCallback,
        FileChooserParams fileChooserParams
    ) {
        LingXiaWebView webView = webViewRef.get();
        if (webView == null) {
            if (filePathCallback != null) {
                filePathCallback.onReceiveValue(null);
            }
            return false;
        }
        return webView.openFileChooser(filePathCallback, fileChooserParams);
    }

    private static boolean isUsableNewWindowUrl(String rawUrl) {
        if (rawUrl == null) {
            return false;
        }
        String url = rawUrl.trim();
        return !url.isEmpty() && !"about:blank".equalsIgnoreCase(url);
    }

    private static void destroyProbe(WebView view) {
        if (view == null) {
            return;
        }
        Runnable destroy = new Runnable() {
            @Override
            public void run() {
                try {
                    view.stopLoading();
                    view.destroy();
                } catch (Throwable t) {
                    Log.w(TAG, "Failed to destroy new-window probe", t);
                }
            }
        };
        if (Looper.myLooper() == Looper.getMainLooper()) {
            destroy.run();
        } else {
            MAIN_HANDLER.post(destroy);
        }
    }

    private int getLogLevel(ConsoleMessage.MessageLevel level) {
        if (level == ConsoleMessage.MessageLevel.TIP) return 2;      // VERBOSE
        if (level == ConsoleMessage.MessageLevel.DEBUG) return 3;    // DEBUG
        if (level == ConsoleMessage.MessageLevel.LOG) return 4;      // INFO
        if (level == ConsoleMessage.MessageLevel.WARNING) return 5;  // WARN
        if (level == ConsoleMessage.MessageLevel.ERROR) return 6;    // ERROR
        return 4; // Default to INFO
    }
}
