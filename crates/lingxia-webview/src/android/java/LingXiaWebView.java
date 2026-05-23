package com.lingxia.webview;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Rect;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.util.Base64;
import android.util.Log;
import android.view.PixelCopy;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebChromeClient;
import android.webkit.CookieManager;
import android.webkit.DownloadListener;
import android.webkit.ValueCallback;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import java.io.ByteArrayOutputStream;
import androidx.webkit.ProxyConfig;
import androidx.webkit.ProxyController;
import androidx.webkit.WebViewFeature;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/**
 * LingXiaWebView provides complete WebView functionality for the LingXia platform.
 * This class contains all WebView logic including callbacks, message handling, and native integration.
 */
public class LingXiaWebView extends WebView {
    private static final String TAG = "LingXiaWebView";
    private static final String MESSAGEPORT_BRIDGE_CLASS = "com.lingxia.webview.AndroidMessagePortBridge";
    // Older Chromium builds expose createWebMessageChannel but its native
    // implementation can SIGSEGV on first call.
    private static final int MIN_CHROMIUM_MAJOR_FOR_MESSAGE_PORT = 60;
    private static final long PROXY_MAIN_THREAD_HOP_TIMEOUT_MS = 5000L;
    private static final long PROXY_CALLBACK_TIMEOUT_MS = 5000L;
    private static final long PROXY_TOTAL_TIMEOUT_MS =
            PROXY_MAIN_THREAD_HOP_TIMEOUT_MS + PROXY_CALLBACK_TIMEOUT_MS + 1000L;
    private static final AtomicLong sProxyRequestRevision = new AtomicLong(0L);
    private static final AtomicLong sFileChooserRequestSeq = new AtomicLong(0L);

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
    private static volatile boolean sHttpProxyEnabled = false;
    private final ConcurrentHashMap<Long, ValueCallback<android.net.Uri[]>> pendingFileChoosers =
            new ConcurrentHashMap<>();

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
        public boolean hasDownloadHandler;
        public boolean hasFileChooserHandler;

        static CreateOptions strictDefault() {
            CreateOptions options = new CreateOptions();
            options.profile = "strict_default";
            options.domStorageEnabled = false;
            options.databaseEnabled = false;
            options.hasDownloadHandler = false;
            options.hasFileChooserHandler = false;
            return options;
        }

