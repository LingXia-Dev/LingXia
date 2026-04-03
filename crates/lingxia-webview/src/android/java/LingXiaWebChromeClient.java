package com.lingxia.webview;

import android.util.Log;
import android.webkit.ConsoleMessage;
import android.webkit.JsPromptResult;
import android.webkit.JsResult;
import android.webkit.WebChromeClient;
import android.webkit.WebView;
import java.lang.ref.WeakReference;

/**
 * WebChromeClient implementation for LingXia WebView
 */
public class LingXiaWebChromeClient extends WebChromeClient {
    private static final String TAG = "LingXiaWebChromeClient";
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

    private int getLogLevel(ConsoleMessage.MessageLevel level) {
        if (level == ConsoleMessage.MessageLevel.TIP) return 2;      // VERBOSE
        if (level == ConsoleMessage.MessageLevel.DEBUG) return 3;    // DEBUG
        if (level == ConsoleMessage.MessageLevel.LOG) return 4;      // INFO
        if (level == ConsoleMessage.MessageLevel.WARNING) return 5;  // WARN
        if (level == ConsoleMessage.MessageLevel.ERROR) return 6;    // ERROR
        return 4; // Default to INFO
    }
}
