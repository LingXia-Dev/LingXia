package com.lingxia.lxapp

import android.content.res.AssetManager

/**
 * Centralized Native API declarations
 *
 * This object contains all JNI function declarations that interface with the Rust layer.
 * All native methods are organized by functionality and provide a single point of reference
 * for the FFI interface.
 */
object NativeApi {

    private var isLibraryLoaded = false

    init {
        synchronized(this) {
            if (!isLibraryLoaded) {
                System.loadLibrary("lingxia")
                isLibraryLoaded = true
            }
        }
    }

    // UI Event Type Constants
    const val UI_EVENT_TABBAR_CLICK = 0
    const val UI_EVENT_CAPSULE_CLICK = 1
    const val UI_EVENT_NAVIGATION_CLICK = 2
    const val UI_EVENT_BACK_PRESS = 3

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
     * @return Home app ID if successful, null otherwise
     */
    @JvmStatic
    external fun onLxAppInited(
        dataDir: String,
        cacheDir: String,
        assetManager: AssetManager
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
}
