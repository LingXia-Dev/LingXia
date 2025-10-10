package com.lingxia.webview;

import android.annotation.SuppressLint;
import android.content.Context;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import android.webkit.ValueCallback;
import java.util.Map;

/**
 * LingXiaWebView provides complete WebView functionality for the LingXia platform.
 * This class contains all WebView logic including callbacks, message handling, and native integration.
 */
public class LingXiaWebView extends WebView {
    private static final String TAG = "LingXiaWebView";
    private static final String ANDROID_MESSAGE_PORT_INIT = "LingXia-port-init";

    private String appId;
    private String currentPath;
    private boolean pageLoaded = false;
    private WebMessagePort nativePort;
    private WebMessagePort webviewPort;
    private boolean portsInitialized = false;

    // Scroll event tracking
    private int lastScrollX = 0;
    private int lastScrollY = 0;
    private long scrollEventThrottleMs = 100;  // Throttle scroll events to avoid excessive calls
    private long lastScrollEventTime = 0;

    public static class WebResourceResponseData {
        public final String mimeType;
        public final String encoding;
        public final int statusCode;
        public final String reasonPhrase;
        public final Map<String, String> responseHeaders;
        public final String filePath;
        public final long contentLength;

        public WebResourceResponseData(String mimeType, String encoding, int statusCode,
                                       String reasonPhrase, Map<String, String> responseHeaders,
                                       String filePath, long contentLength) {
            this.mimeType = mimeType;
            this.encoding = encoding;
            this.statusCode = statusCode;
            this.reasonPhrase = reasonPhrase;
            this.responseHeaders = responseHeaders;
            this.filePath = filePath;
            this.contentLength = contentLength;
        }
    }

    public LingXiaWebView(Context context) {
        super(context);

        if (context == null) {
            throw new IllegalArgumentException("Context cannot be null");
        }

        this.appId = null;
        this.currentPath = null;
        this.pageLoaded = false;
        this.portsInitialized = false;
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
    public static void requestWebView(final String appId, final String path) {

        // Start async WebView creation on main thread
        ensureMainThreadStatic(new Runnable() {
            @Override
            public void run() {
                Log.d(TAG, "Creating WebView on main thread for " + appId + ":" + path);

                try {
                    if (sApplicationContext == null) {
                        throw new RuntimeException("Application context not set. Call LingXiaWebView.setApplicationContext() first.");
                    }

                    LingXiaWebView webView;

                    try {
                        // Try to create UI WebView first
                        Class<?> uiWebViewClass = Class.forName("com.lingxia.lxapp.WebView");
                        Object uiWebView = uiWebViewClass.getConstructor(android.content.Context.class).newInstance(sApplicationContext);
                        uiWebViewClass.getMethod("initializeWebView", String.class, String.class).invoke(uiWebView, appId, path);

                        // If UI WebView is created successfully, we need to get the underlying LingXiaWebView
                        // For now, assume it's a LingXiaWebView or has one
                        if (uiWebView instanceof LingXiaWebView) {
                            webView = (LingXiaWebView) uiWebView;
                        } else {
                            // Fallback to direct creation
                            webView = new LingXiaWebView(sApplicationContext);
                            webView.initializeWebView(appId, path);
                        }
                    } catch (Exception e) {
                        Log.w(TAG, "Failed to create UI WebView, using fallback: " + e.getMessage());
                        webView = new LingXiaWebView(sApplicationContext);
                        webView.initializeWebView(appId, path);
                    }

                    // Notify Rust that WebView is ready, passing the WebView object directly
                    notifyWebViewReady(appId, path, webView);
                } catch (Exception e) {
                    Log.e(TAG, "Failed to create WebView asynchronously", e);
                }
            }
        });
    }

    /**
     * Setup LingXia functionality on a standard Android WebView
     */
    private static void setupLingXiaWebView(android.webkit.WebView webView, String appId, String path) {
        applyWebViewSettings(webView.getSettings());

        webView.setTag(appId.hashCode(), appId);
        webView.setTag(path.hashCode(), path);
        setupWebViewClients(webView, appId, path);
        setupJavaScriptInterface(webView);
        setupMessagePorts(webView, appId, path);

        Log.d(TAG, "LingXia functionality setup completed for " + appId + ":" + path);
    }

    /**
     * Setup JavaScript interface on a standard WebView
     */
    private static void setupJavaScriptInterface(android.webkit.WebView webView) {
        webView.addJavascriptInterface(createStaticLingXiaProxy(), "LingXiaProxy");
    }

    /**
     * Setup WebView clients on a standard WebView
     */
    private static void setupWebViewClients(android.webkit.WebView webView, String appId, String path) {

        webView.setWebChromeClient(createStaticWebChromeClient());
        webView.setWebViewClient(createStaticWebViewClient());
    }

    /**
     * Setup message ports on a standard WebView
     */
    private static void setupMessagePorts(android.webkit.WebView webView, String appId, String path) {

        Log.d(TAG, "Message ports setup for " + appId + ":" + path);
    }

    /**
     * Create JavaScript interface proxy for static WebView
     */
    private static Object createStaticLingXiaProxy() {
        return new StaticLingXiaProxy();
    }

    /**
     * Create WebChromeClient for standard WebView
     */
    private static WebChromeClient createStaticWebChromeClient() {
        return new LingXiaWebChromeClient(null);
    }

    /**
     * Create WebViewClient for standard WebView
     */
    private static WebViewClient createStaticWebViewClient() {
        return new LingXiaWebViewClient(null);
    }

    /**
     * Static inner class for JavaScript interface
     */
    private static class StaticLingXiaProxy {
        @android.webkit.JavascriptInterface
        public void postMessage(String message) {
            Log.d(TAG, "JavaScript message received: " + message);
        }
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

    public void initializeWebView(String appId, String path) {
        Log.d(TAG, "initializeWebView called, thread: " + Thread.currentThread().getName());
        this.appId = appId;
        this.currentPath = path;

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

    private void initializeWebViewInternal() {
        Log.d(TAG, "initializeWebViewInternal on thread: " + Thread.currentThread().getName());

        applyWebViewSettings();
        setupJavaScriptInterface();
        setupMessagePorts();
        setupWebViewClients();
        Log.d(TAG, "LingXiaWebView initialized for appId=" + appId + ", path=" + currentPath);
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
     * Evaluate JavaScript - fire and forget, no waiting, pure async
     */
    public void evaluateJavaScript(final String script) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                evaluateJavascript(script, null);
            }
        });
    }

