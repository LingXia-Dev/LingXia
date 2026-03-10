package com.lingxia.webview;

import android.content.Context;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.util.Base64;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import java.util.Map;
import org.json.JSONObject;

/**
 * LingXiaWebView provides complete WebView functionality for the LingXia platform.
 * This class contains all WebView logic including callbacks, message handling, and native integration.
 */
public class LingXiaWebView extends WebView {
    private static final String TAG = "LingXiaWebView";
    private static final String MESSAGEPORT_BRIDGE_CLASS = "com.lingxia.webview.AndroidMessagePortBridge";

    // MessagePort bridge instance (API 23+ only), accessed via cached reflection
    private Object messagePortBridge;
    private java.lang.reflect.Method sendPortMethod;
    private java.lang.reflect.Method postMessageMethod;
    private java.lang.reflect.Method cleanupMethod;

    private String appId;
    private String currentPath;
    private long sessionId;
    private boolean pageLoaded = false;
    private CreateOptions createOptions = CreateOptions.strictDefault();

    public static class WebResourceResponseData {
        public final String mimeType;
        public final String encoding;
        public final int statusCode;
        public final String reasonPhrase;
        public final Map<String, String> responseHeaders;
        public final String filePath;
        public final int pipeFd;
        public final byte[] data;
        public final long contentLength;

        public WebResourceResponseData(
                String mimeType,
                String encoding,
                int statusCode,
                String reasonPhrase,
                Map<String, String> responseHeaders,
                String filePath,
                int pipeFd,
                byte[] data,
                long contentLength
        ) {
            this.mimeType = mimeType;
            this.encoding = encoding;
            this.statusCode = statusCode;
            this.reasonPhrase = reasonPhrase;
            this.responseHeaders = responseHeaders;
            this.filePath = filePath;
            this.pipeFd = pipeFd;
            this.data = data;
            this.contentLength = contentLength;
        }
    }

    public static class CreateOptions {
        public String profile;
        public boolean domStorageEnabled;
        public boolean databaseEnabled;

        static CreateOptions strictDefault() {
            CreateOptions options = new CreateOptions();
            options.profile = "strict_default";
            options.domStorageEnabled = false;
            options.databaseEnabled = false;
            return options;
        }

        static CreateOptions browserRelaxed() {
            CreateOptions options = new CreateOptions();
            options.profile = "browser_relaxed";
            options.domStorageEnabled = true;
            options.databaseEnabled = true;
            return options;
        }

        static CreateOptions fromProfile(String profile) {
            if ("browser_relaxed".equals(profile)) {
                return browserRelaxed();
            }
            return strictDefault();
        }

        static CreateOptions fromToken(String optionsToken) {
            if (optionsToken == null || optionsToken.isEmpty()) {
                return strictDefault();
            }

            try {
                byte[] decoded = Base64.decode(
                        optionsToken,
                        Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING
                );
                String json = new String(decoded, java.nio.charset.StandardCharsets.UTF_8);
                JSONObject obj = new JSONObject(json);
                String profile = obj.optString("profile", "strict_default");
                if (!"strict_default".equals(profile) && !"browser_relaxed".equals(profile)) {
                    Log.w(TAG, "Invalid profile in options token: " + profile + ", fallback to strict_default");
                    profile = "strict_default";
                }
                return fromProfile(profile);
            } catch (Throwable e) {
                Log.w(TAG, "Failed to decode create options token, fallback to strict default", e);
                return strictDefault();
            }
        }
    }

    public LingXiaWebView(Context context) {
        super(context);

        if (context == null) {
            throw new IllegalArgumentException("Context cannot be null");
        }

        this.appId = null;
        this.currentPath = null;
        this.sessionId = 0L;
        this.pageLoaded = false;
    }

    private static android.content.Context sApplicationContext;

    /**
     * Set the application context for WebView creation.
     * This must be called by the application before creating any WebViews.
     */
    public static void setApplicationContext(android.content.Context context) {
        sApplicationContext = context.getApplicationContext();
        Log.d(TAG, "Application context set for WebView creation");
    }

