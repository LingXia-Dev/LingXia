package com.lingxia.lxapp

import android.content.res.AssetManager
import android.util.Log
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Centralized Native API declarations
 *
 * This object contains all JNI function declarations that interface with the Rust layer.
 * All native methods are organized by functionality and provide a single point of reference
 * for the FFI interface.
 */
internal object NativeApi {
    private const val TAG = "NativeApi"
    private val libraryLoaded = AtomicBoolean(false)
    private val loadAttempted = AtomicBoolean(false)

    @JvmStatic
    fun ensureLoaded(): Boolean {
        if (libraryLoaded.get()) return true
        if (loadAttempted.compareAndSet(false, true)) {
            try {
                System.loadLibrary("lingxia")
                libraryLoaded.set(true)
                Log.d(TAG, "Native library 'lingxia' loaded")
            } catch (error: Throwable) {
                Log.e(TAG, "Failed to load native library 'lingxia'", error)
            }
        }
        return libraryLoaded.get()
    }

    init {
        ensureLoaded()
    }

    // UI Event Type Constants
    const val UI_EVENT_TABBAR_CLICK = 0
    const val UI_EVENT_CAPSULE_CLICK = 1
    const val UI_EVENT_NAVIGATION_CLICK = 2
    const val UI_EVENT_BACK_PRESS = 3
    const val UI_EVENT_PULL_DOWN_REFRESH = 4

    // Key Event Type Constants
    const val KEY_EVENT_DOWN = 0
    const val KEY_EVENT_UP = 1

    // UI Event Data Constants
    const val CAPSULE_ACTION_CLOSE = "close"
    const val CAPSULE_ACTION_CLEAN_CACHE_RESTART = "clean_cache_restart"
    const val CAPSULE_ACTION_RESTART = "restart"
    const val CAPSULE_ACTION_UNINSTALL = "uninstall"
    const val NAVIGATION_ACTION_BACK = "back"
    const val NAVIGATION_ACTION_HOME = "home"

    /**
     * Initialize the Lingxia SDK with data and cache directories
     * @param dataDir Application data directory path
     * @param cacheDir Application cache directory path
     * @param assetManager Android AssetManager for accessing bundled assets
     * @param applicationContext The host application's Context, registered
     *   once with the platform crate so device/display APIs do not depend on
     *   any Activity lifecycle.
     * @param locale System locale (e.g., "en-US", "zh-CN")
     * @return Home app ID if successful, null otherwise
     */
    @JvmStatic
    external fun lingxiaInit(
        dataDir: String,
        cacheDir: String,
        assetManager: AssetManager,
        applicationContext: android.content.Context,
        locale: String
    ): String?

    /**
     * Notify native layer that an LxApp has been opened
     * @param appId The ID of the opened app
     * @param path The initial path/route of the app
     * @param sessionId Runtime session id
     * @return The resolved path that should be used
     */
    @JvmStatic
    external fun onLxAppOpened(appId: String, path: String, sessionId: Long): String

    /**
     * Notify native layer that an LxApp has been closed
     * @param appId The ID of the closed app
     * @param sessionId Runtime session id
     * @return true if close event is accepted for current session, false if stale/rejected
     */
    @JvmStatic
    external fun onLxAppClosed(appId: String, sessionId: Long): Boolean

    /**
     * Handle UI events from the UI layer
     * @param appId The ID of the app
     * @param eventType The type of UI event (use UI_EVENT_* constants)
     * @param data Event-specific data (e.g., tab index, button name)
     * @return 1 if event was handled, 0 otherwise
     */
    @JvmStatic
    external fun onLxappEvent(appId: String, eventType: Int, data: String): Int

    /**
     * Dispatch key event to native layer
     * @param appId The ID of the app
     * @param eventType 0=down, 1=up
     * @param payloadJson JSON payload of key event data
     * @return true if event was dispatched, false otherwise
     */
    @JvmStatic
    external fun onKeyEvent(appId: String, eventType: Int, payloadJson: String): Boolean

    /**
     * Dispatch device orientation change event to native layer.
     * @param appId The ID of the app
     * @param sessionId Runtime session id
     * @param value "portrait" or "landscape"
     * @return true if event was dispatched, false otherwise
     */
    @JvmStatic
    external fun onDeviceOrientationChanged(appId: String, sessionId: Long, value: String): Boolean

    /**
     * Get LxApp information using typed API
     * @param appId The ID of the app
     * @return LxApp information or null if not found
     */
    @JvmStatic
    external fun getLxAppInfo(appId: String): LxAppInfo?

    /**
     * Resolve a lx:// URI or sandbox path to a native-consumable filesystem path.
     *
     * Returns null if the input is not accessible inside the app sandbox.
     */
    @JvmStatic
    external fun resolveLxUri(appId: String, input: String): String?

    /**
     * Emit an SDK-side log entry into the Rust log pipeline.
     *
     * level: 0=verbose, 1=debug, 2=info, 3=warn, 4=error.
     * Returns false when the native log pipeline is not initialized or level is invalid.
     */
    @JvmStatic
    external fun emitSdkLog(
        level: Int,
        category: String,
        appId: String,
        path: String,
        message: String
    ): Boolean

    /**
     * Run the shared browser navigation policy classifier.
     *
     * Returns JSON with decision: in_webview | open_external | deny.
     */
    @JvmStatic
    external fun handleBrowserNavigationPolicy(requestJson: String): String?