    /**
     * Unified WebView settings configuration - SINGLE SOURCE OF TRUTH
     * This method contains all WebView settings and should be the only place
     * where WebView settings are configured.
     */
    private static void applyWebViewSettings(WebSettings settings) {
        if (settings == null) {
            return;
        }

        try {
            // Enable JavaScript, but disable other potentially insecure features
            settings.setJavaScriptEnabled(true);
            
            // Disable DOM Storage API
            settings.setDomStorageEnabled(false);
            
            // Disable all file access by default for security
            settings.setAllowFileAccess(false);
            settings.setAllowFileAccessFromFileURLs(false);
            settings.setAllowUniversalAccessFromFileURLs(false);
            settings.setAllowContentAccess(false);

        } catch (Exception e) {
            Log.e(TAG, "Error applying WebView settings", e);
            throw e;
        }
    }

    /**
     * Instance method wrapper for unified settings
     */
    private void applyWebViewSettings() {
        applyWebViewSettings(getSettings());
    }

    private void setupJavaScriptInterface() {
        addJavascriptInterface(createLingXiaProxy(), "LingXiaProxy");
    }

    private Object createLingXiaProxy() {
        return new LingXiaProxy();
    }

    private class LingXiaProxy {
        @android.webkit.JavascriptInterface
        public String getPort(String portType) {
            if ("LingXiaPort".equals(portType)) {

                if (Looper.myLooper() == Looper.getMainLooper()) {
                    sendMessagePortToWebView();
                } else {
                    new Handler(Looper.getMainLooper()).post(new Runnable() {
                        @Override
                        public void run() {
                            sendMessagePortToWebView();
                        }
                    });
                }
                return "Message port sent";
            }
            return "Unknown port type";
        }
    }

    private void setupWebViewClients() {
        setWebChromeClient(createWebChromeClient());
        setWebViewClient(createWebViewClient());
        Log.d(TAG, "WebView clients setup completed");
    }

    private WebChromeClient createWebChromeClient() {
        return new LingXiaWebChromeClient(this);
    }

    private WebViewClient createWebViewClient() {
        return new LingXiaWebViewClient(this);
    }

    private WebMessagePort.WebMessageCallback createMessageCallback() {
        return new LingXiaMessageCallback(this);
    }

    private void setupMessagePorts() {
        if (portsInitialized) {
            Log.d(TAG, "Message ports already initialized, skipping setup");
            return;
        }

        cleanupPorts();

        try {
            WebMessagePort[] messagePorts = createWebMessageChannel();
            nativePort = messagePorts[0];
            webviewPort = messagePorts[1];

            nativePort.setWebMessageCallback(createMessageCallback(), new Handler(Looper.getMainLooper()));

            portsInitialized = true;
            Log.d(TAG, "Message ports setup completed");

        } catch (Exception e) {
            Log.e(TAG, "Failed to setup message ports: " + e.getMessage(), e);
            cleanupPorts();
        }
    }

    private void cleanupPorts() {
        nativePort = null;
        webviewPort = null;
        portsInitialized = false;
    }

    public void sendMessagePortToWebView() {
        if (portsInitialized && webviewPort != null) {
            try {
                WebMessagePort[] ports = new WebMessagePort[1];
                ports[0] = webviewPort;
                postWebMessage(new WebMessage(ANDROID_MESSAGE_PORT_INIT, ports), Uri.EMPTY);
            } catch (Exception e) {
                Log.e(TAG, "Failed to send message port: " + e.getMessage());
            }
        }
    }

