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
            runCatching {
                System.loadLibrary("lingxia")
                libraryLoaded.set(true)
                Log.d(TAG, "Native library 'lingxia' loaded")
            }.onFailure { error ->
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

    // UI Event Data Constants
    const val CAPSULE_ACTION_MORE = "more"
    const val CAPSULE_ACTION_CLOSE = "close"
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
     * @return The resolved path that should be used
     */
    @JvmStatic
    external fun onLxAppOpened(appId: String, path: String): String

    /**
     * Notify native layer that an LxApp has been closed
     * @param appId The ID of the closed app
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onLxAppClosed(appId: String): Int

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
     * Get LxApp information using typed API
     * @param appId The ID of the app
     * @return LxApp information or null if not found
     */
    @JvmStatic
    external fun getLxAppInfo(appId: String): LxAppInfo?

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
     * @return WebView instance or null if not found
     */
    @JvmStatic
    external fun findWebView(appId: String, path: String): com.lingxia.lxapp.WebView?

    /**
     * Handle AppLink URL by passing the full URL to native layer
     * @param applinkUrl The full AppLink URL (e.g., "https://www.lingxia.app/12/3")
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onAppLinkReceived(applinkUrl: String): Int

    /**
     * Get current active LxApp ID and path from Rust stack
     * @return CurrentLxApp with appId and path, or empty if no active LxApp
     */
    @JvmStatic
    external fun getCurrentLxApp(): CurrentLxApp?

    /**
     * Callback function for async operations
     * @param id Callback ID
     * @param success Whether the operation was successful
     * @param data Result data as JSON string
     * @return true if callback was handled, false otherwise
     */
    @JvmStatic
    external fun onCallback(id: Long, success: Boolean, data: String): Boolean
}
