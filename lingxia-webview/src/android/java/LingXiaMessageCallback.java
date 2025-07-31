package com.lingxia.webview;

import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import java.lang.ref.WeakReference;

/**
 * WebMessagePort callback implementation for LingXia WebView
 */
public class LingXiaMessageCallback extends WebMessagePort.WebMessageCallback {
    private final WeakReference<LingXiaWebView> webViewRef;

    public LingXiaMessageCallback(LingXiaWebView webView) {
        this.webViewRef = new WeakReference<>(webView);
    }

    @Override
    public void onMessage(WebMessagePort port, WebMessage message) {
        String messageData = message.getData();
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.handlePostMessage(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                messageData
            );
        }
    }
}