    /**
     * Request WebView creation for Rust layer
     * Creates WebView asynchronously and notifies Rust via notifyWebViewReady callback
     */
    public static void requestWebView(final String appId, final String path, final long sessionId, final String optionsToken) {
        // WebView creation must happen on the main thread
        ensureMainThreadStatic(new Runnable() {
            @Override
            public void run() {
                try {
                    if (sApplicationContext == null) {
                        throw new RuntimeException("Application context not set. Call LingXiaWebView.setApplicationContext() first.");
                    }

                    LingXiaWebView webView;

                    // Try to create com.lingxia.lxapp.WebView (which extends LingXiaWebView)
                    // This allows the SDK to provide a customized WebView subclass
                    try {
                        Class<?> uiWebViewClass = Class.forName("com.lingxia.lxapp.WebView");
                        webView = (LingXiaWebView) uiWebViewClass
                            .getConstructor(android.content.Context.class)
                            .newInstance(sApplicationContext);
                    } catch (Throwable e) {
                        // Fallback to base LingXiaWebView if SDK class not available
                        webView = new LingXiaWebView(sApplicationContext);
                    }

                    webView.applyCreateOptionsToken(optionsToken);
                    webView.initializeWebView(appId, path, sessionId);

                    // Notify Rust that WebView is ready
                    notifyWebViewReady(appId, path, sessionId, webView);
                } catch (Throwable e) {
                    Log.e(TAG, "Failed to create WebView: " + e.getMessage(), e);
                }
            }
        });
    }

