package com.lingxia.webview;

import android.util.Log;
import android.webkit.ConsoleMessage;
import android.webkit.WebChromeClient;
import android.webkit.WebView;
import java.lang.ref.WeakReference;

/**
 * WebChromeClient implementation for LingXia WebView
 */
public class LingXiaWebChromeClient extends WebChromeClient {
    private static final String TAG = "LingXiaWebChromeClient";
    private final WeakReference<LingXiaWebView> webViewRef;

    public LingXiaWebChromeClient(LingXiaWebView webView) {
        this.webViewRef = new WeakReference<>(webView);
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

    private int getLogLevel(ConsoleMessage.MessageLevel level) {
        if (level == ConsoleMessage.MessageLevel.TIP) return 2;      // VERBOSE
        if (level == ConsoleMessage.MessageLevel.DEBUG) return 3;    // DEBUG
        if (level == ConsoleMessage.MessageLevel.LOG) return 4;      // INFO
        if (level == ConsoleMessage.MessageLevel.WARNING) return 5;  // WARN
        if (level == ConsoleMessage.MessageLevel.ERROR) return 6;    // ERROR
        return 4; // Default to INFO
    }
}
