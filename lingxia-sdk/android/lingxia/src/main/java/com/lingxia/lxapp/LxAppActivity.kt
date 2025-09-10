package com.lingxia.lxapp

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout

import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import android.content.Intent
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import android.view.animation.AccelerateDecelerateInterpolator

/**
 * Navigation type enum for LxApp navigation
 */
enum class NavigationType(val value: Int) {
    /**
     * Launch navigation - for openLxApp to open entry page
     */
    LAUNCH(0),

    /**
     * Forward navigation - navigate to a new page with animation
     */
    FORWARD(1),

    /**
     * Backward navigation - navigate back with animation
     */
    BACKWARD(2),

    /**
     * Replace navigation - replace current page without animation
     */
    REPLACE(3),

    /**
     * Switch tab navigation - switch between tab pages
     */
    SWITCH_TAB(4);

    companion object {
        /**
         * Convert NavigationType to string for logging
         */
        fun toString(type: NavigationType): String {
            return when (type) {
                LAUNCH -> "Launch"
                FORWARD -> "Forward"
                BACKWARD -> "Backward"
                REPLACE -> "Replace"
                SWITCH_TAB -> "SwitchTab"
            }
        }

        /**
         * Convert integer to NavigationType
         */
        fun fromInt(value: Int): NavigationType {
            return when (value) {
                0 -> LAUNCH
                1 -> FORWARD
                2 -> BACKWARD
                3 -> REPLACE
                4 -> SWITCH_TAB
                else -> FORWARD // Default fallback
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
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"
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
                            activity.tabBar?.setConfig(newTabBarConfig)

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

        // Helper function to get status bar height
        fun getStatusBarHeight(context: Context): Int {
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
                    isNavigationBarContrastEnforced = false
                    navigationBarDividerColor = Color.TRANSPARENT
                } else {
                    // TabBar is not transparent, use TabBar's background color for navigation bar
                    clearFlags(android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS)

                    // Use TabBar's background color, fallback to white if not provided
                    val navBarColor = tabBarBackgroundColor ?: Color.WHITE
                    navigationBarColor = navBarColor

                    // Set contrast enforcement based on color brightness
                    val brightness = (Color.red(navBarColor) * 0.299 + Color.green(navBarColor) * 0.587 + Color.blue(navBarColor) * 0.114)
                    isNavigationBarContrastEnforced = brightness > 128 // Light background

                    // Remove divider completely for seamless appearance
                    navigationBarDividerColor = Color.TRANSPARENT
                }
            }
        }
    }

    private lateinit var appId: String
    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    private var tabBar: TabBar? = null
    private var navigationBar: NavigationBar? = null
    private var independentNavigationButton: NavigationButton? = null
    private var isDestroyed = false
    private var pendingWebViewSetup = false
    private var isDisplayingHomeLxApp: Boolean = false

    // Tracks the currently visible WebView instance
    private var currentWebView: com.lingxia.lxapp.WebView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Set reference to this activity in LxApp
        LxApp.setCurrentActivity(this)