    public void postMessageToWebView(String message) {
        if (nativePort != null) {
            nativePort.postMessage(new WebMessage(message));
        }
    }

    public void evaluateJavascript(String script, android.webkit.ValueCallback<String> callback) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                LingXiaWebView.super.evaluateJavascript(script, callback);
            }
        });
    }

    public void loadData(String data, String mimeType, String encoding) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                LingXiaWebView.super.loadData(data, mimeType, encoding);
            }
        });
    }

    public void loadDataWithBaseURL(String baseUrl, String data, String mimeType, String encoding, String historyUrl) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                LingXiaWebView.super.loadDataWithBaseURL(baseUrl, data, mimeType, encoding, historyUrl);
            }
        });
    }

    public void resetViewport() {
        WebSettings settings = getSettings();
        if (settings != null) {
            settings.setUseWideViewPort(true);
            settings.setLoadWithOverviewMode(true);
            settings.setSupportZoom(true);
            settings.setBuiltInZoomControls(true);
            settings.setDisplayZoomControls(false);
        }
    }

    public void clearBrowsingData() {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                Log.d(TAG, "Clearing browsing data");
                clearHistory();
                clearCache(true);
                clearFormData();
            }
        });
    }

    public void setUserAgent(String userAgent) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                getSettings().setUserAgentString(userAgent);
            }
        });
    }

    public void loadHtmlData(String data, String baseUrl, String historyUrl) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                resetViewport();
                loadDataWithBaseURL(baseUrl, data, "text/html", "UTF-8", historyUrl);
            }
        });
    }

    @Override
    public void destroy() {
        Log.d(TAG, "Destroying WebView for appId=" + appId + ", path=" + currentPath);

        try {
            setVisibility(View.GONE);
            stopLoading();
            setWebViewClient(new WebViewClient());
            setWebChromeClient(new WebChromeClient());
            cleanupPorts();

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
                    parent.removeView(this);
                }
            } catch (Exception e) {
                Log.w(TAG, "Error removing WebView from parent: " + e.getMessage());
            }

            super.destroy();
            Log.d(TAG, "WebView destroyed successfully");
        } catch (Exception e) {
            Log.e(TAG, "Critical error during WebView destruction", e);
            throw e;
        }
    }

    /**
     * Enable or disable scroll event listener with optional throttle time.
     * When enabled, scroll events will be sent to the native layer via onScrollChanged.
     *
     * @param enabled Whether to enable scroll event listening
     * @param throttleMs Throttle time in milliseconds (minimum 16ms for 60fps), defaults to 100ms
     */
    public void setScrollListenerEnabled(boolean enabled, long throttleMs) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                // Set throttle time first (with validation)
                scrollEventThrottleMs = Math.max(16, throttleMs);

                if (enabled) {
                    // Set up scroll listener when enabling
                    setOnScrollChangeListener(new OnScrollChangeListener() {
                        @Override
                        public void onScrollChange(View view, int scrollX, int scrollY, int oldScrollX, int oldScrollY) {
                            handleScrollChange(scrollX, scrollY, oldScrollX, oldScrollY);
                        }
                    });
                    Log.d(TAG, "Scroll listener enabled with " + scrollEventThrottleMs + "ms throttle");
                } else {
                    // Clear scroll listener when disabling
                    setOnScrollChangeListener(null);
                    Log.d(TAG, "Scroll listener disabled");
                }
            }
        });
    }

    /**
     * Handle scroll change events with throttling
     */
    private void handleScrollChange(int scrollX, int scrollY, int oldScrollX, int oldScrollY) {
        // Throttle scroll events to avoid excessive native calls
        long currentTime = System.currentTimeMillis();
        if (currentTime - lastScrollEventTime < scrollEventThrottleMs) {
            return;
        }
        lastScrollEventTime = currentTime;

        // Only send scroll events if WebView is properly initialized and visible
        if (appId != null && currentPath != null && pageLoaded && getVisibility() == View.VISIBLE) {
            // Calculate scroll range
            int maxScrollX = computeHorizontalScrollRange() - getWidth();
            int maxScrollY = computeVerticalScrollRange() - getHeight();

            // Send scroll event to native layer
            onScrollChanged(
                appId,
                currentPath,
                scrollX,
                scrollY,
                maxScrollX,
                maxScrollY
            );
        }
    }

    public String getAppId() {
        return appId;
    }

    public String getCurrentPath() {
        return currentPath;
    }

    public boolean isPageLoaded() {
        return pageLoaded;
    }

    public void setPageLoaded(boolean loaded) {
        this.pageLoaded = loaded;
    }

    native void onConsoleMessage(String appId, String path, int level, String message);
    native void onPageStarted(String appId, String path);
    native void onPageFinished(String appId, String path);
    native WebResourceResponseData handleRequest(String appId, String path, String url, String method, String[] headerKeysAndValues);
    native int handlePostMessage(String appId, String path, String message);
    native void onScrollChanged(String appId, String path, int scrollX, int scrollY, int maxScrollX, int maxScrollY);
    native static void notifyWebViewReady(String appId, String path, Object webView);
}