    /**
     * Open a managed internal browser tab and return tabId.
     * Returns null when owner/session is invalid or creation fails.
     */
    @JvmStatic
    external fun openBrowserTab(appId: String, sessionId: Long, url: String): String?

    /**
     * Close a managed internal browser tab.
     */
    @JvmStatic
    external fun browserTabClose(tabId: String): Boolean

    /**
     * Get built-in browser appId.
     */
    @JvmStatic
    external fun getBuiltinBrowserAppId(): String?

    /**
     * Resolve managed browser tab path from tabId.
     */
    @JvmStatic
    external fun browserTabPathForId(tabId: String): String?

    /**
     * Get complete TabBar state with items array (unified API)
     * @param appId The ID of the app
     * @return Complete TabBar state or null if not available
     */
    @JvmStatic
    external fun getTabBarState(appId: String): TabBarState?

    /**
     * Get the navigation bar configuration for a specific page/path
     * @param appId The ID of the app
     * @param path The page path
     * @return Navigation bar configuration or null if not available
     */
    @JvmStatic
    external fun getNavigationBarState(appId: String, path: String): NavigationBarState?

    /**
     * Get page orientation for a specific page
     * @param appId The ID of the app
     * @param path The page path
     * @return Orientation int: 0=auto, 1=portrait, 2=landscape, 3=reverse-portrait, 4=reverse-landscape
     */
    @JvmStatic
    external fun getPageOrientation(appId: String, path: String): Int

    const val ORIENTATION_AUTO = 0
    const val ORIENTATION_PORTRAIT = 1
    const val ORIENTATION_LANDSCAPE = 2
    const val ORIENTATION_REVERSE_PORTRAIT = 3
    const val ORIENTATION_REVERSE_LANDSCAPE = 4

    /**
     * Check whether pull-to-refresh is enabled for a specific page/path
     * @param appId The ID of the app
     * @param path The page path
     * @return true if enabled, false otherwise
     */
    @JvmStatic
    external fun isPullDownRefreshEnabled(appId: String, path: String): Boolean

    /**
     * Notify native layer that a page is being shown
     * @param appId The ID of the app
     * @param path The path of the page being shown
     */
    @JvmStatic
    external fun onPageShow(appId: String, path: String)

    /**
     * Resolve and find an existing WebView instance for app/path/session in one call.
     * @return WebView instance or null if not found
     */
    @JvmStatic
    external fun findWebView(appId: String, path: String, sessionId: Long): com.lingxia.lxapp.WebView?

    @JvmStatic
    external fun findWebViewByPageInstanceId(pageInstanceId: String): com.lingxia.lxapp.WebView?

    @JvmStatic
    external fun notifyPageInstanceMounted(pageInstanceId: String): Boolean

    @JvmStatic
    external fun notifyPageInstanceVisible(pageInstanceId: String): Boolean

    @JvmStatic
    external fun notifyPageInstanceHidden(pageInstanceId: String, reason: String): Boolean

    @JvmStatic
    external fun disposePageInstance(pageInstanceId: String, reason: String): Boolean

    @JvmStatic
    external fun onSurfaceClosed(appId: String, id: String, reason: String): Boolean

    /**
     * Handle AppLink URL by passing the full URL to native layer.
     * @param applinkUrl The full AppLink URL (e.g., "https://www.lingxia.app/lxapp/shop/pages/detail?id=42")
     * @return 1 = handled, 0 = ignored, -1 = rejected
     */
    @JvmStatic
    external fun onAppLinkReceived(applinkUrl: String): Int

    /**
     * Get current active LxApp info from Rust stack
     * @return CurrentLxApp with appId, path and sessionId, or empty if no active LxApp
     */
    @JvmStatic
    external fun getCurrentLxApp(): CurrentLxApp?

    /**
     * Get runtime session id for a specific LxApp.
     * @return session id, or 0 if unavailable
     */
    @JvmStatic
    external fun getLxAppSessionId(appId: String): Long

    /**
     * Callback function for async operations
     * @param id Callback ID for correlating with pending operation
     * @param success Whether the operation completed successfully
     * @param data When success=true: JSON payload; when success=false: error code string
     * @return true if callback was handled, false otherwise
     */
    @JvmStatic
    external fun onCallback(id: Long, success: Boolean, data: String): Boolean

    /**
     * Dispatch NativeComponent event to Rust runtime for binding resolution + Page invocation.
     *
     * @param appId The app id owning the page
     * @param path The page path owning the component
     * @param componentId Component id emitting the event
     * @param eventName Native event name (normalized lower-case)
     * @param payloadJson Standardized event object JSON
     * @param bindingsJson JSON object map: eventName -> pageFunctionName
     * @return true when dispatch request is accepted, false otherwise
     */
    @JvmStatic
    external fun onNativeComponentEvent(
        appId: String,
        path: String,
        componentId: String,
        eventName: String,
        payloadJson: String,
        bindingsJson: String
    ): Boolean

    /**
     * Returns a bitmask of host app capabilities.
     * Check against LxApp.CAP_* constants to determine enabled host capabilities.
     */
    @JvmStatic
    external fun getAppCapabilities(): Int

    /**
     * Notify native layer that app entered foreground
     * Called from LxAppActivity.onStart
     * @param lxappId The ID of the lxapp
     */
    @JvmStatic
    external fun onAppShow(lxappId: String)

    /**
     * Notify native layer that app entered background
     * Called from LxAppActivity.onStop
     * @param lxappId The ID of the lxapp
     */
    @JvmStatic
    external fun onAppHide(lxappId: String)
}