        static CreateOptions browserRelaxed() {
            CreateOptions options = new CreateOptions();
            options.profile = "browser_relaxed";
            options.domStorageEnabled = true;
            options.databaseEnabled = true;
            options.hasDownloadHandler = false;
            options.hasFileChooserHandler = false;
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
                CreateOptions options = fromProfile(profile);
                options.hasDownloadHandler = obj.optBoolean("has_download_handler", false);
                options.hasFileChooserHandler = obj.optBoolean("has_file_chooser_handler", false);
                return options;
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
        Log.i(TAG, "Application context set for WebView creation");
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

    private boolean isBrowserProfile() {
        return createOptions != null && "browser_relaxed".equals(createOptions.profile);
    }

    private boolean hasDownloadHandler() {
        return createOptions != null && createOptions.hasDownloadHandler;
    }

    public boolean shouldSkipRustIntercept(String url) {
        if (!isBrowserProfile() || !sHttpProxyEnabled || url == null) {
            return false;
        }
        String lower = url.toLowerCase(Locale.ROOT);
        return lower.startsWith("http://") || lower.startsWith("https://");
    }

    public static String applyHttpProxy(final String host, final int port, final String[] bypassRules) {
        final AtomicReference<String> result = new AtomicReference<>(null);
        final CountDownLatch done = new CountDownLatch(1);
        final long requestRevision = sProxyRequestRevision.incrementAndGet();

        ensureMainThreadStatic(new Runnable() {
            @Override
            public void run() {
                try {
                    result.set(applyHttpProxyOnMain(host, port, bypassRules, requestRevision));
                } catch (Throwable t) {
                    sHttpProxyEnabled = false;
                    result.set("ERROR:" + t.getClass().getSimpleName() + ": " + t.getMessage());
                } finally {
                    done.countDown();
                }
            }
        });

        try {
            if (!done.await(PROXY_TOTAL_TIMEOUT_MS, TimeUnit.MILLISECONDS)) {
                sProxyRequestRevision.incrementAndGet();
                sHttpProxyEnabled = false;
                return "ERROR:timeout waiting main-thread proxy apply";
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            sProxyRequestRevision.incrementAndGet();
            sHttpProxyEnabled = false;
            return "ERROR:interrupted while waiting proxy apply";
        }

        return result.get();
    }

    private static String applyHttpProxyOnMain(String host, int port, String[] bypassRules, long requestRevision) {
        if (requestRevision != sProxyRequestRevision.get()) {
            return "ERROR:stale proxy request dropped";
        }

        final boolean enable = host != null && !host.trim().isEmpty() && port > 0;

        // API 21/22 builds are supported, but proxy override is unavailable there.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            sHttpProxyEnabled = false;
            return "UNSUPPORTED:android proxy override requires API 23+";
        }

        if (!WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)) {
            sHttpProxyEnabled = false;
            return "UNSUPPORTED:androidx.webkit PROXY_OVERRIDE not available";
        }

        CountDownLatch completion = new CountDownLatch(1);
        Runnable listener = completion::countDown;
        Executor directExecutor = Runnable::run;

        try {
            if (enable) {
                ProxyConfig.Builder builder = new ProxyConfig.Builder()
                        .addProxyRule("http://" + host.trim() + ":" + port);
                if (bypassRules != null) {
                    for (String rawRule : bypassRules) {
                        if (rawRule != null && !rawRule.trim().isEmpty()) {
                            builder.addBypassRule(rawRule.trim());
                        }
                    }
                }
                ProxyController.getInstance().setProxyOverride(builder.build(), directExecutor, listener);
            } else {
                ProxyController.getInstance().clearProxyOverride(directExecutor, listener);
            }

            if (!completion.await(PROXY_CALLBACK_TIMEOUT_MS, TimeUnit.MILLISECONDS)) {
                sHttpProxyEnabled = false;
                return "ERROR:timeout waiting androidx proxy callback";
            }

            if (requestRevision != sProxyRequestRevision.get()) {
                return "ERROR:stale proxy request after callback";
            }

            sHttpProxyEnabled = enable;
            return null;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            sHttpProxyEnabled = false;
            return "ERROR:interrupted while waiting androidx proxy callback";
        } catch (Throwable t) {
            sHttpProxyEnabled = false;
            return "ERROR:" + t.getClass().getSimpleName() + ": " + t.getMessage();
        }
    }

    public void initializeWebView(String appId, String path, long sessionId) {
        Log.i(TAG, "initializeWebView called, thread: " + Thread.currentThread().getName());
        this.appId = appId;
        this.currentPath = path;
        this.sessionId = sessionId;

        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                try {
                    initializeWebViewInternal();
                    Log.i(TAG, "WebView initialized successfully on main thread");
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
                + ", hasDownloadHandler=" + this.createOptions.hasDownloadHandler
                + ", hasFileChooserHandler=" + this.createOptions.hasFileChooserHandler
        );
    }

    private void initializeWebViewInternal() {
        Log.i(TAG, "initializeWebViewInternal on thread: " + Thread.currentThread().getName());

        // On non-standard Android builds (e.g. certain MStar bennet devices),
        // WebView internals may be fragile. Guard each init step so one
        // failure does not prevent the rest from running.
        try {
            applyWebViewSettings();
        } catch (Throwable t) {
            Log.w(TAG, "applyWebViewSettings failed, continuing with defaults", t);
        }

        try {
            setupJavaScriptInterface();
        } catch (Throwable t) {
            Log.w(TAG, "setupJavaScriptInterface failed, JS bridge may be unavailable", t);
        }

        try {
            maybeInitMessagePortBridge();
        } catch (Throwable t) {
            Log.w(TAG, "maybeInitMessagePortBridge failed", t);
        }

        try {
            setupWebViewClients();
        } catch (Throwable t) {
            Log.w(TAG, "setupWebViewClients failed", t);
        }

        Log.i(TAG, "LingXiaWebView initialized for appId=" + appId + ", path=" + currentPath);
    }

