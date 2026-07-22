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
import android.webkit.WebStorage;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import java.io.ByteArrayOutputStream;
import androidx.webkit.ProxyConfig;
import androidx.webkit.ProxyController;
import androidx.webkit.ProfileStore;
import androidx.webkit.WebViewCompat;
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
    private static final int NEW_WINDOW_POLICY_CANCEL = 0;
    private static final int NEW_WINDOW_POLICY_LOAD_IN_SELF = 1;
    private static final AtomicLong sProxyRequestRevision = new AtomicLong(0L);
    private static final AtomicLong sFileChooserRequestSeq = new AtomicLong(0L);
    private static final AtomicLong sEphemeralProfileSeq = new AtomicLong(0L);
    private static final String EPHEMERAL_PROFILE_PREFIX = "lingxia_ephemeral_";

    // MessagePort bridge instance (API 23+ only), accessed via cached reflection
    private Object messagePortBridge;
    private java.lang.reflect.Method sendPortMethod;
    private java.lang.reflect.Method postMessageMethod;
    private java.lang.reflect.Method cleanupMethod;

    private String appId;
    private String currentPath;
    private long sessionId;
    private boolean pageLoaded = false;
    private String defaultUserAgent;
    private CreateOptions createOptions = CreateOptions.strictDefault();
    private String ephemeralProfileName;
    private boolean usesGlobalEphemeralFallback;
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
        public String dataMode;
        public boolean domStorageEnabled;
        public boolean databaseEnabled;
        public boolean hasNewWindowHandler;
        public boolean hasDownloadHandler;
        public boolean hasFileChooserHandler;

        static CreateOptions strictDefault() {
            CreateOptions options = new CreateOptions();
            options.profile = "strict_default";
            options.dataMode = "profile_default";
            options.domStorageEnabled = false;
            options.databaseEnabled = false;
            options.hasNewWindowHandler = false;
            options.hasDownloadHandler = false;
            options.hasFileChooserHandler = false;
            return options;
        }

        static CreateOptions browserRelaxed() {
            CreateOptions options = new CreateOptions();
            options.profile = "browser_relaxed";
            options.dataMode = "profile_default";
            options.domStorageEnabled = true;
            options.databaseEnabled = true;
            options.hasNewWindowHandler = false;
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
                String dataMode = obj.optString("data_mode", "profile_default");
                if (!"profile_default".equals(dataMode) && !"ephemeral".equals(dataMode)) {
                    Log.w(TAG, "Invalid data mode in options token: " + dataMode + ", fallback to profile_default");
                    dataMode = "profile_default";
                }
                options.dataMode = dataMode;
                options.hasNewWindowHandler = obj.optBoolean("has_new_window_handler", false);
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
        WebSettings settings = getSettings();
        this.defaultUserAgent = settings != null ? settings.getUserAgentString() : null;
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

                    final LingXiaWebView createdWebView = webView;
                    createdWebView.applyCreateOptionsToken(optionsToken);
                    createdWebView.prepareDataMode(() -> {
                        createdWebView.initializeWebView(appId, path, sessionId);
                        notifyWebViewReady(appId, path, sessionId, createdWebView);
                    });
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

    public boolean supportsNewWindows() {
        return createOptions != null &&
                (isBrowserProfile() || createOptions.hasNewWindowHandler);
    }

    public boolean handleNewWindowRequest(String url) {
        if (url == null || url.trim().isEmpty() || !supportsNewWindows()) {
            return false;
        }
        String target = url.trim();
        try {
            int policy = handleNewWindowPolicy(
                    getAppId() != null ? getAppId() : "",
                    getCurrentPath() != null ? getCurrentPath() : "",
                    getSessionId(),
                    target
            );
            if (policy == NEW_WINDOW_POLICY_LOAD_IN_SELF) {
                loadUrl(target);
            }
            return true;
        } catch (Throwable t) {
            Log.e(TAG, "Failed to dispatch new-window request: " + target, t);
            return false;
        }
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
                + ", dataMode=" + this.createOptions.dataMode
                + ", domStorage=" + this.createOptions.domStorageEnabled
                + ", database=" + this.createOptions.databaseEnabled
                + ", hasNewWindowHandler=" + this.createOptions.hasNewWindowHandler
                + ", hasDownloadHandler=" + this.createOptions.hasDownloadHandler
                + ", hasFileChooserHandler=" + this.createOptions.hasFileChooserHandler
        );
    }

    private void prepareDataMode(Runnable continuation) {
        if (!"ephemeral".equals(this.createOptions.dataMode)) {
            continuation.run();
            return;
        }
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROFILE)) {
            // Old engines have only a process-global cookie/storage store.
            // Clear it before publishing the WebView so the first navigation
            // cannot race stale SSO cookies; clear again on teardown below.
            this.usesGlobalEphemeralFallback = true;
            CookieManager cookieManager = CookieManager.getInstance();
            cookieManager.removeAllCookies(value -> {
                cookieManager.flush();
                WebStorage.getInstance().deleteAllData();
                new Handler(Looper.getMainLooper()).post(continuation);
            });
            return;
        }

        String profileName = EPHEMERAL_PROFILE_PREFIX
                + android.os.Process.myPid()
                + "_"
                + sEphemeralProfileSeq.incrementAndGet();
        ProfileStore.getInstance().getOrCreateProfile(profileName);
        WebViewCompat.setProfile(this, profileName);
        this.ephemeralProfileName = profileName;
        continuation.run();
    }

    private static void deleteEphemeralProfile(String profileName, int attemptsRemaining) {
        try {
            if (ProfileStore.getInstance().deleteProfile(profileName)) {
                return;
            }
        } catch (Throwable error) {
            Log.w(TAG, "Failed to delete ephemeral WebView profile " + profileName, error);
        }
        if (attemptsRemaining > 0) {
            new Handler(Looper.getMainLooper()).postDelayed(
                    () -> deleteEphemeralProfile(profileName, attemptsRemaining - 1),
                    100L
            );
        } else {
            Log.w(TAG, "Ephemeral WebView profile remains in use: " + profileName);
        }
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

    /** Override the complete user agent, or restore the engine default. */
    public void setUserAgentOverride(final boolean useDefault, final String userAgent) {
        ensureMainThread(new Runnable() {
            @Override
            public void run() {
                WebSettings settings = getSettings();
                if (settings == null) {
                    Log.w(TAG, "WebView.getSettings() returned null, skipping user-agent override");
                    return;
                }
                settings.setUserAgentString(useDefault ? defaultUserAgent : userAgent);
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
            // - browser_relaxed or explicit new-window handler: enable JS popup windows
            boolean supportsNewWindows =
                    "browser_relaxed".equals(options.profile) || options.hasNewWindowHandler;
            settings.setJavaScriptCanOpenWindowsAutomatically(supportsNewWindows);
            settings.setSupportMultipleWindows(supportsNewWindows);

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

        // Routes the result of a Rust-issued `eval_js` back into native land.
        // The wrapper script awaits the user expression, builds a JSON
        // {ok,value|error} envelope, and calls this method with the requestId
        // and token it was given. The token prevents page JS from resolving
        // unrelated pending native eval requests by guessing monotonic ids.
        @android.webkit.JavascriptInterface
        public void resolveEval(String requestIdStr, String token, String resultJson) {
            try {
                long requestId = Long.parseLong(requestIdStr);
                onEvaluateJavascriptResult(requestId, token, resultJson, "");
            } catch (NumberFormatException e) {
                Log.w(TAG, "resolveEval invalid requestId: " + requestIdStr);
            } catch (Exception e) {
                Log.w(TAG, "resolveEval forwarding failed", e);
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

    /**
     * Native-dispatch a touch tap at (x, y) in WebView-local pixels. Emits an
     * ACTION_DOWN followed by ACTION_UP so Chromium's content layer treats it
     * as a real gesture (fires touchstart/touchend → click, focuses inputs,
     * pops the IME). Used by `WebViewInputController::click` on Android in
     * place of synthesized DOM events.
     */
    public void dispatchClickAt(final float x, final float y) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            dispatchClickAtOnMainThread(x, y);
            return;
        }

        final java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);
        final java.util.concurrent.atomic.AtomicReference<Throwable> error =
            new java.util.concurrent.atomic.AtomicReference<>();
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                try {
                    dispatchClickAtOnMainThread(x, y);
                } catch (Throwable t) {
                    error.set(t);
                } finally {
                    latch.countDown();
                }
            }
        });
        try {
            if (!latch.await(2, java.util.concurrent.TimeUnit.SECONDS)) {
                throw new RuntimeException("Timed out dispatching click on main thread");
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException("Interrupted while dispatching click on main thread", e);
        }
        Throwable t = error.get();
        if (t instanceof RuntimeException) {
            throw (RuntimeException) t;
        }
        if (t != null) {
            throw new RuntimeException("Failed to dispatch click on main thread", t);
        }
    }

    private void dispatchClickAtOnMainThread(final float x, final float y) {
        long downTime = android.os.SystemClock.uptimeMillis();
        android.view.MotionEvent down = android.view.MotionEvent.obtain(
            downTime, downTime, android.view.MotionEvent.ACTION_DOWN, x, y, 0);
        try {
            LingXiaWebView.super.dispatchTouchEvent(down);
        } finally {
            down.recycle();
        }
        android.view.MotionEvent up = android.view.MotionEvent.obtain(
            downTime, downTime + 50, android.view.MotionEvent.ACTION_UP, x, y, 0);
        try {
            LingXiaWebView.super.dispatchTouchEvent(up);
        } finally {
            up.recycle();
        }
    }

    /**
     * Scroll page content by a delta in device pixels via a synthesized touch
     * swipe. The lxapp shell renders pages into a WebView sized to content
     * height inside a native scroll parent, so neither DOM `scrollTop` nor
     * `WebView.scrollBy` moves it — only a real gesture routes through the
     * scroll pipeline. To scroll content by (dx, dy) the finger travels by
     * (-dx, -dy). Used by `WebViewInputController::scroll` on Android.
     */
    public void scrollByPixels(final int dx, final int dy) {
        final int w = getWidth();
        final int h = getHeight();
        if (w <= 0 || h <= 0) {
            return;
        }
        // Anchor at the center and keep the whole gesture on-screen: the finger
        // travels by (-dx, -dy), clamped so start and end stay within a safe
        // inset. Large deltas are capped to one gesture (callers repeat).
        final float insetX = w * 0.1f;
        final float insetY = h * 0.15f;
        final float cx = w / 2f;
        final float cy = h / 2f;
        float travelX = -dx;
        float travelY = -dy;
        final float maxX = (w / 2f) - insetX;
        final float maxY = (h / 2f) - insetY;
        if (travelX > maxX) travelX = maxX; else if (travelX < -maxX) travelX = -maxX;
        if (travelY > maxY) travelY = maxY; else if (travelY < -maxY) travelY = -maxY;
        final float startX = cx - travelX / 2f;
        final float startY = cy - travelY / 2f;
        final float endX = cx + travelX / 2f;
        final float endY = cy + travelY / 2f;

        // Dispatch the gesture spaced in REAL time. Firing all MotionEvents in a
        // tight synchronous loop makes Chromium's velocity tracker see the whole
        // travel in ~0ms → a huge fling that overshoots many times. So sleep
        // between events (on a worker; the JNI caller thread is not the main
        // thread) and post each event to the main thread.
        final Runnable swipe = new Runnable() {
            @Override
            public void run() {
                final long downTime = android.os.SystemClock.uptimeMillis();
                postMotion(android.view.MotionEvent.ACTION_DOWN, startX, startY, downTime);
                final int steps = 20;
                for (int i = 1; i <= steps; i++) {
                    sleepQuietly(16);
                    final float t = (float) i / steps;
                    postMotion(android.view.MotionEvent.ACTION_MOVE,
                        startX + (endX - startX) * t, startY + (endY - startY) * t, downTime);
                }
                // Sustained real-time hold at the end so the release velocity is
                // ~zero — this is what actually prevents the fling.
                for (int i = 0; i < 8; i++) {
                    sleepQuietly(20);
                    postMotion(android.view.MotionEvent.ACTION_MOVE, endX, endY, downTime);
                }
                sleepQuietly(16);
                postMotion(android.view.MotionEvent.ACTION_UP, endX, endY, downTime);
                // Flush the main queue so the gesture is fully delivered before
                // scrollByPixels returns (scroll_to loops and re-reads position).
                runOnMainSync(new Runnable() { @Override public void run() {} });
            }
        };

        if (Looper.myLooper() == Looper.getMainLooper()) {
            final Thread worker = new Thread(swipe, "lx-swipe");
            worker.start();
            try {
                worker.join(5000);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        } else {
            swipe.run();
        }
    }

    private void postMotion(final int action, final float x, final float y, final long downTime) {
        final long eventTime = android.os.SystemClock.uptimeMillis();
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                android.view.MotionEvent ev =
                    android.view.MotionEvent.obtain(downTime, eventTime, action, x, y, 0);
                try {
                    LingXiaWebView.super.dispatchTouchEvent(ev);
                } finally {
                    ev.recycle();
                }
            }
        });
    }

    private void runOnMainSync(final Runnable r) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            r.run();
            return;
        }
        final java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                try {
                    r.run();
                } finally {
                    latch.countDown();
                }
            }
        });
        try {
            latch.await(2, java.util.concurrent.TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private void sleepQuietly(long ms) {
        try {
            Thread.sleep(ms);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
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
                    String profileName = ephemeralProfileName;
                    ephemeralProfileName = null;
                    if (profileName != null) {
                        deleteEphemeralProfile(profileName, 5);
                    }
                    if (usesGlobalEphemeralFallback) {
                        usesGlobalEphemeralFallback = false;
                        CookieManager cookieManager = CookieManager.getInstance();
                        cookieManager.removeAllCookies(value -> cookieManager.flush());
                        WebStorage.getInstance().deleteAllData();
                    }
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

    /**
     * Sample this WebView's observable state (URL, title, back/forward
     * availability) and push it into the Rust delegate. Called from the
     * client callbacks so the platform adapter — not host UI — is the source
     * of truth for Rust-visible WebView state.
     */
    void pushWebViewState() {
        onWebViewStateChanged(
            getAppId() != null ? getAppId() : "",
            getCurrentPath() != null ? getCurrentPath() : "",
            getSessionId(),
            getUrl() != null ? getUrl() : "",
            getTitle() != null ? getTitle() : "",
            canGoBack(),
            canGoForward()
        );
    }

    /** Encode the received favicon as PNG and push it into the Rust delegate. */
    void pushFavicon(android.graphics.Bitmap icon) {
        if (icon == null) {
            return;
        }
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        if (icon.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, out)) {
            onFaviconChanged(
                getAppId() != null ? getAppId() : "",
                getCurrentPath() != null ? getCurrentPath() : "",
                getSessionId(),
                out.toByteArray()
            );
        }
    }

    native void onConsoleMessage(String appId, String path, long sessionId, int level, String message);
    native void onPageStarted(String appId, String path, long sessionId, String url);
    native void onPageFinished(String appId, String path, long sessionId, String url);
    native void onPageCommitted(String appId, String path, long sessionId);
    native void onWebViewStateChanged(String appId, String path, long sessionId, String url, String title, boolean canGoBack, boolean canGoForward);
    native void onFaviconChanged(String appId, String path, long sessionId, byte[] pngBytes);
    native void onLoadError(String appId, String path, long sessionId, String url, int errorCode, String description);
    native WebResourceResponseData handleRequest(String appId, String path, long sessionId, String url, String method, String[] headerKeysAndValues);
    native boolean handleNavigationPolicy(
        String appId,
        String path,
        long sessionId,
        String url,
        boolean hasUserGesture,
        boolean isMainFrame
    );
    native int handleNewWindowPolicy(String appId, String path, long sessionId, String url);
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
    native void onEvaluateJavascriptResult(long requestId, String token, String value, String error);
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
