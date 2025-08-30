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

    init {
        System.loadLibrary("lingxia")
    }

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
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onLxAppOpened(appId: String, path: String): Int

    /**
     * Notify native layer that an LxApp has been closed
     * @param appId The ID of the closed app
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onLxAppClosed(appId: String): Int

    /**
     * Handle back button press for an app
     * @param appId The ID of the app handling the back press
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onBackPressed(appId: String): Int

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
}