    private void maybeInitMessagePortBridge() {
        // Android 5 (API 21/22) does not have WebMessagePort, must not load those classes.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            messagePortBridge = null;
            return;
        }
        // createWebMessageChannel can SIGSEGV on old Chromium builds — a native
        // crash Java try/catch cannot rescue. Probe capability before touching it.
        if (!isMessagePortSafe()) {
            messagePortBridge = null;
            sendPortMethod = null;
            postMessageMethod = null;
            cleanupMethod = null;
            Log.i(TAG, "MessagePort bridge disabled; fallback to evaluateJavascript");
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
            Log.i(TAG, "MessagePort bridge enabled (API=" + Build.VERSION.SDK_INT + ")");
        } catch (Throwable t) {
            messagePortBridge = null;
            sendPortMethod = null;
            postMessageMethod = null;
            cleanupMethod = null;
            Log.w(TAG, "MessagePort bridge unavailable, fallback to jsinterface", t);
        }
    }

    // Two gates: androidx feature flag, then a Chromium major-version floor
    // parsed from the UA (catches builds that expose the API but crash on call).
    private boolean isMessagePortSafe() {
        try {
            if (!WebViewFeature.isFeatureSupported(WebViewFeature.CREATE_WEB_MESSAGE_CHANNEL)) {
                return false;
            }
        } catch (Throwable t) {
            return false;
        }
        try {
            WebSettings s = getSettings();
            String ua = s != null ? s.getUserAgentString() : null;
            if (ua != null) {
                java.util.regex.Matcher m =
                        java.util.regex.Pattern.compile("Chrome/(\\d+)").matcher(ua);
                if (m.find()) {
                    int major = Integer.parseInt(m.group(1));
                    if (major < MIN_CHROMIUM_MAJOR_FOR_MESSAGE_PORT) {
                        Log.i(TAG, "MessagePort gated off: Chromium " + major
                                + " < " + MIN_CHROMIUM_MAJOR_FOR_MESSAGE_PORT);
                        return false;
                    }
                }
            }
        } catch (Throwable ignored) {
        }
        return true;
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
        WebSettings settings = getSettings();
        if (settings == null) {
            Log.w(TAG, "WebView.getSettings() returned null, skipping settings");
            return;
        }
        applyWebViewSettings(settings, this.createOptions);
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
        setWebChromeClient(new LingXiaWebChromeClient(this, isBrowserProfile()));
        setWebViewClient(new LingXiaWebViewClient(this));
        setupDownloadSupport();
        Log.i(TAG, "WebView clients setup completed");
    }

    private void setupDownloadSupport() {
        if (!isBrowserProfile()) {
            setDownloadListener(null);
            return;
        }
        setDownloadListener(new DownloadListener() {
            @Override
            public void onDownloadStart(
                    String url,
                    String userAgent,
                    String contentDisposition,
                    String mimeType,
                    long contentLength
            ) {
                if (url == null || url.trim().isEmpty()) {
                    Log.w(TAG, "Ignored download callback with empty URL");
                    return;
                }
                if (!hasDownloadHandler()) {
                    Log.i(TAG, "Browser download suppressed without handler, url=" + url);
                    return;
                }

                String cookie = null;
                try {
                    cookie = CookieManager.getInstance().getCookie(url);
                } catch (Throwable e) {
                    Log.w(TAG, "Failed to read cookie for download URL: " + url, e);
                }

                try {
                    onDownloadRequested(
                        getAppId() != null ? getAppId() : "",
                        getCurrentPath() != null ? getCurrentPath() : "",
                        getSessionId(),
                        url,
                        userAgent != null ? userAgent : "",
                        contentDisposition != null ? contentDisposition : "",
                        mimeType != null ? mimeType : "",
                        contentLength,
                        cookie != null ? cookie : ""
                    );
                } catch (Throwable t) {
                    Log.e(TAG, "Failed to dispatch onDownloadRequested", t);
                }
            }
        });
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

    public void evaluateJavascriptWithResult(String script, long requestId) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                try {
                    LingXiaWebView.super.evaluateJavascript(
                        script,
                        new ValueCallback<String>() {
                            @Override
                            public void onReceiveValue(String value) {
                                try {
                                    onEvaluateJavascriptResult(
                                        requestId,
                                        value != null ? value : "null",
                                        ""
                                    );
                                } catch (Throwable callbackError) {
                                    Log.e(TAG, "Failed to forward evaluateJavascript result", callbackError);
                                }
                            }
                        }
                    );
                } catch (Throwable error) {
                    try {
                        onEvaluateJavascriptResult(
                            requestId,
                            "null",
                            error.getMessage() != null ? error.getMessage() : error.toString()
                        );
                    } catch (Throwable callbackError) {
                        Log.e(TAG, "Failed to forward evaluateJavascript error", callbackError);
                    }
                }
            }
        });
    }

    public boolean openFileChooser(
        final ValueCallback<android.net.Uri[]> filePathCallback,
        final WebChromeClient.FileChooserParams fileChooserParams
    ) {
        if (filePathCallback == null) {
            return false;
        }
        if (createOptions == null || !createOptions.hasFileChooserHandler) {
            filePathCallback.onReceiveValue(null);
            Log.w(TAG, "openFileChooser ignored: no registered file chooser handler");
            return false;
        }

        final long requestId = sFileChooserRequestSeq.incrementAndGet();
        pendingFileChoosers.put(requestId, filePathCallback);
        final String[] acceptTypes = fileChooserParams != null ? fileChooserParams.getAcceptTypes() : new String[0];
        final boolean allowMultiple = fileChooserParams != null
                && fileChooserParams.getMode() == WebChromeClient.FileChooserParams.MODE_OPEN_MULTIPLE;
        final boolean capture = fileChooserParams != null && fileChooserParams.isCaptureEnabled();
        final String sourceUrl = getUrl() != null ? getUrl() : "";

        try {
            onFileChooserRequested(
                getAppId() != null ? getAppId() : "",
                getCurrentPath() != null ? getCurrentPath() : "",
                getSessionId(),
                requestId,
                sourceUrl,
                acceptTypes != null ? acceptTypes : new String[0],
                allowMultiple,
                false,
                capture
            );
        } catch (Throwable error) {
            Log.e(TAG, "Failed to dispatch onFileChooserRequested", error);
            ValueCallback<android.net.Uri[]> callback = pendingFileChoosers.remove(requestId);
            if (callback != null) {
                callback.onReceiveValue(null);
            }
            return false;
        }
        return true;
    }

    public void completeFileChooserRequest(final long requestId, final String[] selectedPaths) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                ValueCallback<android.net.Uri[]> callback = pendingFileChoosers.remove(requestId);
                if (callback == null) {
                    return;
                }

                if (selectedPaths == null || selectedPaths.length == 0) {
                    callback.onReceiveValue(null);
                    return;
                }

                android.net.Uri[] uris = new android.net.Uri[selectedPaths.length];
                for (int i = 0; i < selectedPaths.length; i++) {
                    String raw = selectedPaths[i] != null ? selectedPaths[i].trim() : "";
                    uris[i] = raw.isEmpty() ? null : android.net.Uri.parse(raw);
                }
                callback.onReceiveValue(uris);
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

                    for (ValueCallback<android.net.Uri[]> callback : pendingFileChoosers.values()) {
                        if (callback != null) {
                            callback.onReceiveValue(null);
                        }
                    }
                    pendingFileChoosers.clear();
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
    native void onLoadError(String appId, String path, long sessionId, String url, int errorCode, String description);
    native WebResourceResponseData handleRequest(String appId, String path, long sessionId, String url, String method, String[] headerKeysAndValues);
    native boolean handleNavigationPolicy(String appId, String path, long sessionId, String url);
    native void onFileChooserRequested(
        String appId,
        String path,
        long sessionId,
        long requestId,
        String sourceUrl,
        String[] acceptTypes,
        boolean allowMultiple,
        boolean allowDirectories,
        boolean capture
    );
    native void onDownloadRequested(
        String appId,
        String path,
        long sessionId,
        String url,
        String userAgent,
        String contentDisposition,
        String mimeType,
        long contentLength,
        String cookie
    );
    native void onEvaluateJavascriptResult(long requestId, String value, String error);
    native void onScreenshotResult(long requestId, byte[] pngBytes, String error);

    /**
     * Capture the WebView's visible content into a PNG byte[].
     * Result is delivered asynchronously via {@link #onScreenshotResult}.
     * Implementation: PixelCopy from the containing window on API 26+, with
     * draw(Canvas) as a fallback, then compress to PNG.
     * Caller (Rust side) is expected to hold the requestId in a pending map.
     */
    public void captureScreenshot(final long requestId) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                try {
                    int width = getWidth();
                    int height = getHeight();
                    if (width <= 0 || height <= 0) {
                        onScreenshotResult(requestId, new byte[0],
                            "WebView has zero size; cannot capture");
                        return;
                    }

                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        Activity activity = findActivity(getContext());
                        if (activity != null && activity.getWindow() != null) {
                            Bitmap bitmap = Bitmap.createBitmap(
                                width, height, Bitmap.Config.ARGB_8888);
                            int[] location = new int[2];
                            getLocationInWindow(location);
                            Rect srcRect = new Rect(
                                location[0],
                                location[1],
                                location[0] + width,
                                location[1] + height
                            );
                            try {
                                PixelCopy.request(
                                    activity.getWindow(),
                                    srcRect,
                                    bitmap,
                                    new PixelCopy.OnPixelCopyFinishedListener() {
                                        @Override
                                        public void onPixelCopyFinished(int result) {
                                            if (result == PixelCopy.SUCCESS) {
                                                deliverScreenshotBitmap(requestId, bitmap);
                                            } else {
                                                bitmap.recycle();
                                                Log.w(TAG, "PixelCopy screenshot failed: result=" + result);
                                                captureScreenshotByDraw(requestId, width, height);
                                            }
                                        }
                                    },
                                    new Handler(Looper.getMainLooper())
                                );
                                return;
                            } catch (Throwable pixelCopyError) {
                                bitmap.recycle();
                                Log.w(TAG, "PixelCopy screenshot threw; falling back to draw(Canvas)",
                                    pixelCopyError);
                            }
                        }
                    }

                    captureScreenshotByDraw(requestId, width, height);
                } catch (Throwable error) {
                    try {
                        onScreenshotResult(requestId, new byte[0],
                            error.getMessage() != null ? error.getMessage() : error.toString());
                    } catch (Throwable callbackError) {
                        Log.e(TAG, "Failed to forward screenshot error", callbackError);
                    }
                }
            }
        });
    }

    private void captureScreenshotByDraw(final long requestId, int width, int height) {
        try {
            // Re-read current size: when PixelCopy is invoked and its
            // listener fires asynchronously, the WebView may have been
            // resized between request and callback. Trust the live
            // measurement over the stale `width`/`height` captured before
            // the PixelCopy attempt.
            int liveWidth = getWidth();
            int liveHeight = getHeight();
            if (liveWidth <= 0) liveWidth = width;
            if (liveHeight <= 0) liveHeight = height;
            Bitmap bitmap = Bitmap.createBitmap(liveWidth, liveHeight, Bitmap.Config.ARGB_8888);
            Canvas canvas = new Canvas(bitmap);
            draw(canvas);
            deliverScreenshotBitmap(requestId, bitmap);
        } catch (Throwable error) {
            try {
                onScreenshotResult(requestId, new byte[0],
                    error.getMessage() != null ? error.getMessage() : error.toString());
            } catch (Throwable callbackError) {
                Log.e(TAG, "Failed to forward screenshot fallback error", callbackError);
            }
        }
    }

    private void deliverScreenshotBitmap(final long requestId, Bitmap bitmap) {
        try {
            ByteArrayOutputStream stream = new ByteArrayOutputStream(
                Math.max(64 * 1024, bitmap.getWidth() * bitmap.getHeight() / 4));
            boolean ok = bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream);
            bitmap.recycle();
            if (!ok) {
                onScreenshotResult(requestId, new byte[0],
                    "Bitmap.compress(PNG) returned false");
                return;
            }
            onScreenshotResult(requestId, stream.toByteArray(), "");
        } catch (Throwable error) {
            try {
                if (!bitmap.isRecycled()) {
                    bitmap.recycle();
                }
                onScreenshotResult(requestId, new byte[0],
                    error.getMessage() != null ? error.getMessage() : error.toString());
            } catch (Throwable callbackError) {
                Log.e(TAG, "Failed to forward screenshot error", callbackError);
            }
        }
    }

    private static Activity findActivity(Context context) {
        Context current = context;
        while (current instanceof ContextWrapper) {
            if (current instanceof Activity) {
                return (Activity) current;
            }
            current = ((ContextWrapper) current).getBaseContext();
        }
        return null;
    }
    native int handlePostMessage(String appId, String path, long sessionId, String message);
    native static void notifyWebViewReady(String appId, String path, long sessionId, Object webView);
}
