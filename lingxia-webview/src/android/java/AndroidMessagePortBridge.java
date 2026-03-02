package com.lingxia.webview;

import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;

public final class AndroidMessagePortBridge {
    private static final String TAG = "LingXiaWebView";
    private static final String ANDROID_MESSAGE_PORT_INIT = "LingXia-port-init";

    private final LingXiaWebView webView;
    private WebMessagePort nativePort;
    private WebMessagePort webviewPort;

    private AndroidMessagePortBridge(LingXiaWebView webView) {
        this.webView = webView;
    }

    public static AndroidMessagePortBridge create(LingXiaWebView webView) {
        AndroidMessagePortBridge bridge = new AndroidMessagePortBridge(webView);
        bridge.setupMessagePorts();
        return bridge;
    }

    private void setupMessagePorts() {
        cleanup();

        try {
            WebMessagePort[] ports = webView.createWebMessageChannel();
            nativePort = ports[0];
            webviewPort = ports[1];

            nativePort.setWebMessageCallback(new WebMessagePort.WebMessageCallback() {
                @Override
                public void onMessage(WebMessagePort port, WebMessage message) {
                    String messageData = message != null ? message.getData() : null;
                    try {
                        webView.handlePostMessage(
                                webView.getAppId() != null ? webView.getAppId() : "",
                                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                                webView.getSessionId(),
                                messageData != null ? messageData : ""
                        );
                    } catch (Throwable t) {
                        Log.e(TAG, "Failed to handle MessagePort message", t);
                    }
                }
            }, new Handler(Looper.getMainLooper()));

            Log.d(TAG, "MessagePort bridge initialized");
        } catch (Throwable t) {
            cleanup();
            throw t;
        }
    }

    public void sendMessagePortToWebView() {
        if (webviewPort == null) return;
        try {
            WebMessagePort[] ports = new WebMessagePort[1];
            ports[0] = webviewPort;
            webView.postWebMessage(new WebMessage(ANDROID_MESSAGE_PORT_INIT, ports), Uri.EMPTY);
        } catch (Throwable t) {
            Log.e(TAG, "Failed to send message port", t);
        }
    }

    public boolean postMessageToWebView(String message) {
        if (nativePort == null) return false;
        try {
            nativePort.postMessage(new WebMessage(message));
            return true;
        } catch (Throwable t) {
            Log.e(TAG, "Failed to post message via MessagePort", t);
            return false;
        }
    }

    public void cleanup() {
        nativePort = null;
        webviewPort = null;
    }
}
