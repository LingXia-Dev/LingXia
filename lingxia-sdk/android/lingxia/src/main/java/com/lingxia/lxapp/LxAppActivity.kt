package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import org.json.JSONObject

import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import android.provider.Settings
import android.content.pm.ActivityInfo
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import android.view.animation.AccelerateDecelerateInterpolator
import com.lingxia.lxapp.NativeComponents.NativeBridge
import com.lingxia.lxapp.LxNavBarUtils

/**
 * Animation type enum for page transitions
 */
enum class AnimationType(val value: Int) {
    /**
     * No animation - used for Launch/Replace/SwitchTab semantics
     */
    NONE(0),

    /**
     * Forward animation - push-style animation
     */
    FORWARD(1),

    /**
     * Backward animation - pop-style animation
     */
    BACKWARD(2);

    companion object {
        /**
         * Convert AnimationType to string for logging
         */
        fun toString(type: AnimationType): String {
            return when (type) {
                NONE -> "None"
                FORWARD -> "Forward"
                BACKWARD -> "Backward"
            }
        }

        /**
         * Convert integer to AnimationType
         */
        fun fromInt(value: Int): AnimationType {
            return when (value) {
                1 -> FORWARD
                2 -> BACKWARD
                else -> NONE // 0 or any other value
            }
        }
    }
}

/**
 * Simple navigation state tracker
 */
data class NavigationState(
    val currentPath: String,
    val previousPath: String? = null,
    val isNavigating: Boolean = false
)

class LxAppActivity : AppCompatActivity() {
    private data class MediaFullscreenState(
        val tabBarVisibility: Int,
        val navigationBarVisibility: Int,
        val navigationBarIndex: Int,
        val navigationBarLayoutParams: ViewGroup.LayoutParams?,
        val overlayLayoutParams: FrameLayout.LayoutParams?,
        val overlayTranslationX: Float,
        val overlayTranslationY: Float,
        val rootPaddingLeft: Int,
        val rootPaddingTop: Int,
        val rootPaddingRight: Int,
        val rootPaddingBottom: Int
    )
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"
        const val EXTRA_SESSION_ID = "sessionId"
        internal const val DEFAULT_NAV_BAR_HEIGHT_DP = 44

