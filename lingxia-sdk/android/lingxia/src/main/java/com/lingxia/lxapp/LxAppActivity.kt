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
import java.lang.ref.WeakReference
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
            Log.d(TAG, "updateTabBarUI called for appId: $appId")

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
                            Log.d(TAG, "TabBar refreshed successfully with ${newTabBarConfig.list.size} items")
                        } else {
                            Log.w(TAG, "No TabBar config available for refresh")
                        }
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to refresh TabBar from Rust: ${e.message}", e)
                    }
                }
                Log.d(TAG, "TabBar UI update triggered for appId: $appId")
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
            Log.d(TAG, "updateNavBarUI called for appId: $appId")

            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.appId == appId) {
                activity.runOnUiThread {
                    val currentPath = activity.currentWebView?.getCurrentPath() ?: ""
                    activity.navigationBar?.refreshState(appId, currentPath)
                    activity.updateLayoutMargins()
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
                Log.d(TAG, "Starting parallel WebView creation for $appId:$initialPath")
                findWebView(appId, initialPath)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to start parallel WebView creation: ${e.message}")
        }

        // Force navigationBar to null for recreations due to screen rotation
        navigationBar = null

        // Defer broadcast receiver registration to reduce onCreate time
        // These are not critical for initial display
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
                        val result = NativeApi.onBackPressed(appId)
                        Log.d(TAG, "Back press handled by native: $result")
                    } catch (e: Exception) {
                        Log.e(TAG, "Error handling back press: ${e.message}")
                    }
                }
            })
        }

        Log.d(TAG, "LxAppActivity onCreate completed for appId: $appId, path: $initialPath")
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
            Log.d(TAG, "Invalid or insufficient TabBar config, TabBar not shown.")
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
                    Log.d(TAG, "Tab clicked: index=$index, path=$path")
                    navigate(path, NavigationType.SWITCH_TAB)
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

        // Calculate NavigationBar height - use content height plus small padding for better spacing
        val isNavBarVisible = navigationBar?.visibility == View.VISIBLE
        val navBarContentHeight = if (isNavBarVisible) {
            // Use NavigationBar's content height plus a small padding for optimal visual spacing
            val contentHeight = navigationBar?.getCalculatedContentHeightPx()
                ?: (DEFAULT_NAV_BAR_HEIGHT_DP * resources.displayMetrics.density).toInt()

            // Add 8dp padding for better visual spacing
            contentHeight + (8 * resources.displayMetrics.density).toInt()
        } else 0

        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = navBarContentHeight  // Use NavigationBar content height plus padding
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
            Log.d(TAG, "Attaching and resuming WebView for path: ${view.getCurrentPath()}")

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

    private class MoreDotsDrawable : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.BLACK
            style = Paint.Style.FILL
        }

        override fun draw(canvas: Canvas) {
            val centerY = bounds.height() / 2f
            val centerX = bounds.width() / 2f

            // Center dot is larger, side dots are smaller
            val centerDotRadius = bounds.height() / 7f  // Larger center dot
            val sideDotRadius = bounds.height() / 10f   // Smaller side dots
            val spacing = centerDotRadius * 2.8f        // Adjusted spacing

            // Draw side dots
            canvas.drawCircle(centerX - spacing, centerY, sideDotRadius, paint)
            canvas.drawCircle(centerX + spacing, centerY, sideDotRadius, paint)

            // Draw center dot (larger)
            canvas.drawCircle(centerX, centerY, centerDotRadius, paint)
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
    }

    private inner class CloseButtonDrawable : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.BLACK
            style = Paint.Style.STROKE
            strokeWidth = 3f * this@LxAppActivity.resources.displayMetrics.density  // Increase circle thickness
        }

        private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.BLACK
            style = Paint.Style.FILL
        }

        override fun draw(canvas: Canvas) {
            val centerX = bounds.width() / 2f
            val centerY = bounds.height() / 2f
            val radius = bounds.width() / 2f  // Adjust circle size

            // Draw circle with thicker stroke
            paint.style = Paint.Style.STROKE
            canvas.drawCircle(centerX, centerY, radius, paint)

            // Draw smaller center dot
            paint.style = Paint.Style.FILL
            canvas.drawCircle(centerX, centerY, radius / 2.5f, dotPaint)  // Center dot
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
            dotPaint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
            dotPaint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT
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
                cornerRadius = 18f * resources.displayMetrics.density // Half of height (36/2) for perfect rounded corners
                setStroke((0.5f * resources.displayMetrics.density).toInt(), 0xFFDDDDDD.toInt())
            }

            // Capsule layout parameters - Position fixed relative to status bar
            val capsuleLayoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                (36 * resources.displayMetrics.density).toInt()
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
            setImageDrawable(MoreDotsDrawable())
            layoutParams = LinearLayout.LayoutParams(
                (44 * resources.displayMetrics.density).toInt(),
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setOnClickListener {
                // Handle more options click
                Log.d(TAG, "More options clicked")
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
            setImageDrawable(CloseButtonDrawable())
            layoutParams = LinearLayout.LayoutParams(
                (44 * resources.displayMetrics.density).toInt(),
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setOnClickListener {
                closeLxApp()
                LxApp.openHomeLxApp()
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
            Log.d(TAG, "Resuming current WebView in onResume")
            currentWebView?.visibility = View.VISIBLE // Ensure visibility
            webViewContainer.visibility = View.VISIBLE
            currentWebView?.resume()
        } else {
            Log.d(TAG, "Skipping WebView resume in onResume because pendingWebViewSetup is true")
        }
    }

    override fun onPause() {
        super.onPause()
        Log.d(TAG, "Pausing current WebView in onPause")
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
            Log.d(TAG, "Pausing current WebView (onDestroy)")
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

        // Allow same path for launch (app initialization)
        if (currentWebView?.getCurrentPath() == targetPath && navigationType != NavigationType.LAUNCH) {
            return true
        }

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
        Log.d(TAG, "Coordinated navigation update: $targetPath with type: $navigationType")

        // Step 1: Get navbar state early for coordination
        val pageConfig = getNavBarState(appId, targetPath)

        // Step 2: Apply tabbar updates with coordination
        applyNavigationTypeUpdates(navigationType, targetPath)

        // Step 3: Navigate with coordinated timing
        return navigateToPageWithCoordination(targetPath, navigationType, pageConfig)
    }

    /**
     * Apply navigation type specific UI updates with smooth animations
     *
     * IMPROVEMENT: Added smooth tabbar transitions to match iOS/macOS/Harmony polish
     * - SWITCH_TAB: Shows tabbar with fade-in animation
     * - LAUNCH: Hides tabbar with fade-out animation (non-tab pages)
     * - REPLACE/FORWARD/BACKWARD: Hides tabbar with fade-out animation
     */
    private fun applyNavigationTypeUpdates(navigationType: NavigationType, targetPath: String) {
        when (navigationType) {
            NavigationType.SWITCH_TAB -> {
                showTabBarWithAnimation(true)
                tabBar?.findTabIndexByPath(targetPath)?.let { index ->
                    if (index >= 0) tabBar?.setSelectedIndex(index, notifyListener = false)
                }
            }
            NavigationType.LAUNCH -> {
                showTabBarWithAnimation(false)  // Non-tab page
            }
            NavigationType.REPLACE, NavigationType.FORWARD, NavigationType.BACKWARD -> {
                showTabBarWithAnimation(false)
            }
        }
    }

    /**
     * Show or hide TabBar with smooth fade animation
     * Provides elegant transitions like iOS/macOS/Harmony platforms
     */
    private fun showTabBarWithAnimation(show: Boolean) {
        tabBar?.let { tabBar ->
            // Avoid duplicate animations
            if (show && tabBar.visibility == View.VISIBLE && tabBar.alpha == 1f) {
                return // Already visible
            }
            if (!show && tabBar.visibility == View.GONE) {
                return // Already hidden
            }

            // Cancel any existing animation to prevent conflicts
            tabBar.animate().cancel()

            if (show) {
                // Fade in animation with slight scale effect for polish
                Log.d(TAG, "TabBar fade in animation")
                tabBar.visibility = View.VISIBLE
                tabBar.alpha = 0f
                tabBar.scaleY = 0.95f // Subtle scale effect
                tabBar.animate()
                    .alpha(1f)
                    .scaleY(1f)
                    .setDuration(250) // Slightly longer for smoothness
                    .setInterpolator(android.view.animation.DecelerateInterpolator(1.5f))
                    .withStartAction {
                        // Ensure proper initial state
                        tabBar.visibility = View.VISIBLE
                    }
                    .start()
            } else {
                // Fade out animation with subtle scale effect
                Log.d(TAG, "TabBar fade out animation")
                tabBar.animate()
                    .alpha(0f)
                    .scaleY(0.95f) // Subtle scale effect
                    .setDuration(200) // Faster fade out
                    .setInterpolator(android.view.animation.AccelerateInterpolator(1.2f))
                    .withEndAction {
                        tabBar.visibility = View.GONE
                        tabBar.alpha = 1f // Reset for next show
                        tabBar.scaleY = 1f // Reset scale
                    }
                    .start()
            }
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
        navigationType: NavigationType,
        pageConfig: NavigationBarState?
    ): Boolean {
        Log.d(TAG, "Coordinated page navigation: $targetPath")

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

                    Log.d(TAG, "Old container animated out and cleaned up")
                } catch (e: Exception) {
                    Log.e(TAG, "Error cleaning up old container: ${e.message}")
                }
            }
            .start()
    }

    /**
     * Perform WebView transition animation
     *
     * Extracted from navigateToPage for reuse in coordinated navigation
     */
    private fun performWebViewTransition(oldWebView: WebView?, newContainer: FrameLayout, isBackNavigation: Boolean, shouldAnimate: Boolean = true) {
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

        // Update layout margins to position the new container vertically
        updateLayoutMargins()

        if (shouldAnimate) {
            // Set up animation based on navigation direction
            val slideInTranslation = if (isBackNavigation) -webViewContainer.width.toFloat() else webViewContainer.width.toFloat()
            val slideOutTranslation = if (isBackNavigation) webViewContainer.width.toFloat() else -webViewContainer.width.toFloat()

            // Set initial position for new container
            newContainer.translationX = slideInTranslation

            // Animate the transition
            val animationDuration = 300L

            // Animate new container sliding in
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
                        // Clean up old container
                        cleanupOldContainer(container)
                    }
                    .start()
            }
        } else {
            // No animation - just set position and trigger callbacks immediately
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
        Log.d(TAG, "navigateToPage (Coordinated): targetPath=$targetPath, isReplace=$isReplace, isBackNavigation=$isBackNavigation")

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

            // COORDINATED UPDATE: Update navigation bar and webview together
            // This prevents timing issues between navbar and webview updates
            updateNavigationBar(actualPageConfig, isBackNavigation, disableAnimation = false, targetPath = targetPath)

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
            performWebViewTransition(oldWebView, newContainer, isBackNavigation, shouldAnimate)

            // Update the current WebView reference
            currentWebView = newWebView

        } catch (e: Exception) {
            Log.e(TAG, "Error in coordinated navigation: ${e.message}", e)
        }
    }

    /**
     * Updates the navigation bar based on the page configuration and navigation context.
     * This method determines the required state and delegates the update and animation
     * to the NavigationBar instance.
     *
     * @param config The navigation bar configuration for the target page.
     * @param isBackNavigation Whether the navigation event is a 'back' navigation.
     * @param disableAnimation Whether the update should be instant (true) or animated (false).
     * @param targetPath The target path to update navbar for (optional, uses currentWebView if null).
     */
    private fun updateNavigationBar(config: NavigationBarState?, isBackNavigation: Boolean, disableAnimation: Boolean = false, targetPath: String? = null) {
        Log.d(TAG, "updateNavigationBar called: isBackNavigation=$isBackNavigation, disableAnimation=$disableAnimation, targetPath=$targetPath")

        try {
            // Use explicit targetPath if provided, otherwise fall back to currentWebView
            val pathForNavbar = targetPath ?: currentWebView?.getCurrentPath() ?: ""
            Log.d(TAG, "Getting fresh navbar state for $appId:$pathForNavbar")

            if (navigationBar == null) {
                // Create navbar if it doesn't exist
                Log.d(TAG, "Creating new NavigationBar")
                val statusBarHeight = getStatusBarHeight(this)
                val newNavBar = NavigationBar(this)

                // Setup navbar layout and properties
                val navBarContentHeightPx = newNavBar.getCalculatedContentHeightPx()
                val finalNavBarLayoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    navBarContentHeightPx + statusBarHeight
                ).apply {
                    gravity = Gravity.TOP
                }
                newNavBar.layoutParams = finalNavBarLayoutParams
                newNavBar.setPadding(newNavBar.paddingLeft, 0, newNavBar.paddingRight, newNavBar.paddingBottom)
                newNavBar.setExternalStatusBarHeight(statusBarHeight)

                // Setup button click listeners
                newNavBar.setOnBackButtonClickListener { handleBackButtonClick() }
                newNavBar.setOnHomeButtonClickListener { handleHomeButtonClick() }

                navigationBar = newNavBar
            }

            // Use the unified refreshState API to get fresh state from Rust
            navigationBar?.refreshState(appId, pathForNavbar)

            // Ensure navbar is added to container
            if (navigationBar != null && ::rootContainer.isInitialized) {
                if (navigationBar?.parent != null) {
                    (navigationBar?.parent as? ViewGroup)?.removeView(navigationBar)
                }
                rootContainer.addView(navigationBar)
            }

            updateLayoutMargins()

        } catch (e: Exception) {
            Log.e(TAG, "Error updating navigation bar", e)
        }
    }

    /**
     * Handles the click event from the NavigationBar's back button.
     */
    private fun handleBackButtonClick() {
        Log.d(TAG, "NavigationBar back button clicked")
    }

    /**
     * Handles the click event from the NavigationBar's home button.
     */
    private fun handleHomeButtonClick() {
        Log.d(TAG, "NavigationBar home button clicked")
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
        Log.d(TAG, "Closing current LxApp: $appId")

        // Notify native layer
        notifyLxAppClosed()

        // Pause and clean up current WebView
        currentWebView?.let { webView ->
            webView.pause()
            webView.visibility = View.GONE
        }
        webViewContainer.removeAllViews()
        currentWebView = null

        // Hide tab bar with animation (capsule and navbar remain)
        showTabBarWithAnimation(false)

        // Clear app state
        appId = ""
        isDisplayingHomeLxApp = false
    }

    // Switch to a different LxApp in the current activity (openLxApp/closeLxApp lifecycle)
    fun openLxApp(appId: String, path: String) {
        Log.d(TAG, "Opening LxApp: $appId at path: $path")

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
            val targetPath = if (path.isNotEmpty()) path else getInitialRoute() ?: ""
            if (targetPath.isNotEmpty()) {
                navigate(targetPath, NavigationType.LAUNCH)
            } else {
                Log.e(TAG, "No valid path to navigate to")
            }
        }
    }

    /**
     * 1. Necessary preparation for LxApp (build tabbar, etc.)
     */
    private fun prepareLxApp(appId: String) {
        Log.d(TAG, "Preparing LxApp: $appId")

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
            showTabBarWithAnimation(true)
            Log.d(TAG, "TabBar configured for app: $appId")
        } else {
            showTabBarWithAnimation(false)
            Log.d(TAG, "No TabBar for app: $appId")
        }
    }

    /**
     * 2. Check whether to show capsule button (home=hide, others=show)
     */
    private fun updateCapsuleButtonVisibility(appId: String) {
        val isHomeLxApp = (appId == LxApp.HomeLxAppId)

        if (isHomeLxApp) {
            // Home LxApp: hide capsule button
            val capsuleButton = rootContainer.findViewWithTag<View>("capsule_button")
            capsuleButton?.visibility = View.GONE
            Log.d(TAG, "Capsule button hidden for home LxApp")
        } else {
            // Other LxApps: ensure capsule button exists and is visible
            updateCapsuleButton()
            Log.d(TAG, "Capsule button shown for LxApp: $appId")
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

    /**
     * Get initial route for the current app
     */
    fun getInitialRoute(): String? {
        if (!::appId.isInitialized) return null

        val appInfo = NativeApi.getLxAppInfo(appId)
        return appInfo?.initialRoute
    }

    // Get current WebView (internal access for LxApp)
    internal fun getCurrentWebView(): com.lingxia.lxapp.WebView? = currentWebView

    // Handle configuration changes to prevent Activity recreation
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        Log.d(TAG, "Configuration changed, updating layout")

        // Update layout to adapt to screen orientation changes
        updateLayoutMargins()
    }
}