    private void ensureMainThread(Runnable action) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            action.run();
        } else {
            new Handler(Looper.getMainLooper()).post(action);
        }
    }

    /**
     * Static version of ensureMainThread for static methods
     */
    private static void ensureMainThreadStatic(Runnable action) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            action.run();
        } else {
            new Handler(Looper.getMainLooper()).post(action);
        }
    }

    public void initializeWebView(String appId, String path, long sessionId) {
        Log.d(TAG, "initializeWebView called, thread: " + Thread.currentThread().getName());
        this.appId = appId;
        this.currentPath = path;
        this.sessionId = sessionId;

        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                try {
                    initializeWebViewInternal();
                    Log.d(TAG, "WebView initialized successfully on main thread");
                } catch (Exception e) {
                    Log.e(TAG, "Failed to initialize WebView on main thread", e);
                }
            }
        });
    }

    private void applyCreateOptionsToken(String optionsToken) {
        this.createOptions = CreateOptions.fromToken(optionsToken);
        Log.i(
            TAG,
            "Apply create options: profile=" + this.createOptions.profile
                + ", domStorage=" + this.createOptions.domStorageEnabled
                + ", database=" + this.createOptions.databaseEnabled
        );
    }

    private void initializeWebViewInternal() {
        Log.d(TAG, "initializeWebViewInternal on thread: " + Thread.currentThread().getName());

        applyWebViewSettings();
        setupJavaScriptInterface();
        maybeInitMessagePortBridge();
        setupWebViewClients();
        Log.d(TAG, "LingXiaWebView initialized for appId=" + appId + ", path=" + currentPath);
    }

    private void maybeInitMessagePortBridge() {
        // Android 5 (API 21/22) does not have WebMessagePort, must not load those classes.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            messagePortBridge = null;
            return;
        }
        try {
            Class<?> bridgeClz = Class.forName(MESSAGEPORT_BRIDGE_CLASS);
            java.lang.reflect.Method create = bridgeClz.getMethod("create", LingXiaWebView.class);
            messagePortBridge = create.invoke(null, this);
            // Cache reflection methods for performance
            sendPortMethod = bridgeClz.getMethod("sendMessagePortToWebView");
            postMessageMethod = bridgeClz.getMethod("postMessageToWebView", String.class);
            cleanupMethod = bridgeClz.getMethod("cleanup");
            Log.d(TAG, "MessagePort bridge enabled (API=" + Build.VERSION.SDK_INT + ")");
        } catch (Throwable t) {
            messagePortBridge = null;
            sendPortMethod = null;
            postMessageMethod = null;
            cleanupMethod = null;
            Log.w(TAG, "MessagePort bridge unavailable, fallback to jsinterface", t);
        }
    }

    /**
     * Load URL ensuring main thread execution
     */
    public void loadUrl(final String url) {
        Log.d(TAG, "loadUrl called: " + url);
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                LingXiaWebView.super.loadUrl(url);
                Log.d(TAG, "URL loaded on main thread: " + url);
            }
        });
    }

    /**
     * Apply standard WebView settings (static version for external use)
     */
    @SuppressWarnings("deprecation")
    public static void applyWebViewSettings(WebSettings settings) {
        applyWebViewSettings(settings, CreateOptions.strictDefault());
    }

    @SuppressWarnings("deprecation")
    public static void applyWebViewSettings(WebSettings settings, CreateOptions options) {
        try {
            // Enable JavaScript
            settings.setJavaScriptEnabled(true);
            // Profile policy on Android:
            // - strict_default: disable JS popup windows
            // - browser_relaxed: enable JS popup windows
            settings.setJavaScriptCanOpenWindowsAutomatically("browser_relaxed".equals(options.profile));

            // Disable media
            settings.setMediaPlaybackRequiresUserGesture(true);

            // Layout and viewport
            settings.setUseWideViewPort(true);
            settings.setLoadWithOverviewMode(true);

            // Disable zoom
            settings.setSupportZoom(false);
            settings.setBuiltInZoomControls(false);

            // Encoding
            settings.setDefaultTextEncodingName("UTF-8");

            // Caching - minimal caching for security
            settings.setCacheMode(WebSettings.LOAD_NO_CACHE);

            settings.setDatabaseEnabled(options.databaseEnabled);
            settings.setDomStorageEnabled(options.domStorageEnabled);

            // File access is always disabled (not profile-configurable).
            settings.setAllowFileAccess(false);
            settings.setAllowFileAccessFromFileURLs(false);
            settings.setAllowUniversalAccessFromFileURLs(false);
            settings.setAllowContentAccess(false);

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                // HTTPS-only policy: mixed content is always blocked.
                settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
            }

        } catch (Exception e) {
            Log.e(TAG, "Error applying WebView settings", e);
            throw e;
        }
    }

    /**
     * Instance method wrapper for unified settings
     */
    private void applyWebViewSettings() {
        applyWebViewSettings(getSettings(), this.createOptions);
    }

    private void setupJavaScriptInterface() {
        addJavascriptInterface(new LingXiaProxy(), "LingXiaProxy");
    }

    /**
     * Send MessagePort to WebView for bidirectional communication.
     * Called from NativeBridge or LingXiaProxy. Must be called on main thread.
     */
    public void sendMessagePortToWebView() {
        if (messagePortBridge == null || sendPortMethod == null) return;
        try {
            sendPortMethod.invoke(messagePortBridge);
        } catch (Throwable t) {
            Log.w(TAG, "Failed to send message port", t);
        }
    }

    /**
     * Check if MessagePort is available (API 23+ and bridge initialized).
     */
    public boolean hasMessagePort() {
        return messagePortBridge != null && sendPortMethod != null;
    }

    private class LingXiaProxy {
        @android.webkit.JavascriptInterface
        public boolean supportsMessagePort() {
            return hasMessagePort();
        }

        @android.webkit.JavascriptInterface
        public String getPort(String portType) {
            if (!"LingXiaPort".equals(portType)) {
                return "Unknown port type";
            }
            if (!hasMessagePort()) {
                return "MessagePort unsupported";
            }
            ensureMainThread(LingXiaWebView.this::sendMessagePortToWebView);
            return "Message port sent";
        }

        @android.webkit.JavascriptInterface
        public void postMessage(String message) {
            // Android 5 compatible JS->native channel
            try {
                handlePostMessage(
                    getAppId() != null ? getAppId() : "",
                    getCurrentPath() != null ? getCurrentPath() : "",
                    getSessionId(),
                    message
                );
            } catch (Exception e) {
                Log.e(TAG, "Failed to handle JS message: " + e.getMessage(), e);
            }
        }
    }

    private void setupWebViewClients() {
        setWebChromeClient(new LingXiaWebChromeClient(this));
        setWebViewClient(new LingXiaWebViewClient(this));
        Log.d(TAG, "WebView clients setup completed");
    }

    public void postMessageToWebView(String message) {
        // WebView APIs and WebMessagePort are safest on the main thread.
        // Bridge calls come from Rust/JS worker threads, so always hop to the UI thread.
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
            // Prefer MessagePort on API 23+; fall back to evaluateJavascript for Android 5.
            if (messagePortBridge != null && postMessageMethod != null) {
                try {
                    Object ok = postMessageMethod.invoke(messagePortBridge, message);
                    if (ok instanceof Boolean && ((Boolean) ok)) {
                        return;
                    }
                } catch (Throwable t) {
                    Log.w(TAG, "MessagePort send failed, fallback to evaluateJavascript", t);
                }
            }
            try {
                final String quoted = JSONObject.quote(message);
                final String script = "(function(){var fn=window.__LingXiaRecvMessage; if(typeof fn==='function'){fn(" + quoted + ");}})();";
                evaluateJavascript(script, null);
            } catch (Exception e) {
                Log.e(TAG, "Failed to post message to WebView: " + e.getMessage(), e);
            }
            }
        });
    }

    public void evaluateJavascript(String script, android.webkit.ValueCallback<String> callback) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                LingXiaWebView.super.evaluateJavascript(script, callback);
            }
        });
    }

    /**
     * Load HTML data ensuring main thread execution
     */
    public void loadHtmlData(String data, String baseUrl, String historyUrl) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                resetViewport();
                loadDataWithBaseURL(baseUrl, data, "text/html", "UTF-8", historyUrl);
            }
        });
    }

    public void resetViewport() {
        try {
            evaluateJavascript(
                "(function(){" +
                "var head=document.head||document.getElementsByTagName('head')[0];" +
                "if(!head){return;}" +
                "var meta=document.querySelector('meta[name=viewport]');" +
                "if(!meta){meta=document.createElement('meta');meta.setAttribute('name','viewport');head.appendChild(meta);}" +
                "meta.setAttribute('content','width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no');" +
                "})();",
                null
            );
        } catch (Exception e) {
            Log.w(TAG, "Failed to reset viewport", e);
        }
    }

    @Override
    public void destroy() {
        Log.d(TAG, "Destroying WebView for appId=" + appId + ", path=" + currentPath);

        // Ensure all View operations happen on the main (UI) thread
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                try {
                    setVisibility(View.GONE);
                    stopLoading();
                    setWebViewClient(new WebViewClient());
                    setWebChromeClient(new WebChromeClient());
                    if (messagePortBridge != null && cleanupMethod != null) {
                        try {
                            cleanupMethod.invoke(messagePortBridge);
                        } catch (Throwable t) {
                            Log.w(TAG, "Failed to cleanup MessagePort bridge", t);
                        } finally {
                            messagePortBridge = null;
                            sendPortMethod = null;
                            postMessageMethod = null;
                            cleanupMethod = null;
                        }
                    }

                    try {
                        clearHistory();
                        clearCache(true);
                        clearFormData();
                    } catch (Exception e) {
                        Log.w(TAG, "Error clearing WebView data: " + e.getMessage());
                    }

                    try {
                        ViewGroup parent = (ViewGroup) getParent();
                        if (parent != null) {
                            parent.removeView(LingXiaWebView.this);
                        }
                    } catch (Exception e) {
                        Log.w(TAG, "Error removing WebView from parent: " + e.getMessage());
                    }

                    LingXiaWebView.super.destroy();
                    Log.d(TAG, "WebView destroyed successfully");
                } catch (Exception e) {
                    Log.e(TAG, "Critical error during WebView destruction", e);
                }
            }
        });
    }

    public String getAppId() {
        return appId;
    }

    public String getCurrentPath() {
        return currentPath;
    }

    public long getSessionId() {
        return sessionId;
    }

    public boolean isPageLoaded() {
        return pageLoaded;
    }

    public void setPageLoaded(boolean loaded) {
        this.pageLoaded = loaded;
    }

    native void onConsoleMessage(String appId, String path, long sessionId, int level, String message);
    native void onPageStarted(String appId, String path, long sessionId);
    native void onPageFinished(String appId, String path, long sessionId);
    native WebResourceResponseData handleRequest(String appId, String path, long sessionId, String url, String method, String[] headerKeysAndValues);
    native boolean handleNavigationPolicy(String appId, String path, long sessionId, String url);
    native int handlePostMessage(String appId, String path, long sessionId, String message);
    native static void notifyWebViewReady(String appId, String path, long sessionId, Object webView);
}