        /**
         * Update TabBar UI for a specific appId
         * In single-activity architecture, updates the current activity's TabBar
         */
        @JvmStatic
        fun updateTabBarUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.appId == appId) {
                // Run on UI thread to update TabBar directly
                activity.runOnUiThread {
                    try {
                        // Get fresh TabBar state from Rust
                        val newTabBarConfig = NativeApi.getTabBarState(appId)
                        if (newTabBarConfig != null) {
                            // Update existing TabBar with new configuration
                            activity.setupTabBar(newTabBarConfig)

                        } else {
                            Log.w(TAG, "No TabBar config available for refresh")
                        }
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to refresh TabBar from Rust: ${e.message}", e)
                    }
                }

                return true
            } else {
                Log.w(TAG, "No matching activity found for appId: $appId (current: ${activity?.appId})")
                return false
            }
        }

        /**
         * Update NavigationBar UI for a specific appId
         * Triggers the NavigationBar to refresh its state from Rust
         */
        @JvmStatic
        fun updateNavBarUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.appId == appId) {
                activity.runOnUiThread {
                    val currentPath = activity.currentWebView?.getCurrentPath() ?: ""
                    activity.updateNavigationBar(null, false, true, currentPath)
                }
                return true
            }
            return false
        }

        /**
         * Update orientation UI for a specific appId
         * Re-applies the current page orientation from native config.
         */
        @JvmStatic
        fun updateOrientationUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.appId == appId) {
                activity.runOnUiThread {
                    val currentPath = activity.currentWebView?.getCurrentPath() ?: ""
                    activity.applyPageOrientation(currentPath)
                }
                return true
            }
            Log.w(TAG, "No matching activity found for appId: $appId (current: ${activity?.appId})")
            return false
        }

        // Helper function to get status bar height
        fun getStatusBarHeight(context: Context): Int {
            // Try WindowInsets API (API 23+)
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
                val activity = context as? Activity
                val insetTop = activity?.window?.decorView?.rootWindowInsets
                    ?.let { WindowInsetsCompat.toWindowInsetsCompat(it) }
                    ?.getInsets(WindowInsetsCompat.Type.statusBars())
                    ?.top ?: 0
                if (insetTop > 0) {
                    return insetTop
                }
            }

            // Fallback: use system resource
            var result = 0
            val resourceId = context.resources.getIdentifier("status_bar_height", "dimen", "android")
            if (resourceId > 0) {
                result = context.resources.getDimensionPixelSize(resourceId)
            }
            // Fallback if resource not found (less likely but safe)
            if (result == 0) {
                result = (24 * context.resources.displayMetrics.density).toInt()
            }
            return result
        }

        /**
         * Configures the system bars (status bar and navigation bar) to be transparent
         * and edge-to-edge. This method enables immersive view experience.
         *
         * @param activity The activity whose system bars should be configured
         * @param lightStatusBarIcons Whether the status bar icons should be light (true) or dark (false)
         */
        @JvmStatic
        fun configureTransparentSystemBars(activity: AppCompatActivity, lightStatusBarIcons: Boolean = true) {
            // Enable Edge-to-Edge using WindowCompat
            WindowCompat.setDecorFitsSystemWindows(activity.window, false)

            activity.window.apply {
                addFlags(android.view.WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)
                // Set initial status bar to transparent - navbar will override when needed
                statusBarColor = Color.TRANSPARENT
            }

            WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
                isAppearanceLightStatusBars = lightStatusBarIcons
                isAppearanceLightNavigationBars = lightStatusBarIcons
            }
        }

        @JvmStatic
        fun updateNavigationBarTransparency(activity: AppCompatActivity, isTabBarTransparent: Boolean, tabBarBackgroundColor: Int? = null) {
            activity.window.apply {
                if (isTabBarTransparent) {
                    // TabBar is transparent, make navigation bar transparent for overlay effect
                    addFlags(android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS)
                    navigationBarColor = Color.TRANSPARENT
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        isNavigationBarContrastEnforced = false
                    }
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                        navigationBarDividerColor = Color.TRANSPARENT
                    }
                } else {
                    // TabBar is not transparent, use TabBar's background color for navigation bar
                    clearFlags(android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS)

                    // Use TabBar's background color, fallback to white if not provided
                    val navBarColor = tabBarBackgroundColor ?: Color.WHITE
                    navigationBarColor = navBarColor

                    // Set contrast enforcement based on color brightness
                    val brightness = (Color.red(navBarColor) * 0.299 + Color.green(navBarColor) * 0.587 + Color.blue(navBarColor) * 0.114)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        isNavigationBarContrastEnforced = brightness > 128 // Light background
                    }

                    // Remove divider completely for seamless appearance
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                        navigationBarDividerColor = Color.TRANSPARENT
                    }
                }
            }
        }
    }

    internal lateinit var appId: String
    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    internal var pullToRefreshHelper: PullToRefreshHelper? = null
    private var tabBar: TabBar? = null
    private var navigationBar: NavigationBar? = null
    private var isDestroyed = false
    private var hasEnteredBackground = false
    private var currentSessionId: Long = 0L

    // Tracks the currently visible WebView instance
    private var currentWebView: com.lingxia.lxapp.WebView? = null
    private var systemBottomInset: Int = 0
    private var isMediaFullscreen = false
    private var isPageFullscreen = false  // For page-level fullscreen (landscape + custom navbar)
    private var mediaFullscreenState: MediaFullscreenState? = null
    private var pendingTabBarVisibility: Int? = null
    private var pendingNavBarVisibility: Int? = null
    private var shouldRestoreOverlayOrder = false
    private var lastDispatchedDeviceOrientation: String? = null

    private fun ensureRuntimeReady(
        targetAppId: String,
        requestedPath: String,
        requestedSessionId: Long
    ): Pair<String, Long>? {
        if (LxApp.HomeLxAppId == null) {
            Log.e(TAG, "LxApp runtime is not initialized before LxAppActivity creation")
            return null
        }

        var sessionId = NativeApi.getLxAppSessionId(targetAppId)
        if (sessionId <= 0L) {
            sessionId = requestedSessionId
        } else if (requestedSessionId > 0L && requestedSessionId != sessionId) {
            Log.w(TAG, "Ignoring stale intent sessionId=$requestedSessionId for appId=$targetAppId, using runtime sessionId=$sessionId")
        }

        if (sessionId <= 0L) {
            Log.e(TAG, "Missing valid runtime session for appId=$targetAppId")
            return null
        }

        val current = NativeApi.getCurrentLxApp()
        val currentMatches = current != null &&
            current.isValid() &&
            current.appId == targetAppId &&
            current.sessionId == sessionId
        val resolvedPath = if (currentMatches && !current.path.isNullOrBlank()) {
            current.path
        } else {
            requestedPath
        }

        return Pair(resolvedPath, sessionId)
    }

    private fun shouldShowCapsuleButton(targetAppId: String, targetSessionId: Long): Boolean {
        if (isMediaFullscreen) return false
        if (targetAppId.isBlank() || targetSessionId <= 0L) return false

        val homeAppId = LxApp.HomeLxAppId ?: return false
        if (targetAppId == homeAppId) return false

        val current = NativeApi.getCurrentLxApp() ?: return false
        if (!current.isValid()) return false

        return current.appId == targetAppId && current.sessionId == targetSessionId
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Configure transparent system bars for edge-to-edge experience
        configureTransparentSystemBars(this)

        // Set reference to this activity in LxApp
        LxApp.setCurrentActivity(this)

        // Initialize appId from intent FIRST (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            Log.e(TAG, "Missing required parameter: appId")
            finish()
            return
        }
        var initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""
        val requestedSessionId = intent.getLongExtra(EXTRA_SESSION_ID, 0L)
        val resolvedEntry = ensureRuntimeReady(appId, initialPath, requestedSessionId) ?: run {
            finish()
            return
        }
        initialPath = resolvedEntry.first
        currentSessionId = resolvedEntry.second
        if (currentSessionId <= 0L) {
            Log.e(TAG, "Runtime returned invalid session for appId=$appId")
            finish()
            return
        }

        // Apply initial page orientation before first frame to avoid visible startup rotation.
        // Full orientation/immersive sync still happens later in setupWebViewContentWithExisting().
        runCatching {
            val initialOrientation = NativeApi.getPageOrientation(appId, normalizePath(initialPath))
            updateRequestedOrientation(initialOrientation)
        }.onFailure { error ->
            Log.w(TAG, "Failed to apply initial orientation before first frame: ${error.message}")
        }

        // Start WebView creation in parallel while setting up UI
        var webViewFuture: java.util.concurrent.Future<com.lingxia.lxapp.WebView?>? = null
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()

        try {
            webViewFuture = executor.submit<com.lingxia.lxapp.WebView?> {

                findWebView(appId, initialPath)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to start parallel WebView creation: ${e.message}")
        }

        // Create root container first
        rootContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.TRANSPARENT)
        }

        setContentView(rootContainer)

        // Get TabBar config and setup UI in parallel
        val tabBarConfig = NativeApi.getTabBarState(appId)

        // Setup containers and UI components
        setupWebViewContainer()
        setupTabBar(tabBarConfig)

        // Create global NavigationBar (always present, controlled by visibility)
        createNavBar()

        // Defer capsule button creation to post-layout
        rootContainer.post {
            addCapsuleButton()
        }

        // Setup window insets listener
        ViewCompat.setOnApplyWindowInsetsListener(rootContainer) { view, insets ->
            if (isMediaFullscreen || isPageFullscreen) {
                view.setPadding(0, 0, 0, 0)
                return@setOnApplyWindowInsetsListener WindowInsetsCompat.CONSUMED
            }
            val sysBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            systemBottomInset = resolveContentBottomInset(insets)

            val currentBg = tabBar?.config?.backgroundColor ?: tabBarConfig?.backgroundColor
            val isTabBarTransparent = currentBg == Color.TRANSPARENT ||
                                     (currentBg?.let { Color.alpha(it) < 255 } == true)

            if (isTabBarTransparent) {
                // Let TabBar overlay nav bar; do not pad root
                view.setPadding(0, 0, 0, 0)
            } else {
                // Non-transparent TabBar: keep default system bars padding for root
                view.setPadding(sysBars.left, 0, sysBars.right, sysBars.bottom)
            }

            // Re-apply TabBar layout so bottom margin reflects latest inset when transparent
            tabBar?.config?.let { cfg ->
                tabBar?.let { tb -> applyTabBarLayoutParams(tb, cfg) }
            }
            insets
        }

        // Setup WebView content using parallel result
        try {
            val webViewResult = webViewFuture?.get(500, java.util.concurrent.TimeUnit.MILLISECONDS)
            if (webViewResult != null) {
                setupWebViewContentWithExisting(webViewResult)
            } else {
                setupWebViewContent(appId, initialPath)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Parallel WebView creation timeout/error, falling back to sync: ${e.message}")
            setupWebViewContent(appId, initialPath)
        } finally {
            executor.shutdown()
        }

        // Defer non-critical setup to post-layout
        rootContainer.post {
            // Setup back press handler
            onBackPressedDispatcher.addCallback(object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    try {
                        currentWebView?.visibility = View.VISIBLE
                        NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_BACK_PRESS, "")
                    } catch (e: Exception) {
                        Log.e(TAG, "Error handling back press: ${e.message}")
                    }
                }
            })
        }
    }

    /**
     * Provides a single source of truth for bottom content inset.
     * Gesture nav -> 0, 3-button visible -> visible bottom inset, others -> 0.
     */
    fun getContentBottomInset(): Int = systemBottomInset

    private fun resolveContentBottomInset(insets: WindowInsetsCompat): Int {
        val navVisible = insets.isVisible(WindowInsetsCompat.Type.navigationBars())
        val gestureInset = insets.getInsets(WindowInsetsCompat.Type.systemGestures()).bottom
        val visible = maxOf(
            insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom,
            insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
        )
        val stable = maxOf(
            insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.navigationBars()).bottom,
            insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.systemBars()).bottom
        )

        val clearGesture = !navVisible && visible == 0 && gestureInset > 0
        val navMode = resolveNavigationMode()
        when (navMode) {
            2 -> return 0 // gesture navigation: keep content flush
            0, 1 -> return if (clearGesture) 0 else visible // legacy 3-button/2-button
        }

        if (clearGesture) return 0
        if (navVisible && visible > 0) return visible
        // Some OEMs report stable>0 for gesture; do not use stable for content inset
        if (!navVisible && visible == 0 && stable > 0 && gestureInset == 0) return 0
        return 0
    }

    private fun resolveNavigationMode(): Int? {
        return try {
            Settings.Secure.getInt(contentResolver, "navigation_mode")
        } catch (_: Throwable) {
            val resId = resources.getIdentifier("config_navBarInteractionMode", "integer", "android")
            if (resId > 0) resources.getInteger(resId) else null
        }
    }

    private fun setupContainers() {
        // Create root container
        rootContainer = FrameLayout(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Set transparent background to allow TabBar transparency to work
            setBackgroundColor(Color.TRANSPARENT)
        }
        setContentView(rootContainer)

        // Create WebView container
        webViewContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Set transparent background to allow TabBar transparency to work
            setBackgroundColor(Color.TRANSPARENT)
        }
        rootContainer.addView(webViewContainer)
    }

    private fun setupTabBar(config: TabBarState?) {
        if (config == null) {

            return
        }

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        // Get the actual TabBar background color (considering defaults)
        val actualTabBarColor: Int = when (config.position) {
            TabBarState.Position.BOTTOM -> config.backgroundColor // Use configured color for bottom
            else -> {
                // Use vertical TabBar default color for LEFT/RIGHT positions
                0xFFF8F8F8.toInt() // VERTICAL_TABBAR_BACKGROUND_COLOR
            }
        }

        // Update system navigation bar transparency based on TabBar transparency and color
        updateNavigationBarTransparency(this, isTabBarTransparent, actualTabBarColor)

        if (tabBar == null) {
            tabBar = TabBar(this).apply {
                setConfig(config)
                setOnTabSelectedListener { index, path ->
                    // Use new UI event API
                    NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_TABBAR_CLICK, index.toString())
                }
                applyTabBarLayoutParams(this, config)
            }

            if (isTabBarTransparent) {
                if (webViewContainer.parent == null) {
                    rootContainer.addView(webViewContainer)
                }
                rootContainer.addView(tabBar)
            } else {
                rootContainer.addView(tabBar)
            }
        } else {
            tabBar?.setConfig(config)
            tabBar?.let { tb -> applyTabBarLayoutParams(tb, config) }
            // Update navigation bar transparency when tabbar config changes
            updateNavigationBarTransparency(this, isTabBarTransparent, actualTabBarColor)
        }

        updateLayoutMargins()
    }

    private fun applyTabBarLayoutParams(tabBar: TabBar, config: TabBarState) {
        val isVertical = config.position == TabBarState.Position.LEFT || config.position == TabBarState.Position.RIGHT
        val density = resources.displayMetrics.density
        // Use configured dimension (Rust provides default value)
        val tabBarDimension = config.dimension ?: 64 // Fallback just in case
        val tabBarSizePx = (tabBarDimension * density).toInt()

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        (tabBar.layoutParams as? FrameLayout.LayoutParams)?.apply {
            if (isVertical) {
                width = tabBarSizePx
                height = ViewGroup.LayoutParams.MATCH_PARENT
                gravity = when (config.position) {
                    TabBarState.Position.LEFT -> Gravity.START
                    TabBarState.Position.RIGHT -> Gravity.END
                    else -> Gravity.START
                }
                // Add top margin to avoid status bar for vertical TabBars
                topMargin = getStatusBarHeight(this@LxAppActivity)
                bottomMargin = 0
            } else {
                width = ViewGroup.LayoutParams.MATCH_PARENT
                height = tabBarSizePx
                gravity = Gravity.BOTTOM
                // For a transparent TabBar, lift it above the system navigation bar
                bottomMargin = if (isTabBarTransparent) systemBottomInset else 0
            }
            tabBar.layoutParams = this
        } ?: run {
            // Create new LayoutParams with correct initial dimensions
            val newLayoutParams = if (isVertical) {
                FrameLayout.LayoutParams(tabBarSizePx, ViewGroup.LayoutParams.MATCH_PARENT).apply {
                    gravity = when (config.position) {
                        TabBarState.Position.LEFT -> Gravity.START
                        TabBarState.Position.RIGHT -> Gravity.END
                        else -> Gravity.START
                    }
                    // Add top margin to avoid status bar for vertical TabBars
                    topMargin = getStatusBarHeight(this@LxAppActivity)
                    bottomMargin = 0
                }
            } else {
                FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, tabBarSizePx).apply {
                    gravity = Gravity.BOTTOM
                    // For a transparent TabBar, lift it above the system navigation bar
                    bottomMargin = if (isTabBarTransparent) systemBottomInset else 0
                }
            }
            tabBar.layoutParams = newLayoutParams
        }
    }

    private fun updateLayoutMargins() {
        if (isMediaFullscreen) {
            navigationBar?.visibility = View.GONE
            tabBar?.visibility = View.GONE
            (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
                topMargin = 0
                bottomMargin = 0
                leftMargin = 0
                rightMargin = 0
                webViewContainer.layoutParams = this
                webViewContainer.requestLayout()
            }
            val container = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
            container?.translationY = 0f
            container?.requestLayout()
            return
        }
        val isTabBarVisible = tabBar?.visibility == View.VISIBLE
        val tabBarHeight = tabBar?.layoutParams?.height ?: 0
        val tabBarWidth = tabBar?.layoutParams?.width ?: 0
        val tabBarBgColor = tabBar?.config?.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        // Calculate NavigationBar height for webview margin
        val isNavBarVisible = navigationBar?.visibility == View.VISIBLE
        val navBarHeight = if (isNavBarVisible) {
            // WebView should start right after navbar (navbar content height + statusbar height)
            val statusBarHeight = getStatusBarHeight(this)
            val navBarContentHeight = navigationBar?.getCalculatedContentHeightPx() ?: 0
            navBarContentHeight + statusBarHeight
        } else {
            0
        }

        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = navBarHeight  // Use NavigationBar total height
            bottomMargin = 0
            leftMargin = 0
            rightMargin = 0

            if (!isTabBarTransparent) {
                when (tabBar?.config?.position) {
                    TabBarState.Position.BOTTOM -> {
                        if (isTabBarVisible) bottomMargin = tabBarHeight
                    }
                    TabBarState.Position.LEFT -> {
                        if (isTabBarVisible) leftMargin = tabBarWidth
                    }
                    TabBarState.Position.RIGHT -> {
                        if (isTabBarVisible) rightMargin = tabBarWidth
                    }
                    null -> { }
                }
            }

            webViewContainer.layoutParams = this
            webViewContainer.requestLayout()
        }

        val container = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
        container?.translationY = if (!isTabBarTransparent) calculateWebViewTranslationY() else 0f
        container?.requestLayout()
    }



    // Find WebView - ONLY find WebView, nothing else
    private fun findWebView(appId: String, path: String, sessionId: Long = currentSessionId): com.lingxia.lxapp.WebView? {
        if (sessionId <= 0L) {
            Log.w(TAG, "findWebView called with invalid sessionId for appId=$appId, path=$path")
            return null
        }
        val webView = com.lingxia.lxapp.WebView.findWebView(appId, path, sessionId)
        if (webView == null) {
            Log.w(TAG, "WebView not found for appId=$appId, path=$path")
        }
        return webView
    }

    // Get navbar state
    private fun getNavBarState(appId: String, path: String): NavigationBarState? {
        return NativeApi.getNavigationBarState(appId, path)
    }

    // Helper function to attach a WebView to the container and resume it
    private fun attachWebViewToUI(view: com.lingxia.lxapp.WebView?) {
        if (view == null) {
            Log.e(TAG, "attachWebViewToUI called with null view!")
            return
        }
        if (!isDestroyed) {

            // Ensure view is visible
            view.visibility = View.VISIBLE

            val existingWrapper = (view.parent as? ViewGroup)?.takeIf { it.parent == webViewContainer }

            val container = existingWrapper ?: FrameLayout(this).apply {
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                tag = "current_webview_container"

                if (view.parent != null && view.parent != this) {
                    (view.parent as? ViewGroup)?.removeView(view)
                }
                view.layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                addView(view)
            }

            if (existingWrapper == null) {
                webViewContainer.addView(container)
            } else {
                webViewContainer.bringChildToFront(container)
                container.tag = "current_webview_container"
            }

            // Attach native bridge for component overlay
            NativeBridge.attachIfNeeded(view)

            ensurePullToRefreshHelper().attachToWebView(view)
            updatePullToRefreshEnabledForPath(view.getCurrentPath())

            // Resume the WebView's activities
            view.resume()
        } else {
            Log.w(TAG, "attachWebViewToUI: Activity is destroyed, skipping WebView attachment")
        }
    }

    private fun setupWebViewContent(appId: String, path: String) {
        val initialWebView = findWebView(appId, path)
        if (initialWebView == null) {
            Log.e(TAG, "Initial WebView missing for appId=$appId, path=$path")
            finishWithSessionClose("initial_webview_missing")
            return
        }
        setupWebViewContentWithExisting(initialWebView)
    }

    private fun finishWithSessionClose(reason: String) {
        if (::appId.isInitialized && appId.isNotBlank() && currentSessionId > 0L) {
            val closed = runCatching { notifyLxAppClosed(currentSessionId) }.getOrElse { error ->
                Log.w(TAG, "finishWithSessionClose notify failed (reason=$reason): ${error.message}")
                false
            }
            if (!closed) {
                Log.w(TAG, "finishWithSessionClose stale/ignored close (reason=$reason, appId=$appId, sessionId=$currentSessionId)")
            }
        }
        // Prevent onStop() from dispatching onAppHide for a session that is already closed.
        appId = ""
        currentSessionId = 0L
        finish()
    }

    // New method to setup WebView content with an existing WebView
    private fun setupWebViewContentWithExisting(webView: com.lingxia.lxapp.WebView) {
        // Set the current WebView first
        this.currentWebView = webView

        // Attach and resume immediately
        attachWebViewToUI(webView)

        // Update navbar and statusbar for initial page
        val currentPath = webView.getCurrentPath() ?: ""
        val navbarState = NativeApi.getNavigationBarState(appId, currentPath)
        if (navbarState != null) {
            updateNavigationBar(navbarState, false, true, currentPath)
        }
        updatePullToRefreshEnabledForPath(currentPath)

        // Apply page orientation
        applyPageOrientation(currentPath)

        // Trigger onPageShow for initial WebView (this is the single place for initial page show)
        if (webView.getAppId() != null && webView.getCurrentPath() != null) {
            NativeApi.onPageShow(webView.getAppId()!!, webView.getCurrentPath()!!)
        }
    }

    // Function to setup the FrameLayout that holds the WebViews
    private fun setupWebViewContainer() {
        webViewContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Use transparent background to respect page configuration
            setBackgroundColor(Color.TRANSPARENT)
        }

        if (webViewContainer.parent == null) {
            rootContainer.addView(webViewContainer)
        }

        ensurePullToRefreshHelper()
    }

    private fun ensurePullToRefreshHelper(): PullToRefreshHelper {
        if (pullToRefreshHelper == null) {
            pullToRefreshHelper = PullToRefreshHelper(this, webViewContainer) { handlePullToRefresh() }
        }
        return pullToRefreshHelper!!
    }

    private fun normalizePath(rawPath: String?): String {
        if (rawPath.isNullOrEmpty()) return ""
        return rawPath.substringBefore('?').substringBefore('#')
    }

    private fun updatePullToRefreshEnabledForPath(path: String?) {
        val helper = pullToRefreshHelper ?: return

        val normalized = normalizePath(path)
        if (normalized.isEmpty()) {
            helper.setEnabled(true)
            return
        }

        val enabled = try {
            NativeApi.isPullDownRefreshEnabled(appId, normalized)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read pull-to-refresh config for $path: ${e.message}")
            true // fall back to enabled if config lookup fails
        }
        helper.setEnabled(enabled)
    }

    private fun handlePullToRefresh() {
        val helper = pullToRefreshHelper ?: return
        if (!helper.isEnabled()) {
            helper.endRefreshing()
            return
        }

        val path = normalizePath(currentWebView?.getCurrentPath())
        if (path.isEmpty()) {
            helper.endRefreshing()
            return
        }

        // Notify Rust layer via on_ui_event
        try {
            NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_PULL_DOWN_REFRESH, path)
        } catch (e: Exception) {
            Log.e(TAG, "onUiEvent pull-to-refresh failed: ${e.message}")
            helper.endRefreshing()
        }
    }

    private fun addCapsuleButton() {
        if (!shouldShowCapsuleButton(appId, currentSessionId)) return

        val density = resources.displayMetrics.density
        val statusBarHeight = getStatusBarHeight(this)
        val capsuleHeightPx = (LxAppTheme.Metrics.CAPSULE_HEIGHT_DP * density).toInt()
        val capsuleTopMarginPx = LxAppTheme.Metrics.calculateCapsuleTopMargin(statusBarHeight, density)

        val capsule = CapsuleButton(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                capsuleHeightPx
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                topMargin = capsuleTopMarginPx
                rightMargin = (LxAppTheme.Metrics.CAPSULE_TRAILING_MARGIN_DP * density).toInt()
            }
            setOnMenuClickListener {
                CapsuleMenuBottomSheet.show(this@LxAppActivity, appId)
            }
            setOnCloseClickListener {
                NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_CAPSULE_CLICK, NativeApi.CAPSULE_ACTION_CLOSE)
            }
        }

        rootContainer.post {
            rootContainer.removeView(rootContainer.findViewWithTag("capsule_button"))
            rootContainer.addView(capsule)
        }
    }

    fun getCapsuleRectJSON(): String {
        if (!shouldShowCapsuleButton(appId, currentSessionId)) {
            return "{}"
        }

        val capsuleView = rootContainer.findViewWithTag<View>("capsule_button") ?: return "{}"
        if (!capsuleView.isShown) {
            return "{}"
        }

        val widthPx = capsuleView.width
        val heightPx = capsuleView.height
        if (widthPx <= 0 || heightPx <= 0) {
            return "{}"
        }

        val density = resources.displayMetrics.density
        val statusBarHeight = getStatusBarHeight(this)

        val capsuleTopDp = LxAppTheme.Metrics.calculateCapsuleTopDp(statusBarHeight, density)

        // Web layout uses items-center with height+16, causing an 8px centering offset
        // Compensate by returning capsule_position - 8 (same strategy as iOS)
        val top = (capsuleTopDp - 8).toDouble()

        val width = widthPx / density
        val height = LxAppTheme.Metrics.CAPSULE_HEIGHT_DP.toDouble()

        val screenWidth = resources.displayMetrics.widthPixels / density
        val right = screenWidth - LxAppTheme.Metrics.CAPSULE_TRAILING_MARGIN_DP
        val left = right - width
        val bottom = top + height

        Log.i(
            TAG,
            "Capsule rect: top=${String.format("%.1f", top)}dp (capsule=${String.format("%.1f", capsuleTopDp)}dp, offset=-8) " +
                "width=${String.format("%.1f", width)}dp height=${String.format("%.1f", height)}dp"
        )

        return JSONObject().apply {
            put("width", width)
            put("height", height)
            put("top", top)
            put("right", right)
            put("bottom", bottom)
            put("left", left)
        }.toString()
    }

    override fun onResume() {
        super.onResume()
        webViewContainer.visibility = View.VISIBLE
        attachWebViewToUI(currentWebView)
        val currentPath = currentWebView?.getCurrentPath()
        if (!currentPath.isNullOrEmpty()) {
            applyPageOrientation(currentPath)
        }
        // Resume native components
        currentWebView?.let { NativeBridge.notifyPageActive(it) }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (::appId.isInitialized && appId.isNotBlank()) {
            val action = event.action
            if (action == KeyEvent.ACTION_DOWN || action == KeyEvent.ACTION_UP) {
                val payload = buildKeyEventPayload(event)
                if (payload != null) {
                    val eventType = if (action == KeyEvent.ACTION_DOWN) {
                        NativeApi.KEY_EVENT_DOWN
                    } else {
                        NativeApi.KEY_EVENT_UP
                    }
                    runCatching {
                        NativeApi.onKeyEvent(appId, eventType, payload)
                    }.onFailure { error ->
                        Log.w(TAG, "onKeyEvent failed: ${error.message}")
                    }
                }
            }
        }
        return super.dispatchKeyEvent(event)
    }

    private fun buildKeyEventPayload(event: KeyEvent): String? {
        val code = KeyEvent.keyCodeToString(event.keyCode)
        val key = resolveKey(event, code)
        if (key.isEmpty() && code.isEmpty()) return null

        return JSONObject().apply {
            put("key", key)
            put("code", code)
            if (event.isAltPressed) put("altKey", true)
            if (event.isCtrlPressed) put("ctrlKey", true)
            if (event.isShiftPressed) put("shiftKey", true)
            if (event.isMetaPressed) put("metaKey", true)
            if (event.repeatCount > 0) put("repeat", true)
        }.toString()
    }

    private fun resolveKey(event: KeyEvent, code: String): String {
        val unicode = event.getUnicodeChar(event.metaState)
        if (unicode != 0) {
            return String(Character.toChars(unicode))
        }

        return when (event.keyCode) {
            KeyEvent.KEYCODE_ENTER -> "Enter"
            KeyEvent.KEYCODE_DEL -> "Backspace"
            KeyEvent.KEYCODE_TAB -> "Tab"
            KeyEvent.KEYCODE_ESCAPE -> "Escape"
            KeyEvent.KEYCODE_BACK -> "Back"
            KeyEvent.KEYCODE_DPAD_LEFT -> "ArrowLeft"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "ArrowRight"
            KeyEvent.KEYCODE_DPAD_UP -> "ArrowUp"
            KeyEvent.KEYCODE_DPAD_DOWN -> "ArrowDown"
            KeyEvent.KEYCODE_SPACE -> " "
            KeyEvent.KEYCODE_MOVE_HOME -> "Home"
            KeyEvent.KEYCODE_MOVE_END -> "End"
            KeyEvent.KEYCODE_PAGE_UP -> "PageUp"
            KeyEvent.KEYCODE_PAGE_DOWN -> "PageDown"
            else -> code.removePrefix("KEYCODE_")
        }
    }

    override fun onPause() {
        super.onPause()
        currentWebView?.pause()
        currentWebView?.let { NativeBridge.notifyPageInactive(it) }
    }

    override fun onStart() {
        super.onStart()

        if (::appId.isInitialized && appId.isNotBlank()) {
            updateCapsuleButtonVisibility(appId)
            if (hasEnteredBackground) {
                NativeApi.onAppShow(appId)
                hasEnteredBackground = false
            }
        }

    }

    override fun onStop() {
        super.onStop()

        // Avoid spurious background/foreground events during configuration changes (e.g. rotation).
        if (isChangingConfigurations) return

        if (::appId.isInitialized && appId.isNotBlank()) {
            NativeApi.onAppHide(appId)
            hasEnteredBackground = true
        }
    }

    /**
     * Notifies the native layer that a mini app is being closed
     * Returns whether the close matches current runtime session.
     */
    private fun notifyLxAppClosed(sessionId: Long = currentSessionId): Boolean {
        return NativeApi.onLxAppClosed(appId, sessionId)
    }

    override fun onDestroy() {
        isDestroyed = true

        // Destroy native components before pausing WebView
        currentWebView?.let { NativeBridge.notifyPageDestroyed(it) }

        // Pause current WebView but don't destroy it
        // WebView destruction is managed by native
        currentWebView?.let { view ->
            view.pause()
        }

        // Clear reference to this activity
        LxApp.setCurrentActivity(null)

        super.onDestroy()
    }

    /**
     * Navigate to any page - super simple
     */
    fun navigate(targetPath: String, animationType: AnimationType): Boolean {
        if (!::appId.isInitialized) return false

        try {
            // Coordinate all UI updates in the same step for consistency
            return coordinatedNavigationUpdate(targetPath, animationType)
        } catch (e: Exception) {
            Log.e(TAG, "Navigation failed: ${e.message}", e)
            return false
        }
    }

    /**
     * Coordinate all UI updates (TabBar, NavBar, WebView) in the same step
     *
     * IMPROVEMENT: Ensures WebView, NavBar, and TabBar updates are synchronized
     * to prevent timing issues and provide smooth, coordinated transitions
     */
    private fun coordinatedNavigationUpdate(targetPath: String, animationType: AnimationType): Boolean {

        val pageConfig = getNavBarState(appId, targetPath)

        applyAnimationTypeUpdates(animationType, targetPath)

        return navigateToPageWithCoordination(targetPath, animationType, pageConfig)
    }

    /**
     * Apply animation type specific UI updates with smooth animations
     */
    private fun applyAnimationTypeUpdates(animationType: AnimationType, targetPath: String) {
        // Reflect visibility from Rust TabBarState only
        val tabBarConfig = NativeApi.getTabBarState(appId)
        val visible = tabBarConfig?.visible ?: false
        showTabBar(visible)
        tabBarConfig?.let {
            tabBar?.setSelectedIndex(it.selectedIndex, notifyListener = false)
        }
    }

    private fun showTabBar(show: Boolean) {
        if (isMediaFullscreen) {
            pendingTabBarVisibility = if (show) View.VISIBLE else View.GONE
            tabBar?.visibility = View.GONE
            return
        }
        tabBar?.visibility = if (show) View.VISIBLE else View.GONE
    }

    fun enterMediaFullscreen() {
        if (isMediaFullscreen) return
        val navBar = navigationBar
        val tab = tabBar
        val navParent = navBar?.parent as? ViewGroup
        val navIndex = navParent?.indexOfChild(navBar) ?: -1
        val overlayHost = rootContainer.findViewWithTag<View>("ComponentOverlay")
        val overlayParams = overlayHost?.layoutParams as? FrameLayout.LayoutParams
        mediaFullscreenState = MediaFullscreenState(
            tabBarVisibility = tab?.visibility ?: View.GONE,
            navigationBarVisibility = navBar?.visibility ?: View.GONE,
            navigationBarIndex = navIndex,
            navigationBarLayoutParams = navBar?.layoutParams,
            overlayLayoutParams = overlayParams?.let { FrameLayout.LayoutParams(it) },
            overlayTranslationX = overlayHost?.translationX ?: 0f,
            overlayTranslationY = overlayHost?.translationY ?: 0f,
            rootPaddingLeft = rootContainer.paddingLeft,
            rootPaddingTop = rootContainer.paddingTop,
            rootPaddingRight = rootContainer.paddingRight,
            rootPaddingBottom = rootContainer.paddingBottom
        )
        pendingTabBarVisibility = null
        pendingNavBarVisibility = null
        isMediaFullscreen = true
        if (overlayHost != null && rootContainer.indexOfChild(overlayHost) != rootContainer.childCount - 1) {
            shouldRestoreOverlayOrder = true
            rootContainer.bringChildToFront(overlayHost)
        }
        if (overlayHost != null) {
            overlayHost.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ).apply {
                leftMargin = 0
                topMargin = 0
                rightMargin = 0
                bottomMargin = 0
            }
            overlayHost.translationX = 0f
            overlayHost.translationY = 0f
            overlayHost.requestLayout()
        }
        if (rootContainer.indexOfChild(webViewContainer) != rootContainer.childCount - 1) {
            shouldRestoreOverlayOrder = true
            rootContainer.bringChildToFront(webViewContainer)
        }
        if (navParent != null && navIndex >= 0) {
            navParent.removeView(navBar)
        } else {
            navBar?.visibility = View.GONE
        }
        navBar?.visibility = View.GONE
        tab?.visibility = View.GONE
        updateCapsuleButton()
        rootContainer.setPadding(0, 0, 0, 0)
        updateLayoutMargins()
        rootContainer.requestApplyInsets()
    }

    fun exitMediaFullscreen() {
        if (!isMediaFullscreen) return
        isMediaFullscreen = false
        mediaFullscreenState?.let { state ->
            val tabVisibility = pendingTabBarVisibility ?: state.tabBarVisibility
            val navVisibility = pendingNavBarVisibility ?: state.navigationBarVisibility
            tabBar?.visibility = tabVisibility
            navigationBar?.visibility = navVisibility
            rootContainer.setPadding(
                state.rootPaddingLeft,
                state.rootPaddingTop,
                state.rootPaddingRight,
                state.rootPaddingBottom
            )
            val navBar = navigationBar
            if (navBar != null && navBar.parent == null && state.navigationBarIndex >= 0) {
                val params = state.navigationBarLayoutParams ?: FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
                rootContainer.addView(navBar, state.navigationBarIndex, params)
                navBar.visibility = navVisibility
            }
            val overlayHost = rootContainer.findViewWithTag<View>("ComponentOverlay")
            if (overlayHost != null) {
                state.overlayLayoutParams?.let { overlayHost.layoutParams = FrameLayout.LayoutParams(it) }
                overlayHost.translationX = state.overlayTranslationX
                overlayHost.translationY = state.overlayTranslationY
                overlayHost.requestLayout()
            }
        }
        mediaFullscreenState = null
        pendingTabBarVisibility = null
        pendingNavBarVisibility = null
        updateCapsuleButton()
        if (shouldRestoreOverlayOrder) {
            navigationBar?.let { rootContainer.bringChildToFront(it) }
            tabBar?.let { rootContainer.bringChildToFront(it) }
            rootContainer.findViewWithTag<View>("capsule_button")?.let { rootContainer.bringChildToFront(it) }
            shouldRestoreOverlayOrder = false
        }
        updateLayoutMargins()
        rootContainer.requestApplyInsets()
        // Ensure WebView top margin is recalculated after NavigationBar has been re-attached/measured.
        rootContainer.post {
            if (isMediaFullscreen) return@post
            updateLayoutMargins()
            rootContainer.requestApplyInsets()
        }
    }

    /**
     * Show WebView with appropriate animation and trigger onPageShow
     */
    /**
     * Navigate to page with coordinated UI updates
     *
     * IMPROVEMENT: Coordinates WebView navigation with navbar updates for smooth transitions
     */
    private fun navigateToPageWithCoordination(
        targetPath: String,
        animationType: AnimationType,
        pageConfig: NavigationBarState?
    ): Boolean {
        // All animation types use coordinated logic
        val success = when (animationType) {
            AnimationType.FORWARD -> {
                navigateToPage(targetPath, pageConfig, isReplace = false, isBackNavigation = false)
                true
            }
            AnimationType.BACKWARD -> {
                navigateToPage(targetPath, pageConfig, isReplace = false, isBackNavigation = true)
                true
            }
            AnimationType.NONE -> {
                // No animation - used for Launch/Replace/SwitchTab semantics
                navigateToPage(targetPath, pageConfig, isReplace = true, isBackNavigation = false)
                true
            }
        }

        return success
    }

    /**
     * Animate old container out with cleanup
     */
    private fun animateOldContainerOut(
        oldContainer: ViewGroup,
        oldWebView: com.lingxia.lxapp.WebView,
        endX: Float,
        duration: Long,
        interpolator: AccelerateDecelerateInterpolator
    ) {
        oldContainer.animate()
            .translationX(endX)
            .setDuration(duration)
            .setInterpolator(interpolator)
            .withEndAction {
                try {
                    // Pause and clean up old WebView
                    oldWebView.pause()
                    oldWebView.visibility = View.GONE

                    // Remove old container from parent
                    (oldContainer.parent as? ViewGroup)?.removeView(oldContainer)

                } catch (e: Exception) {
                    Log.e(TAG, "Error cleaning up old container: ${e.message}")
                }
            }
            .start()
    }

    /**
     * Perform WebView transition animation with synchronized navbar animation
     *
     * Extracted from navigateToPage for reuse in coordinated navigation
     */
    private fun performWebViewTransition(oldWebView: WebView?, newContainer: FrameLayout, isBackNavigation: Boolean, shouldAnimate: Boolean = true, navbarState: NavigationBarState? = null) {
        // Get reference to old container BEFORE adding new one
        val oldContainer = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
        oldContainer?.tag = "previous_webview_container" // Re-tag old container

        try {
            // Add the new container to the webview container
            webViewContainer.addView(newContainer)
        } catch (e: Exception) {
            Log.e(TAG, "Error adding new container to webViewContainer: ${e.message}")
            return
        }

        if (shouldAnimate) {
            // Set up animation based on navigation direction
            val slideInTranslation = if (isBackNavigation) -webViewContainer.width.toFloat() else webViewContainer.width.toFloat()
            val slideOutTranslation = if (isBackNavigation) webViewContainer.width.toFloat() else -webViewContainer.width.toFloat()

            // Set initial position for new container
            newContainer.translationX = slideInTranslation

            // Animate the transition
            val animationDuration = 300L

            // Animate navbar and webview together
            if (navbarState != null && navigationBar != null) {
                animateNavBar(navbarState, isBackNavigation)
            }

            // Update layout margins after navbar state change
            updateLayoutMargins()

            // Animate new container sliding in - SAME TIME AS NAVBAR
            newContainer.animate()
                .translationX(0f)
                .setDuration(animationDuration)
                .setInterpolator(android.view.animation.DecelerateInterpolator())
                .withEndAction {
                    // Trigger onPageShow after animation completes
                    triggerOnPageShow(newContainer)
                }
                .start()

            // Animate old container sliding out (if exists)
            oldContainer?.let { container ->
                container.animate()
                    .translationX(slideOutTranslation)
                    .setDuration(animationDuration)
                    .setInterpolator(android.view.animation.AccelerateInterpolator())
                    .withEndAction {
                        cleanupOldContainer(container)
                    }
                    .start()
            }
        } else {
            // No animation - update navbar immediately
            if (navbarState != null && navigationBar != null) {
                updateNavBar(navbarState)
            }

            // Update layout margins after navbar state change
            updateLayoutMargins()

            newContainer.translationX = 0f

            // Clean up old container immediately
            oldContainer?.let { container ->
                cleanupOldContainer(container)
            }

            // Trigger onPageShow immediately
            triggerOnPageShow(newContainer)
        }
    }

    /**
     * Trigger onPageShow for WebView container
     *
     * IMPORTANT: Post to next frame to ensure all UI components (including pullToRefreshHelper)
     * are fully initialized before JS onShow is triggered
     */
    private fun triggerOnPageShow(container: FrameLayout) {
        container.post {
            try {
                val webView = container.getChildAt(0) as? WebView
                if (webView?.getAppId() != null && webView.getCurrentPath() != null) {
                    val pagePath = webView.getCurrentPath()!!
                    NativeApi.onPageShow(webView.getAppId()!!, pagePath)
                    applyPageOrientation(pagePath)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to call nativeOnPageShow in performWebViewTransition: ${e.message}")
            }
        }
    }

    /**
     * Clean up old container after animation
     */
    private fun cleanupOldContainer(container: ViewGroup) {
        try {
            webViewContainer.removeView(container)
        } catch (e: Exception) {
            Log.e(TAG, "Error cleaning up old container: ${e.message}")
        }
    }

    /**
     * Navigate to a page with coordinated navbar and webview updates
     *
     * IMPROVEMENT: Coordinates navbar and webview updates in the same step
     * @param targetPath Path of the page to navigate to
     * @param pageConfig Navigation bar configuration (optional, will be fetched if null)
     * @param isReplace Whether this is a replace navigation
     * @param isBackNavigation Whether this is a back navigation
     */
    private fun navigateToPage(
        targetPath: String,
        pageConfig: NavigationBarState? = null,
        isReplace: Boolean = false,
        isBackNavigation: Boolean = false
    ) {
        try {
            // Get current WebView before changes
            val oldWebView = currentWebView

            // Find WebView for the target page
            val newWebView = findWebView(appId, targetPath)
            if (newWebView == null) {
                Log.e(TAG, "Failed to find WebView for path: $targetPath")
                return
            }

            if (oldWebView != null && oldWebView != newWebView) {
                NativeBridge.notifyPageInactive(oldWebView)
            }

            val navbarState = pageConfig ?: getNavBarState(appId, targetPath)

            // Continue with webview setup...
            if (newWebView.parent != null) {
                (newWebView.parent as? ViewGroup)?.removeView(newWebView)
            }

            // IMPORTANT: Make sure the new WebView is fully prepared before animation
            newWebView.visibility = View.VISIBLE
            newWebView.resume()

            // Create a new container for the WebView
            val newContainer = FrameLayout(this).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                tag = "current_webview_container"

                try {
                    addView(newWebView)
                } catch (e: Exception) {
                    Log.e(TAG, "Error adding WebView to container: ${e.message}")
                    return@apply
                }
            }

            NativeBridge.attachIfNeeded(newWebView)

            ensurePullToRefreshHelper().attachToWebView(newWebView)
            updatePullToRefreshEnabledForPath(targetPath)

            if (oldWebView != newWebView) {
                NativeBridge.notifyPageActive(newWebView)
            }

            // Use coordinated WebView transition (handles all animation and onPageShow)
            // Only animate for forward/backward navigation, not for replace operations (tab switch, launch, replace)
            val shouldAnimate = !isReplace
            performWebViewTransition(oldWebView, newContainer, isBackNavigation, shouldAnimate, navbarState)

            // Update the current WebView reference
            currentWebView = newWebView

        } catch (e: Exception) {
            Log.e(TAG, "Error in coordinated navigation: ${e.message}", e)
        }
    }

    private fun animateNavBar(navbarState: NavigationBarState, isBackNavigation: Boolean) {
        if (!navbarState.showNavbar) {
            navigationBar?.visibility = View.GONE
            if (isMediaFullscreen) {
                pendingNavBarVisibility = View.GONE
            }
            return
        }

        navigationBar?.apply {
            visibility = View.VISIBLE
            translationX = if (isBackNavigation) -width.toFloat() else width.toFloat()

            configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = false
            )
        }
        if (isMediaFullscreen) {
            pendingNavBarVisibility = View.VISIBLE
            navigationBar?.visibility = View.GONE
        }
    }

    private fun updateNavBar(navbarState: NavigationBarState) {
        if (!navbarState.showNavbar) {
            navigationBar?.visibility = View.GONE
            if (isMediaFullscreen) {
                pendingNavBarVisibility = View.GONE
            }
            return
        }

        navigationBar?.apply {
            visibility = View.VISIBLE
            configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = true
            )
        }
        if (isMediaFullscreen) {
            pendingNavBarVisibility = View.VISIBLE
            navigationBar?.visibility = View.GONE
        }
    }

    private fun createNavBar() {
        if (navigationBar != null) return

        try {
            val statusBarHeight = getStatusBarHeight(this)
            navigationBar = NavigationBar(this).apply {
                val navBarContentHeight = getCalculatedContentHeightPx()
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    navBarContentHeight + statusBarHeight  // Include status bar height
                ).apply {
                    gravity = Gravity.TOP
                    topMargin = 0  // Start from very top
                }

                setPadding(paddingLeft, statusBarHeight, paddingRight, paddingBottom)  // Add status bar padding
                setOnBackButtonClickListener { handleBackButtonClick() }
                setOnHomeButtonClickListener { handleHomeButtonClick() }
                visibility = View.GONE
            }

            rootContainer.addView(navigationBar)
        } catch (e: Exception) {
            navigationBar = null
        }
    }

    private fun updateNavigationBar(config: NavigationBarState?, isBackNavigation: Boolean, disableAnimation: Boolean = false, targetPath: String? = null) {
        val pathForNavbar = targetPath ?: currentWebView?.getCurrentPath() ?: ""
        val navbarState = config ?: NativeApi.getNavigationBarState(appId, pathForNavbar)

        if (navbarState != null) {
            // Create navbar if needed
            if (navigationBar == null) {
                createNavBar()
            }

            navigationBar?.configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = disableAnimation
            )
            if (isMediaFullscreen) {
                val shouldShow = navbarState.showNavbar ||
                    navbarState.showBackButton ||
                    navbarState.showHomeButton
                pendingNavBarVisibility = if (shouldShow) View.VISIBLE else View.GONE
                navigationBar?.visibility = View.GONE
            }
        } else {
            // Hide navbar completely
            navigationBar?.visibility = View.GONE
            if (isMediaFullscreen) {
                pendingNavBarVisibility = View.GONE
            }
        }

        updateLayoutMargins()
    }

    /**
     * Handles the click event from the NavigationBar's back button.
     */
    private fun handleBackButtonClick() {
        NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_NAVIGATION_CLICK, NativeApi.NAVIGATION_ACTION_BACK)
    }

    /**
     * Handles the click event from the NavigationBar's home button.
     */
    private fun handleHomeButtonClick() {
        NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_NAVIGATION_CLICK, NativeApi.NAVIGATION_ACTION_HOME)
    }

    /**
     * Apply page orientation configuration
     */
    private fun applyPageOrientation(path: String) {
        val normalizedPath = normalizePath(path)
        val orientation = NativeApi.getPageOrientation(appId, normalizedPath)

        updateRequestedOrientation(orientation)
        updateImmersiveMode(orientation, normalizedPath)
        dispatchUiOrientationChangeIfNeeded()
    }

    private fun updateRequestedOrientation(orientation: Int) {
        val targetOrientation = resolveScreenOrientation(orientation)
        if (requestedOrientation != targetOrientation) {
            requestedOrientation = targetOrientation
        }
    }

    private fun resolveScreenOrientation(orientation: Int): Int {
        return when (orientation) {
            NativeApi.ORIENTATION_PORTRAIT -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            NativeApi.ORIENTATION_LANDSCAPE -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            NativeApi.ORIENTATION_REVERSE_PORTRAIT -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
            NativeApi.ORIENTATION_REVERSE_LANDSCAPE -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
            NativeApi.ORIENTATION_AUTO -> ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            else -> ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    private fun updateImmersiveMode(orientation: Int, path: String) {
        val shouldFullscreen = shouldEnterImmersiveMode(orientation, path)
        if (shouldFullscreen) {
            enterImmersiveMode()
        } else {
            exitImmersiveMode()
        }
    }

    private fun shouldEnterImmersiveMode(orientation: Int, path: String): Boolean {
        if (
            orientation != NativeApi.ORIENTATION_LANDSCAPE &&
            orientation != NativeApi.ORIENTATION_REVERSE_LANDSCAPE
        ) {
            return false
        }

        val navbarState = NativeApi.getNavigationBarState(appId, path)
        return navbarState != null && !navbarState.showNavbar
    }

    /**
     * Enter immersive fullscreen mode (hide status bar and navigation bar)
     */
    private fun enterImmersiveMode() {
        if (isPageFullscreen) {
            return
        }
        isPageFullscreen = true

        // Allow content to extend into display cutout (notch/punch hole) area
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }

        @Suppress("DEPRECATION")
        window.setFlags(
            android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN,
            android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN
        )
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        controller?.apply {
            hide(WindowInsetsCompat.Type.statusBars())
            hide(WindowInsetsCompat.Type.navigationBars())
            hide(WindowInsetsCompat.Type.displayCutout())
            systemBarsBehavior = androidx.core.view.WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        // Trigger layout update
        rootContainer.setPadding(0, 0, 0, 0)
        rootContainer.requestApplyInsets()
    }

    /**
     * Exit immersive mode (show status bar and navigation bar)
     */
    private fun exitImmersiveMode() {
        if (!isPageFullscreen) {
            return
        }
        isPageFullscreen = false

        // Restore default cutout mode
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_DEFAULT
        }

        @Suppress("DEPRECATION")
        window.clearFlags(android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        controller?.apply {
            show(WindowInsetsCompat.Type.statusBars())
            show(WindowInsetsCompat.Type.navigationBars())
        }
        // Trigger layout update
        rootContainer.requestApplyInsets()
    }

    // Helper to calculate the Y translation based on visible bars
    private fun calculateWebViewTranslationY(): Float {
        // Since topMargin in updateLayoutMargins() already handles NavigationBar positioning,
        // and we no longer support TOP positioned TabBars, no additional offset is needed
        val tabBarOffset = 0
        // Return only TabBar offset, NavigationBar is handled by topMargin
        return tabBarOffset.toFloat()
    }

    // Close the current LxApp
    fun closeLxApp(sessionId: Long = currentSessionId) {
        // Notify native layer first
        if (!notifyLxAppClosed(sessionId)) {
            Log.w(TAG, "Ignoring stale close callback for appId=$appId sessionId=$sessionId")
            return
        }

        // Pause and clean up current WebView
        currentWebView?.let { webView ->
            webView.pause()
            webView.visibility = View.GONE
        }
        webViewContainer.removeAllViews()
        currentWebView = null

        // Hide tab bar with animation (capsule and navbar remain)
        showTabBar(false)

        // Clear current app state
        appId = ""

        // Get next LxApp from Rust stack and open it
        val currentLxApp = NativeApi.getCurrentLxApp()
        if (currentLxApp != null && currentLxApp.isValid()) {
            openLxApp(currentLxApp.appId, currentLxApp.path, currentLxApp.sessionId)
        } else {
        }
    }

    // Switch to a different LxApp in the current activity
    fun openLxApp(appId: String, path: String, sessionId: Long) {
        if (sessionId <= 0L) {
            Log.e(TAG, "Refusing to open app without valid sessionId: appId=$appId")
            return
        }

        // Ensure all UI operations are on the main thread
        runOnUiThread {
            // Update app state (no intent extras needed - we're not switching activities)
            this.appId = appId
            this.currentSessionId = sessionId

            // 1. Necessary preparation (build tabbar, etc.)
            prepareLxApp(appId)

            // 2. Check whether to show capsule button (home=hide, others=show)
            updateCapsuleButtonVisibility(appId)

            // 3. Call navigate as entry point
            if (path.isNotEmpty()) {
                navigate(path, AnimationType.NONE)
            } else {
                Log.e(TAG, "No valid path to navigate to")
            }
        }
    }

    /**
     *  Necessary preparation for LxApp (build tabbar, etc.)
     */
    private fun prepareLxApp(appId: String) {
        // Pause current WebView
        currentWebView?.let { webView ->
            webView.pause()
            webView.visibility = View.GONE
        }

        // Clear WebView container for new app
        webViewContainer.removeAllViews()
        currentWebView = null
        pullToRefreshHelper?.setEnabled(false)

        // Build tab bar configuration for new app (tabbar is dynamic)
        val tabBarConfig = NativeApi.getTabBarState(appId)
        if (tabBarConfig != null) {
            tabBar?.setConfig(tabBarConfig)
            // Reflect visibility from Rust state, not inferred by presence
            showTabBar(tabBarConfig.visible)
        } else {
            showTabBar(false)
        }
    }

    /**
     * Check whether to show capsule button (home=hide, others=show)
     */
    private fun updateCapsuleButtonVisibility(appId: String) {
        if (!shouldShowCapsuleButton(appId, currentSessionId)) {
            val capsuleButton = rootContainer.findViewWithTag<View>("capsule_button")
            capsuleButton?.visibility = View.GONE
        } else {
            updateCapsuleButton()
        }
    }

    // Update capsule button visibility
    private fun updateCapsuleButton() {
        rootContainer.post {
            val capsule = rootContainer.findViewWithTag<View>("capsule_button")
            if (!shouldShowCapsuleButton(appId, currentSessionId)) {
                capsule?.visibility = View.GONE
            } else {
                if (capsule == null) {
                    addCapsuleButton()
                } else {
                    capsule.visibility = View.VISIBLE
                }
            }
        }
    }

    // Get current app ID
    fun getAppId(): String = appId
    fun getSessionId(): Long = currentSessionId

    // Get current WebView (internal access for LxApp)
    internal fun getCurrentWebView(): com.lingxia.lxapp.WebView? = currentWebView

    // Handle configuration changes to prevent Activity recreation
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)

        // Update layout to adapt to screen orientation changes
        if (::webViewContainer.isInitialized) {
            updateLayoutMargins()
        }
        dispatchUiOrientationChangeIfNeeded()
    }

    private fun currentUiOrientationLabel(): String? {
        return when (resources.configuration.orientation) {
            android.content.res.Configuration.ORIENTATION_LANDSCAPE -> "landscape"
            android.content.res.Configuration.ORIENTATION_PORTRAIT -> "portrait"
            else -> null
        }
    }

    private fun dispatchUiOrientationChangeIfNeeded() {
        val orientationValue = currentUiOrientationLabel() ?: return
        if (lastDispatchedDeviceOrientation == orientationValue) {
            return
        }
        if (!::appId.isInitialized || appId.isBlank()) {
            return
        }
        val sessionId = currentSessionId
        if (sessionId <= 0L) {
            return
        }

        try {
            if (NativeApi.onDeviceOrientationChanged(appId, sessionId, orientationValue)) {
                lastDispatchedDeviceOrientation = orientationValue
            }
        } catch (error: Throwable) {
            Log.w(TAG, "onDeviceOrientationChanged failed: ${error.message}")
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        if (PermissionManager.handleRequestPermissionsResult(requestCode, permissions, grantResults)) {
            return
        }
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    }
}
