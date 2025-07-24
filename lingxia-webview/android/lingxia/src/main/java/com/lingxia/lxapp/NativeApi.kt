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
     * Get the TabBar configuration for a mini app
     * @param appId The ID of the app
     * @return TabBar configuration or null if not available
     */
    @JvmStatic
    external fun getTabBarConfig(appId: String): TabBarConfig?

    /**
     * Get a specific TabBar item by index
     * @param appId The ID of the app
     * @param index The index of the tab item
     * @return TabBar item or null if not found
     */
    @JvmStatic
    external fun getTabBarItem(appId: String, index: Int): TabBarItem?

    /**
     * Get the navigation bar configuration for a specific page/path
     * @param appId The ID of the app
     * @param path The page path
     * @return Navigation bar configuration or null if not available
     */
    @JvmStatic
    external fun getNavigationBarConfig(appId: String, path: String): NavigationBarConfig?

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
     * Handle post message from WebView JavaScript to native layer
     * @param appId The ID of the app
     * @param path The page path
     * @param message The message content
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun handlePostMessage(appId: String, path: String, message: String): Int

    /**
     * Notify native layer that page loading has started
     * @param appId The ID of the app
     * @param path The page path
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onPageStarted(appId: String, path: String): Int

    /**
     * Notify native layer that page loading has finished
     * @param appId The ID of the app
     * @param path The page path
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onPageFinished(appId: String, path: String): Int

    /**
     * Check if URL loading should be overridden
     * @param appId The ID of the app
     * @param url The URL being loaded
     * @return Status code (0 = allow, 1 = override)
     */
    @JvmStatic
    external fun shouldOverrideUrlLoading(appId: String, url: String): Int

    /**
     * Handle HTTP request interception
     * @param appId The ID of the app
     * @param url The request URL
     * @param method The HTTP method
     * @param headers The request headers as JSON string
     * @return WebResourceResponseData or null if not intercepted
     */
    @JvmStatic
    external fun handleRequest(
        appId: String,
        url: String,
        method: String,
        headers: String
    ): WebResourceResponseData?

    /**
     * Handle console message from WebView
     * @param appId The ID of the app
     * @param path The page path
     * @param level The log level (0=debug, 1=info, 2=warn, 3=error)
     * @param message The console message
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onConsoleMessage(appId: String, path: String, level: Int, message: String): Int

    /**
     * Handle scroll position changes in WebView
     * @param appId The ID of the app
     * @param path The page path
     * @param scrollX Current horizontal scroll position
     * @param scrollY Current vertical scroll position
     * @param maxScrollX Maximum horizontal scroll
     * @param maxScrollY Maximum vertical scroll
     * @return Status code (0 = success)
     */
    @JvmStatic
    external fun onScrollChanged(
        appId: String,
        path: String,
        scrollX: Int,
        scrollY: Int,
        maxScrollX: Int,
        maxScrollY: Int
    ): Int
}
