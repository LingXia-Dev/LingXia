package com.lingxia.webview;

import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import androidx.annotation.RequiresApi;

@RequiresApi(api = Build.VERSION_CODES.M)
public final class AndroidMessagePortBridge {
    private static final String TAG = "LingXiaWebView";
    private static final String ANDROID_MESSAGE_PORT_INIT = "LingXia-port-init";

    private final LingXiaWebView webView;
    private final long loadToken;
    private final long documentGeneration;
    private WebMessagePort nativePort;
    private WebMessagePort webviewPort;
    private boolean transferred;

    private AndroidMessagePortBridge(
            LingXiaWebView webView,
            long loadToken,
            long documentGeneration
    ) {
        this.webView = webView;
        this.loadToken = loadToken;
        this.documentGeneration = documentGeneration;
    }

    public static AndroidMessagePortBridge create(
            LingXiaWebView webView,
            long loadToken,
            long documentGeneration
    ) {
        AndroidMessagePortBridge bridge =
                new AndroidMessagePortBridge(webView, loadToken, documentGeneration);
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
                        if (!webView.acceptsDocumentPort(loadToken, documentGeneration)) {
                            return;
                        }
                        webView.handlePostMessage(
                                webView.getAppId() != null ? webView.getAppId() : "",
                                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                                webView.getSessionId(),
                                webView.getNativeViewId(),
                                LingXiaWebView.MESSAGE_TRANSPORT_PORT,
                                loadToken,
                                documentGeneration,
                                webView.getDiagnosticUrl(),
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
        if (webviewPort == null || transferred) return;
        try {
            WebMessagePort[] ports = new WebMessagePort[1];
            ports[0] = webviewPort;
            webView.postWebMessage(new WebMessage(ANDROID_MESSAGE_PORT_INIT, ports), Uri.EMPTY);
            // A WebMessagePort can only be transferred once. Keep nativePort for
            // page traffic, but mark the web-facing port as consumed so the next
            // page init gets a fresh channel pair.
            webviewPort = null;
            transferred = true;
        } catch (Throwable t) {
            Log.e(TAG, "Failed to send message port", t);
        }
    }

    public boolean postMessageToWebView(String message) {
        if (nativePort == null
                || !webView.acceptsDocumentPort(loadToken, documentGeneration)) return false;
        try {
            nativePort.postMessage(new WebMessage(message));
            return true;
        } catch (Throwable t) {
            Log.e(TAG, "Failed to post message via MessagePort", t);
            return false;
        }
    }

    public void cleanup() {
        closePort(nativePort);
        closePort(webviewPort);
        nativePort = null;
        webviewPort = null;
        transferred = false;
    }

    private static void closePort(WebMessagePort port) {
        if (port == null) return;
        try {
            port.close();
        } catch (Throwable ignored) {
        }
    }
}