        // Initialize appId from intent FIRST (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            Log.e(TAG, "Missing required parameter: appId")
            finish()
            return
        }
        val initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""

        // Initialize the new flag
        isDisplayingHomeLxApp = (appId == LxApp.HomeLxAppId)

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

        // Create independent navigation button (for when navbar is hidden but button is needed)
        createIndependentNavigationButton()

        // Defer capsule button creation to post-layout
        rootContainer.post {
            addCapsuleButton()
        }

        // Setup window insets listener
        ViewCompat.setOnApplyWindowInsetsListener(rootContainer) { view, insets ->
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val tabBarBgColor = tabBarConfig?.backgroundColor
            val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                     (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

            if (isTabBarTransparent) {
                view.setPadding(0, 0, 0, 0)
            } else {
                view.setPadding(systemBars.left, 0, systemBars.right, systemBars.bottom)
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
            } else {
                width = ViewGroup.LayoutParams.MATCH_PARENT
                height = tabBarSizePx
                gravity = Gravity.BOTTOM

                // No bottom margin for transparent TabBar - it should overlay content
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
                }
            } else {
                FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, tabBarSizePx).apply {
                    gravity = Gravity.BOTTOM
                    // No bottom margin for transparent TabBar - it should overlay content
                }
            }
            tabBar.layoutParams = newLayoutParams
        }
    }

    private fun updateLayoutMargins() {
        val isTabBarVisible = tabBar?.visibility == View.VISIBLE
        val tabBarHeight = tabBar?.layoutParams?.height ?: 0
        val tabBarWidth = tabBar?.layoutParams?.width ?: 0
        val tabBarBgColor = tabBar?.config?.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        // Calculate NavigationBar height for webview margin
        val isNavBarVisible = navigationBar?.visibility == View.VISIBLE
        val navBarHeight = if (isNavBarVisible) {
            // WebView should start right after navbar (navbar total height)
            navigationBar?.layoutParams?.height ?: 0
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

    /**
     * Update status bar to match navbar background color
     */
    private fun updateStatusBarForNavbar(navbarBgColor: Int) {
        // Set status bar background to match navbar
        window.statusBarColor = navbarBgColor

        // Set status bar text color based on navbar background brightness
        val isNavbarDark = NavigationBar.ColorUtils.isColorDark(navbarBgColor)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = !isNavbarDark  // Light text on dark bg, dark text on light bg
        }
    }

    // Find WebView - ONLY find WebView, nothing else
    private fun findWebView(appId: String, path: String): com.lingxia.lxapp.WebView? {
        val webView = com.lingxia.lxapp.WebView.findWebView(appId, path)
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

            // Add to webview container if not already added
            if (view.parent != webViewContainer) {
                // Remove from existing parent if it has one
                (view.parent as? ViewGroup)?.removeView(view)

                // Set proper layout parameters before adding
                view.layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )

                // Add to webview container
                webViewContainer.addView(view)
            }

            // Resume the WebView's activities
            view.resume()
        } else {
            Log.w(TAG, "attachWebViewToUI: Activity is destroyed, skipping WebView attachment")
        }
    }

    private fun setupWebViewContent(appId: String, path: String) {
        val initialWebView = findWebView(appId, path)
        if (initialWebView == null) {
            Log.e(TAG, "Failed to find or create initial WebView for $path")
            closeLxApp(); return
        }
        setupWebViewContentWithExisting(initialWebView)
    }

    // New method to setup WebView content with an existing WebView
    private fun setupWebViewContentWithExisting(webView: com.lingxia.lxapp.WebView) {
        // Set the current WebView first
        this.currentWebView = webView

        // Attach and resume immediately
        attachWebViewToUI(webView)

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
            setBackgroundColor(Color.TRANSPARENT)
        }
        if (webViewContainer.parent == null) {
            rootContainer.addView(webViewContainer)
        }
    }

    private fun addCapsuleButton() {
        // Don't show capsule button for the main/home app
        if (isDisplayingHomeLxApp) {
            return
        }

        val statusBarHeight = getStatusBarHeight(this)

        // Create capsule container
        val capsule = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            tag = "capsule_button" // Add tag to find it later
            elevation = 1000f // Ensure it stays on top of other views

            // Set capsule background
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                setColor(Color.WHITE)
                cornerRadius = 16f * resources.displayMetrics.density // Half of height (32/2) for perfect rounded corners
                setStroke((0.5f * resources.displayMetrics.density).toInt(), 0xFFDDDDDD.toInt())
            }

            // Capsule layout parameters - Position fixed relative to status bar
            val capsuleLayoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                (32 * resources.displayMetrics.density).toInt()
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                // Position with fixed offset relative to status bar (moved up 4dp to avoid overlap with navbar)
                topMargin = statusBarHeight + (4 * resources.displayMetrics.density).toInt()
                rightMargin = (12 * resources.displayMetrics.density).toInt()
            }
            layoutParams = capsuleLayoutParams

            setPadding(
                (2 * resources.displayMetrics.density).toInt(),
                0,
                (2 * resources.displayMetrics.density).toInt(),
                0
            )
        }

        // Create more button with custom dots drawable
        val btnMore = ImageButton(this).apply {
            setBackgroundColor(Color.TRANSPARENT)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageDrawable(LxAppDrawables.createMoreDots())
            layoutParams = LinearLayout.LayoutParams(
                (44 * resources.displayMetrics.density).toInt(),
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setOnClickListener {
                NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_CAPSULE_CLICK, NativeApi.CAPSULE_ACTION_MORE)
            }
        }

        // Create divider
        val divider = View(this).apply {
            setBackgroundColor(0xFFDDDDDD.toInt())
            layoutParams = LinearLayout.LayoutParams(
                (0.5f * resources.displayMetrics.density).toInt(),
                (18 * resources.displayMetrics.density).toInt()
            ).apply {
                gravity = Gravity.CENTER_VERTICAL
                marginStart = (2 * resources.displayMetrics.density).toInt()
                marginEnd = (2 * resources.displayMetrics.density).toInt()
            }
        }

        // Create close button with custom circle drawable
        val btnClose = ImageButton(this).apply {
            setBackgroundColor(Color.TRANSPARENT)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageDrawable(LxAppDrawables.createCloseButton(resources))
            layoutParams = LinearLayout.LayoutParams(
                (44 * resources.displayMetrics.density).toInt(),
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setOnClickListener {
                NativeApi.onUiEvent(appId, NativeApi.UI_EVENT_CAPSULE_CLICK, NativeApi.CAPSULE_ACTION_CLOSE)
            }
        }

        // Add views to capsule
        capsule.addView(btnMore)
        capsule.addView(divider)
        capsule.addView(btnClose)

        // Add capsule to root container at the end to ensure it's on top
        rootContainer.post {
            // Ensure we don't add multiple capsules if this runs multiple times
            rootContainer.removeView(rootContainer.findViewWithTag("capsule_button"))
            rootContainer.addView(capsule)
        }
    }

    override fun onResume() {
        super.onResume()
        if (!pendingWebViewSetup) {

            currentWebView?.visibility = View.VISIBLE // Ensure visibility
            webViewContainer.visibility = View.VISIBLE
            currentWebView?.resume()
        }
    }

    override fun onPause() {
        super.onPause()
        currentWebView?.pause()
    }

    /**
     * Notifies the native layer that a mini app is being closed
     * Used only for state synchronization, doesn't affect closure decision
     */
    private fun notifyLxAppClosed() {
        NativeApi.onLxAppClosed(appId)
    }

    override fun onDestroy() {
        isDestroyed = true

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
    fun navigate(targetPath: String, navigationType: NavigationType): Boolean {
        if (!::appId.isInitialized) return false

        try {
            // Resolve actual navigation type (like macOS)
            val actualType = resolveNavigationType(navigationType, targetPath)

            // Coordinate all UI updates in the same step for consistency
            return coordinatedNavigationUpdate(targetPath, actualType)
        } catch (e: Exception) {
            Log.e(TAG, "Navigation failed: ${e.message}", e)
            return false
        }
    }

    /**
     * Resolve navigation type based on path (like macOS logic)
     */
    private fun resolveNavigationType(navigationType: NavigationType, targetPath: String): NavigationType {
        return when (navigationType) {
            NavigationType.LAUNCH -> {
                // Launch: convert to tab switch if it's a tab page
                if (isTabPage(targetPath)) NavigationType.SWITCH_TAB else NavigationType.LAUNCH
            }
            else -> navigationType
        }
    }

    /**
     * Coordinate all UI updates (TabBar, NavBar, WebView) in the same step
     *
     * IMPROVEMENT: Ensures WebView, NavBar, and TabBar updates are synchronized
     * to prevent timing issues and provide smooth, coordinated transitions
     */
    private fun coordinatedNavigationUpdate(targetPath: String, navigationType: NavigationType): Boolean {

        val pageConfig = getNavBarState(appId, targetPath)

        applyNavigationTypeUpdates(navigationType, targetPath)

        return navigateToPageWithCoordination(targetPath, navigationType, pageConfig)
    }

    /**
     * Apply navigation type specific UI updates with smooth animations
     */
    private fun applyNavigationTypeUpdates(navigationType: NavigationType, targetPath: String) {
        // Reflect visibility from Rust TabBarState only
        val tabBarConfig = NativeApi.getTabBarState(appId)
        val visible = tabBarConfig?.visible ?: false
        showTabBar(visible)
        if (visible && tabBarConfig != null) {
            tabBar?.setSelectedIndex(tabBarConfig.selectedIndex, notifyListener = false)
        }
    }

    private fun showTabBar(show: Boolean) {
        tabBar?.visibility = if (show) View.VISIBLE else View.GONE
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
        navigationType: NavigationType,
        pageConfig: NavigationBarState?
    ): Boolean {


        // All navigation types use coordinated logic
        val success = when (navigationType) {
            NavigationType.SWITCH_TAB -> {
                // Tab switch = launch without animation (like macOS)
                navigateToPage(targetPath, pageConfig, isReplace = true, isBackNavigation = false)
                true
            }
            NavigationType.FORWARD -> {
                navigateToPage(targetPath, pageConfig, isReplace = false, isBackNavigation = false)
                true
            }
            NavigationType.BACKWARD -> {
                navigateToPage(targetPath, pageConfig, isReplace = false, isBackNavigation = true)
                true
            }
            NavigationType.LAUNCH, NavigationType.REPLACE -> {
                navigateToPage(targetPath, pageConfig, isReplace = true, isBackNavigation = false)
                true
            }
        }

        return success
    }

    /**
     * Check if path is a tab page
     */
    private fun isTabPage(targetPath: String): Boolean {
        return tabBar?.findTabIndexByPath(targetPath)?.let { it >= 0 } ?: false
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
     */
    private fun triggerOnPageShow(container: FrameLayout) {
        try {
            val webView = container.getChildAt(0) as? WebView
            if (webView?.getAppId() != null && webView.getCurrentPath() != null) {
                NativeApi.onPageShow(webView.getAppId()!!, webView.getCurrentPath()!!)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to call nativeOnPageShow in performWebViewTransition: ${e.message}")
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

            // Get navbar state (use provided or fetch)
            val actualPageConfig = pageConfig ?: getNavBarState(appId, targetPath)

            // Get navbar state for synchronized animation
            val navbarState = NativeApi.getNavigationBarState(appId, targetPath)

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
            // IMPORTANT: Animate independent home button when navbar is hidden
            animateIndependentNavigationButton(navbarState, isBackNavigation)
            return
        }

        // IMPORTANT: Hide independent navigation button to avoid duplication
        independentNavigationButton?.visibility = View.GONE

        navigationBar?.apply {
            visibility = View.VISIBLE
            translationX = if (isBackNavigation) -width.toFloat() else width.toFloat()

            val textColor = NavigationBar.ColorUtils.resolveNavTextColor(navbarState)

            updateStateAndAnimate(
                title = navbarState.navigationBarTitleText,
                bgColor = navbarState.navigationBarBackgroundColor,
                textColor = textColor,
                showBackButton = navbarState.showBackButton,
                showHomeButton = navbarState.showHomeButton,
                isBackNavigation = isBackNavigation,
                disableAnimation = false,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() }
            )

            // Update status bar to match navbar
            updateStatusBarForNavbar(navbarState.navigationBarBackgroundColor)
        }
    }

    /**
     * Handle independent navigation button when navbar is hidden
     */
    private fun animateIndependentNavigationButton(navbarState: NavigationBarState, isBackNavigation: Boolean) {
        // Hide NavigationBar's buttons to avoid duplication
        navigationBar?.apply {
            setHomeButtonVisible(false)
            setBackButtonVisible(false)
        }

        // Update button state and animate if visible
        updateIndependentNavigationButton(navbarState)

        independentNavigationButton?.takeIf { it.visibility == View.VISIBLE }?.let { button ->
            val slideInTranslation = if (isBackNavigation) -button.width.toFloat() else button.width.toFloat()
            button.translationX = slideInTranslation
            button.alpha = 0f

            button.animate()
                .translationX(0f)
                .alpha(1f)
                .setDuration(LxAppDrawables.Constants.ANIMATION_DURATION_MS)
                .setInterpolator(android.view.animation.DecelerateInterpolator())
                .start()
        }
    }

    private fun updateNavBar(navbarState: NavigationBarState) {
        if (!navbarState.showNavbar) {
            navigationBar?.visibility = View.GONE
            updateIndependentNavigationButton(navbarState)
            return
        }

        navigationBar?.apply {
            visibility = View.VISIBLE
            val textColor = NavigationBar.ColorUtils.resolveNavTextColor(navbarState)

            updateStateAndAnimate(
                title = navbarState.navigationBarTitleText,
                bgColor = navbarState.navigationBarBackgroundColor,
                textColor = textColor,
                showBackButton = navbarState.showBackButton,
                showHomeButton = navbarState.showHomeButton,
                isBackNavigation = false,
                disableAnimation = true,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() }
            )

            // Update status bar to match navbar
            updateStatusBarForNavbar(navbarState.navigationBarBackgroundColor)
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
        val navbarState = NativeApi.getNavigationBarState(appId, pathForNavbar)

        if (navbarState?.showNavbar == true) {
            // Create navbar if needed
            if (navigationBar == null) {
                createNavBar()
            }

            val textColor = when (navbarState.navigationBarTextStyle.lowercase()) {
                "white" -> Color.WHITE
                "black" -> Color.BLACK
                else -> if (NavigationBar.ColorUtils.isColorDark(navbarState.navigationBarBackgroundColor)) Color.WHITE else Color.BLACK
            }

            navigationBar?.updateStateAndAnimate(
                title = navbarState.navigationBarTitleText,
                bgColor = navbarState.navigationBarBackgroundColor,
                textColor = textColor,
                showBackButton = navbarState.showBackButton,
                showHomeButton = navbarState.showHomeButton,
                isBackNavigation = isBackNavigation,
                disableAnimation = disableAnimation,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() }
            )

            // Update status bar to match navbar
            updateStatusBarForNavbar(navbarState.navigationBarBackgroundColor)
        } else {
            // Hide navbar completely
            navigationBar?.visibility = View.GONE

            // Reset status bar to transparent when navbar is hidden
            window.statusBarColor = Color.TRANSPARENT
            WindowCompat.getInsetsController(window, window.decorView).apply {
                isAppearanceLightStatusBars = true  // Default to light status bar
            }
        }

        updateLayoutMargins()
        updateIndependentNavigationButton(navbarState)
    }

    private fun updateIndependentNavigationButton(navbarState: NavigationBarState?) {
        val shouldShow = navbarState != null && !navbarState.showNavbar &&
                        (navbarState.showBackButton || navbarState.showHomeButton)

        if (shouldShow) {
            if (independentNavigationButton == null) createIndependentNavigationButton()

            independentNavigationButton?.apply {
                visibility = View.VISIBLE
                val isBackButton = navbarState!!.showBackButton
                setButtonType(if (isBackButton) NavigationButton.ButtonType.BACK else NavigationButton.ButtonType.HOME)
                setOnButtonClickListener(if (isBackButton) { -> handleBackButtonClick() } else { -> handleHomeButtonClick() })
                setButtonColor(NavigationBar.ColorUtils.resolveNavTextColor(navbarState))
            }
        } else {
            independentNavigationButton?.visibility = View.GONE
        }
    }

    private fun createIndependentNavigationButton() {
        if (independentNavigationButton != null) return

        val density = resources.displayMetrics.density
        independentNavigationButton = NavigationButton(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                (LxAppDrawables.Constants.BUTTON_SIZE_DP * density).toInt(),
                (LxAppDrawables.Constants.BUTTON_SIZE_DP * density).toInt()
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                topMargin = getStatusBarHeight(this@LxAppActivity) + (4 * density).toInt()
                marginStart = (LxAppDrawables.Constants.MARGIN_START_DP * density).toInt()
            }
            elevation = 1000f
            visibility = View.GONE
        }
        rootContainer.addView(independentNavigationButton)
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

    // Helper to calculate the Y translation based on visible bars
    private fun calculateWebViewTranslationY(): Float {
        // Since topMargin in updateLayoutMargins() already handles NavigationBar positioning,
        // and we no longer support TOP positioned TabBars, no additional offset is needed
        val tabBarOffset = 0
        // Return only TabBar offset, NavigationBar is handled by topMargin
        return tabBarOffset.toFloat()
    }

    // Close the current LxApp
    fun closeLxApp() {
        // Notify native layer first
        notifyLxAppClosed()

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
        isDisplayingHomeLxApp = false

        // Get next LxApp from Rust stack and open it
        val currentLxApp = NativeApi.getCurrentLxApp()
        if (currentLxApp != null && currentLxApp.isValid()) {
            Log.i(TAG, "Opening next LxApp from stack: ${currentLxApp.appId}:${currentLxApp.path}")
            openLxApp(currentLxApp.appId, currentLxApp.path)
        } else {
            Log.i(TAG, "No more LxApps in stack, activity will remain empty")
        }
    }

    // Switch to a different LxApp in the current activity
    fun openLxApp(appId: String, path: String) {

        // Ensure all UI operations are on the main thread
        runOnUiThread {
            // Update app state (no intent extras needed - we're not switching activities)
            this.appId = appId
            this.isDisplayingHomeLxApp = (appId == LxApp.HomeLxAppId)

            // 1. Necessary preparation (build tabbar, etc.)
            prepareLxApp(appId)

            // 2. Check whether to show capsule button (home=hide, others=show)
            updateCapsuleButtonVisibility(appId)

            // 3. Call navigate as entry point
            if (path.isNotEmpty()) {
                navigate(path, NavigationType.LAUNCH)
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
        val isHomeLxApp = (appId == LxApp.HomeLxAppId)

        if (isHomeLxApp) {
            // Home LxApp: hide capsule button
            val capsuleButton = rootContainer.findViewWithTag<View>("capsule_button")
            capsuleButton?.visibility = View.GONE

        } else {
            // Other LxApps: ensure capsule button exists and is visible
            updateCapsuleButton()
        }
    }

    // Update capsule button visibility
    private fun updateCapsuleButton() {
        rootContainer.post {
            val capsule = rootContainer.findViewWithTag<View>("capsule_button")
            if (isDisplayingHomeLxApp) {
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



    // Get current WebView (internal access for LxApp)
    internal fun getCurrentWebView(): com.lingxia.lxapp.WebView? = currentWebView

    // Handle configuration changes to prevent Activity recreation
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)

        // Update layout to adapt to screen orientation changes
        updateLayoutMargins()
    }
}
