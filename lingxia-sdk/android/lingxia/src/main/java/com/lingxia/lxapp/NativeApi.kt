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
object NativeApi {
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
     * Initialize the LxApp system with data and cache directories
     * @param dataDir Application data directory path
     * @param cacheDir Application cache directory path
     * @param assetManager Android AssetManager for accessing bundled assets
     * @param locale System locale (e.g., "en-US", "zh-CN")
     * @return Home app ID if successful, null otherwise
     */
    @JvmStatic
    external fun onLxAppInited(
        dataDir: String,
        cacheDir: String,
        assetManager: AssetManager,
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
    external fun onUiEvent(appId: String, eventType: Int, data: String): Int

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
     * Run the shared browser address input handler.
     *
     * The input and output are JSON payloads so the schema can evolve without
     * repeatedly changing platform FFI signatures.
     */
    @JvmStatic
    external fun handleBrowserAddressInput(requestJson: String): String?

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
     * Find an existing WebView instance for the given app and path
     * @param appId The ID of the app
     * @param path The page path
     * @param sessionId Runtime session id
     * @return WebView instance or null if not found
     */
    @JvmStatic
    external fun findWebView(appId: String, path: String, sessionId: Long): com.lingxia.lxapp.WebView?

    /**
     * Handle AppLink URL by passing the full URL to native layer
     * @param applinkUrl The full AppLink URL (e.g., "https://www.lingxia.app/12/3")
     * @return Status code (0 = success)
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
