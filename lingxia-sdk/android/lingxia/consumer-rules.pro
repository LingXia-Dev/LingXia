# Public host APIs that product apps call from Java/Kotlin. Host R8 must
# not strip them — the library itself is not minified.
-keep class com.lingxia.app.Lingxia { *; }
-keep class com.lingxia.app.media.** { *; }

-keepnames interface com.lingxia.app.media.modules.CameraModule
-keepnames interface com.lingxia.app.media.modules.ScannerModule

# The Rust runtime binds native methods by symbol (Java_<class>_<method>), so
# the declaring class and method names are part of the native ABI.
-keepclasseswithmembernames,includedescriptorclasses class com.lingxia.** {
    native <methods>;
}

# Classes the Rust runtime resolves by name to call their static methods.
-keep class com.lingxia.app.AppScreenshot { public static !synthetic <methods>; }
-keep class com.lingxia.app.UpdateManager { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.LxApp { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.NativeComponents.ComponentRouter { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppActionSheet { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppCapsule { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppClipboard { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppDevice { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppFile { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppLocation { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppMedia { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppModal { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppNetwork { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppNotification { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppPullToRefresh { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppShare { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppSurface { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppToast { public static !synthetic <methods>; }
-keep class com.lingxia.lxapp.APIs.LxAppWifi { public static !synthetic <methods>; }
# Resolved with the others at load; the runtime calls nothing on it.
-keep class com.lingxia.lxapp.APIs.LxAppPicker

# Objects the Rust runtime constructs and hands back across JNI.
-keep class com.lingxia.app.CurrentLxApp { <init>(...); }
-keep class com.lingxia.lxapp.LxAppInfo { <init>(...); }
-keep class com.lingxia.lxapp.APIs.media.PreviewMediaPayload { <init>(...); }
-keep class com.lingxia.lxapp.chrome.NavigationBarState { <init>(...); }
-keep class com.lingxia.lxapp.chrome.TabBarItem { <init>(...); }
-keep class com.lingxia.lxapp.chrome.TabBarState { <init>(...); }
-keep class com.lingxia.lxapp.chrome.TabBarState$Position {
    public static final com.lingxia.lxapp.chrome.TabBarState$Position BOTTOM;
}
-keep class com.lingxia.webview.LingXiaWebView$WebResourceResponseData { <init>(...); }

# Called back from the Rust runtime when a page's WebView resolves.
-keep interface com.lingxia.app.PageWebViewCallback {
    void onResult(com.lingxia.lxapp.WebView, int);
}

# WebView methods the Rust runtime calls on the instance it was handed.
-keep class com.lingxia.webview.LingXiaWebView {
    public static void requestWebView(...);
    public static java.lang.String applyHttpProxy(...);
    long allocateNavigationLoadToken();
    void captureScreenshot(long);
    void clearBrowsingData();
    void completeFileChooserRequest(long, java.lang.String[]);
    void dispatchClickAt(float, float);
    void loadHtmlData(java.lang.String, java.lang.String, java.lang.String);
    void loadTrustedHtmlData(long, java.lang.String, java.lang.String, java.lang.String);
    boolean postDocumentMessageNow(long, java.lang.String);
    void postMessageToWebView(java.lang.String);
    void scheduleDocumentMessage(long);
    void scrollByPixels(int, int);
    void setNativeViewId(long);
    void setUserAgentOverride(boolean, java.lang.String);
}

# Looked up reflectively by LingXiaWebView.
-keep class com.lingxia.lxapp.WebView { <init>(android.content.Context); }
-keepclassmembers class com.lingxia.lxapp.LxApp {
    public static java.lang.Boolean appearanceDarkFor(java.lang.String);
}
-keep class com.lingxia.webview.AndroidMessagePortBridge {
    public static com.lingxia.webview.AndroidMessagePortBridge create(...);
    public *** sendMessagePortToWebView();
    public *** renewWebViewPort();
    public *** postMessageToWebView(java.lang.String);
    public *** cleanup();
}

# Page scripts reach these by name through addJavascriptInterface.
-keepattributes RuntimeVisibleAnnotations
-keepclassmembers class com.lingxia.** {
    @android.webkit.JavascriptInterface <methods>;
}
