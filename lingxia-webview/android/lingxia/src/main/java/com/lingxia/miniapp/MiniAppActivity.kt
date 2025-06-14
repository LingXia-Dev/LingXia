package com.lingxia.miniapp

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
import android.content.BroadcastReceiver
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import android.view.animation.AccelerateDecelerateInterpolator

// Define a constant for the switch page action
const val ACTION_SWITCH_PAGE = "com.lingxia.SWITCH_PAGE_ACTION"
// Define a constant for the close mini app action
const val ACTION_CLOSE_MINIAPP = "com.lingxia.CLOSE_MINIAPP_ACTION"

class MiniAppActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"
        internal const val DEFAULT_NAV_BAR_HEIGHT_DP =12
        internal const val DEFAULT_TAB_BAR_SIZE_DP = 56

        private var lastWebView: WeakReference<com.lingxia.miniapp.WebView>? = null

        // Native method for handling mini app closed event
        @JvmStatic
        external fun nativeOnMiniAppClosed(appId: String): Int

        // Native method for handling back press event
        @JvmStatic
        private external fun nativeOnBackPressed(appId: String): Int

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
                clearFlags(android.view.WindowManager.LayoutParams.FLAG_TRANSLUCENT_STATUS)
                clearFlags(android.view.WindowManager.LayoutParams.FLAG_TRANSLUCENT_NAVIGATION)
                statusBarColor = Color.TRANSPARENT
                // Navigation bar color will be set by updateNavigationBarTransparency
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

        @JvmStatic
        external fun nativeOnPageShow(appId: String, path: String)
    }

    private lateinit var appId: String
    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    private var tabBar: TabBar? = null
    private var navigationBar: NavigationBar? = null
    private var isDestroyed = false
    private var pendingWebViewSetup = false
    private var isDisplayingHomeMiniApp: Boolean = false

    // Tracks the currently visible WebView instance
    private var currentWebView: com.lingxia.miniapp.WebView? = null

    // Broadcast receiver for receiving mini app close requests
    private val closeAppReceiver = object : android.content.BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == ACTION_CLOSE_MINIAPP) {
                val targetAppId = intent.getStringExtra("appId")
                if (::appId.isInitialized && targetAppId == appId) {
                    Log.d(TAG, "Received close request for appId: $appId")
                    finish()
                }
            }
        }
    }

    // Broadcast receiver for page switching // Changed comment
    private val switchPageReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == ACTION_SWITCH_PAGE) {
                val targetAppId = intent.getStringExtra("appId")
                val targetPath = intent.getStringExtra("path")

                if (::appId.isInitialized && targetAppId == appId && targetPath != null) {
                    Log.d(TAG, "Received switch page broadcast - appId: $appId, path: $targetPath") // Changed log

                    // Added try-catch and pre-load logic
                    try {
                        // Pre-load existing WebView if available to prevent white screen
                        val existingWebView = com.lingxia.miniapp.WebView.nativeFindWebView(appId, targetPath)
                        if (existingWebView != null) {
                            existingWebView.visibility = View.VISIBLE
                            existingWebView.resume()
                        }

                        // Trigger page switch
                        switchPage(targetPath)
                    } catch (e: Exception) {
                        Log.e(TAG, "Error switching page via broadcast: ${e.message}", e)
                    }
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize appId from intent FIRST (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            Log.e(TAG, "Missing required parameter: appId")
            finish()
            return
        }
        val initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""

        // Initialize the new flag
        isDisplayingHomeMiniApp = (this.appId == MiniApp.HomeMiniAppId)

        // Start WebView creation in parallel while setting up UI
        var webViewFuture: java.util.concurrent.Future<Pair<com.lingxia.miniapp.WebView?, NavigationBarConfig?>>? = null
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()

        try {
            webViewFuture = executor.submit<Pair<com.lingxia.miniapp.WebView?, NavigationBarConfig?>> {
                Log.d(TAG, "Starting parallel WebView creation for $appId:$initialPath")
                findWebViewForPage(appId, initialPath)
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
        val tabBarJson = MiniApp.nativeGetTabBarConfig(appId)
        val tabBarConfig = TabBarConfig.fromJson(tabBarJson)

        // Configure system UI early but efficiently
        configureTransparentSystemBars(this)
        updateNavigationBarTransparency(this, false, Color.WHITE)
        window.setBackgroundDrawableResource(android.R.color.transparent)

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
                                     (tabBarBgColor != null && Color.alpha(tabBarBgColor) < 255)

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
            if (webViewResult?.first != null) {
                setupWebViewContentWithExisting(webViewResult.first!!)
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
            // Register broadcast receivers after UI is ready
            registerReceiver(closeAppReceiver, android.content.IntentFilter(ACTION_CLOSE_MINIAPP))
            registerReceiver(switchPageReceiver, android.content.IntentFilter(ACTION_SWITCH_PAGE))

            // Setup back press handler
            onBackPressedDispatcher.addCallback(object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    try {
                        currentWebView?.visibility = View.VISIBLE
                        val result = nativeOnBackPressed(appId)
                        Log.d(TAG, "Back press handled by native: $result")
                        if (result <= 0) {
                            Log.d(TAG, "No back navigation available, finishing")
                            finish()
                        }
                    } catch (e: Exception) {
                        Log.e(TAG, "Error handling back press: ${e.message}")
                        finish()
                    }
                }
            })

            // Final layout update
            updateLayoutMargins()
        }

        Log.d(TAG, "MiniAppActivity onCreate completed for appId: $appId, path: $initialPath")
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

    private fun setupTabBar(config: TabBarConfig?) {
        if (config == null) {
            Log.d(TAG, "Invalid or insufficient TabBar config, TabBar not shown.")
            return
        }

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor != null && Color.alpha(tabBarBgColor) < 255)

        // Get the actual TabBar background color (considering defaults)
        val actualTabBarColor = when {
            config.backgroundColor != null -> config.backgroundColor!!
            config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT -> {
                // Use vertical TabBar default color from TabBar class
                Color.parseColor("#F8F8F8") // VERTICAL_TABBAR_BACKGROUND_COLOR
            }
            else -> Color.WHITE // DEFAULT_BACKGROUND_COLOR
        }

        // Update system navigation bar transparency based on TabBar transparency and color
        updateNavigationBarTransparency(this, isTabBarTransparent, actualTabBarColor)

        if (tabBar == null) {
            tabBar = TabBar(this).apply {
                setConfig(config)
                setOnTabSelectedListener { index, path ->
                    Log.d(TAG, "Tab clicked: index=$index, path=$path")
                    switchToTab(path)
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

    private fun applyTabBarLayoutParams(tabBar: TabBar, config: TabBarConfig) {
        val isVertical = config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT
        val density = resources.displayMetrics.density
        val defaultTabBarSizePx = (DEFAULT_TAB_BAR_SIZE_DP * density).toInt()
        val verticalWidthPx = (DEFAULT_TAB_BAR_SIZE_DP * 1.0f * density).toInt()

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor != null && Color.alpha(tabBarBgColor) < 255)

        (tabBar.layoutParams as? FrameLayout.LayoutParams)?.apply {
            if (isVertical) {
                width = verticalWidthPx
                height = ViewGroup.LayoutParams.MATCH_PARENT
                gravity = when (config.position) {
                    TabBarConfig.Position.LEFT -> Gravity.START
                    TabBarConfig.Position.RIGHT -> Gravity.END
                    else -> Gravity.START
                }
            } else {
                width = ViewGroup.LayoutParams.MATCH_PARENT
                height = defaultTabBarSizePx
                gravity = when (config.position) {
                    TabBarConfig.Position.TOP -> Gravity.TOP
                    TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    else -> Gravity.BOTTOM
                }

                if (isTabBarTransparent && config.position == TabBarConfig.Position.BOTTOM) {
                    // For transparent TabBar, use a small fixed margin to avoid excessive spacing
                    // while still providing enough space to avoid overlap with system navigation
                    bottomMargin = (8 * resources.displayMetrics.density).toInt()
                }
            }
            tabBar.layoutParams = this
        } ?: run {
            val newLayoutParams = FrameLayout.LayoutParams(0,0)
            if (isVertical) {
                newLayoutParams.width = verticalWidthPx
                newLayoutParams.height = ViewGroup.LayoutParams.MATCH_PARENT
                newLayoutParams.gravity = when (config.position) {
                    TabBarConfig.Position.LEFT -> Gravity.START
                    TabBarConfig.Position.RIGHT -> Gravity.END
                    else -> Gravity.START
                }
            } else {
                newLayoutParams.width = ViewGroup.LayoutParams.MATCH_PARENT
                newLayoutParams.height = defaultTabBarSizePx
                newLayoutParams.gravity = when (config.position) {
                    TabBarConfig.Position.TOP -> Gravity.TOP
                    TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    else -> Gravity.BOTTOM
                }

                if (isTabBarTransparent && config.position == TabBarConfig.Position.BOTTOM) {
                    // For transparent TabBar, use a small fixed margin to avoid excessive spacing
                    // while still providing enough space to avoid overlap with system navigation
                    newLayoutParams.bottomMargin = (8 * resources.displayMetrics.density).toInt()
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
                                 (tabBarBgColor != null && Color.alpha(tabBarBgColor) < 255)

        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = 0
            bottomMargin = 0
            leftMargin = 0
            rightMargin = 0

            if (!isTabBarTransparent) {
                when (tabBar?.config?.position) {
                    TabBarConfig.Position.BOTTOM -> {
                        if (isTabBarVisible) bottomMargin = tabBarHeight
                    }
                    TabBarConfig.Position.TOP -> {
                        if (isTabBarVisible) topMargin = tabBarHeight
                    }
                    TabBarConfig.Position.LEFT -> {
                        if (isTabBarVisible) leftMargin = tabBarWidth
                    }
                    TabBarConfig.Position.RIGHT -> {
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

    // Helper function to find existing WebView instance for a given path/page
    private fun findWebViewForPage(appId: String, path: String): Pair<com.lingxia.miniapp.WebView?, NavigationBarConfig?> {
        var webView = com.lingxia.miniapp.WebView.nativeFindWebView(appId, path)

        if (webView == null) {
            Log.w(TAG, "WebView not found for appId=$appId, path=$path. WebView should be created by Rust layer.")
            return Pair(null, null)
        } else {
            Log.d(TAG, "Using existing WebView instance for page: $path")
        }

        // Get page config - Nav bar configuration is now handled by the caller
        val pageConfig = MiniApp.getPageConfig(appId, path)

        return Pair(webView, pageConfig)
    }

    // Helper function to attach a WebView to the container and resume it
    private fun attachWebViewToUI(view: com.lingxia.miniapp.WebView?) {
        if (view == null) {
            Log.e(TAG, "attachWebViewToUI called with null view!")
            return
        }
        if (!isDestroyed) {
            Log.d(TAG, "Attaching and resuming WebView for path: ${view.currentPath}")

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
            
            // Unified onPageShow trigger - called for all WebViews when attached to UI
            if (view.appId != null && view.currentPath != null) {
                try {
                    nativeOnPageShow(view.appId!!, view.currentPath!!)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to call nativeOnPageShow: ${e.message}")
                }
            }
        }
    }

    private fun setupWebViewContent(appId: String, path: String) {
        val initialWebView = findWebViewForPage(appId, path)
        if (initialWebView.first == null) {
            Log.e(TAG, "Failed to find or create initial WebView for $path")
            finish(); return
        }
        setupWebViewContentWithExisting(initialWebView.first!!)
    }

    // New method to setup WebView content with an existing WebView
    private fun setupWebViewContentWithExisting(webView: com.lingxia.miniapp.WebView) {
        // Attach and resume immediately
        attachWebViewToUI(webView)

        // Set the current WebView
        this.currentWebView = webView

        // Update last used WebView reference
        lastWebView = WeakReference(webView)
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
            strokeWidth = 3f * this@MiniAppActivity.resources.displayMetrics.density  // Increase circle thickness
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
        if (isDisplayingHomeMiniApp) {
            Log.d(TAG, "Not adding capsule button because it is the home app.")
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
                setColor(Color.parseColor("#FFFFFF"))
                cornerRadius = 20f * resources.displayMetrics.density
                setStroke((0.5f * resources.displayMetrics.density).toInt(), Color.parseColor("#DDDDDD"))
            }

            // Capsule layout parameters - Position fixed relative to status bar
            val capsuleLayoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                (36 * resources.displayMetrics.density).toInt()
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                // Position with fixed offset relative to status bar
                topMargin = statusBarHeight + (8 * resources.displayMetrics.density).toInt()
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
            setBackgroundColor(Color.parseColor("#DDDDDD"))
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
                // Directly close the current activity
                finish()
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
    private fun notifyMiniAppClosed() {
        nativeOnMiniAppClosed(appId)
    }

    override fun onDestroy() {
        isDestroyed = true

        // Unregister broadcast receivers // Changed comment
        try {
            unregisterReceiver(closeAppReceiver)
            unregisterReceiver(switchPageReceiver) // Kept comment change
        } catch (e: Exception) {
            Log.w(TAG, "Failed to unregister receiver: ${e.message}")
        }

        // Pause current WebView but don't destroy it
        // WebView destruction is managed by native
        currentWebView?.let { view ->
            Log.d(TAG, "Pausing current WebView (onDestroy)")
            view.pause()
        }

        // Clear page config cache to prevent memory leaks
        MiniApp.clearPageConfigCache()

        super.onDestroy()
    }

    // Handles switching ROOT pages associated with Tabs
    private fun switchToTab(targetPath: String) {
        Log.d(TAG, "Switching TAB to path: $targetPath, container children: ${webViewContainer.childCount}")

        val appId = intent.getStringExtra(EXTRA_APP_ID)
        if (appId.isNullOrEmpty()) {
            Log.e(TAG, "switchToTab failed: Cannot get/create WebView, appId is missing.")
            return
        }

        // Bail early if trying to switch to the current path
        if (currentWebView?.currentPath == targetPath) {
            Log.d(TAG, "Already on this tab, no need to switch")
            return
        }

        // Capture reference to previous WebView before changing anything
        val previousWebView = currentWebView

        // First prep the UI changes before touching WebViews
        val targetIndex = tabBar?.findTabIndexByPath(targetPath) ?: -1
        if (targetIndex == -1) {
            Log.e(TAG, "switchToTab failed: Path '$targetPath' not found in TabBar items.")
            return
        }

        // Find or create target WebView
        val (targetWebView, pageConfig) = findWebViewForPage(appId, targetPath)
        if (targetWebView == null) {
            Log.e(TAG, "switchToTab failed: findWebViewForPage returned null for $targetPath")
            return
        }

        // Update last used WebView reference *before* potentially pausing previous one
        lastWebView = WeakReference(targetWebView)

        // Set current WebView to target for tracking *early*
        currentWebView = targetWebView

        // Update TabBar UI first (without triggering listener)
        tabBar?.setSelectedIndex(targetIndex, notifyListener = false)

        // Configure navigation bar for the TARGET tab page using the helper (disable animation)
        updateNavigationBar(pageConfig, isBackNavigation = false, disableAnimation = true)

        // Use unified attachment logic - this will trigger nativeOnPageShow
        attachWebViewToUI(targetWebView)

        // Pause and hide the previous WebView
        if (previousWebView != null && previousWebView != targetWebView) {
            previousWebView.pause()
            previousWebView.visibility = View.GONE
            if (previousWebView.parent == webViewContainer) {
                webViewContainer.removeView(previousWebView)
                Log.d(TAG, "Removed previous tab WebView: ${previousWebView.currentPath}")
            }
        }
    }

     /**
     * Switch to a specific page within the mini app.
     * This is the main entry point for page navigation.
     *
     * @param targetPath Path of the page to navigate to
     */
    private fun switchPage(targetPath: String) { // Replaced old public switchPage with this private dispatcher
        if (!::appId.isInitialized) {
            Log.e(TAG, "Cannot switch page: appId not initialized")
            return
        }

        try {
            // Check if trying to navigate to current page
            if (currentWebView?.currentPath == targetPath) {
                return
            }

            // Check if this is a tab page
            val tabIndex = tabBar?.findTabIndexByPath(targetPath)
            if (tabIndex != null && tabIndex >= 0) {
                Log.d(TAG, "Switching to tab page at index: $tabIndex")
                switchToTab(targetPath)
            } else {
                // Handle non-tab page navigation
                Log.d(TAG, "Navigating to non-tab page: $targetPath")

                // Determine if this is back navigation (simplistically by path length)
                val currentPath = currentWebView?.currentPath
                val isBackNavigation = currentPath != null && currentPath.length > targetPath.length

                navigateToPage(targetPath, isReplace = false, isBackNavigation = isBackNavigation)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error in switchPage: ${e.message}", e)
        }
    }

    /**
     * Navigate to a non-tab page. This version focuses on correctness over animation.
     *
     * @param targetPath Path of the page to navigate to
     * @param isBackNavigation Whether this is a back navigation
     */
    private fun navigateToPage(targetPath: String, isReplace: Boolean = false, isBackNavigation: Boolean = false) { // Reintroducing container animation
        Log.d(TAG, "navigateToPage (Animated): targetPath=$targetPath, isReplace=$isReplace, isBackNavigation=$isBackNavigation")

        try {
            // Get current WebView before changes
            val oldWebView = currentWebView

            // Find or create WebView for the target page
            val (newWebView, pageConfig) = findWebViewForPage(appId, targetPath)
            if (newWebView == null) {
                Log.e(TAG, "Failed to create WebView for path: $targetPath")
                return
            }

            // Update navigation bar configuration (pass disableAnimation=false)
            updateNavigationBar(pageConfig, isBackNavigation, disableAnimation = false)

            if (newWebView.parent != null) {
                (newWebView.parent as? ViewGroup)?.removeView(newWebView)
            }

            // IMPORTANT: Make sure the new WebView is fully prepared before animation
            newWebView.visibility = View.VISIBLE // Should be visible INSIDE its container
            newWebView.resume()

            // Create a new container for the WebView
            val newContainer = FrameLayout(this).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                tag = "current_webview_container" // Tag the container

                try {
                    addView(newWebView)
                } catch (e: Exception) {
                    Log.e(TAG, "Error adding WebView to new container: ${e.message}")
                    return@apply
                }
            }

            // Get reference to old container BEFORE adding new one
            val oldContainer = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
            oldContainer?.tag = "previous_webview_container" // Re-tag old container

            try {
                webViewContainer.addView(newContainer)
            } catch (e: Exception) {
                Log.e(TAG, "Error adding new container to webViewContainer: ${e.message}")
                return
            }

            // Update layout margins NOW to position the new container vertically
            updateLayoutMargins()

            // Set initial horizontal position for animation (AFTER vertical positioning)
            val startX = if (isBackNavigation) -webViewContainer.width.toFloat() else webViewContainer.width.toFloat()
            newContainer.translationX = startX

            // Animation parameters
            val duration = 250L
            val interpolator = AccelerateDecelerateInterpolator()
            val endXOld = if (isBackNavigation) webViewContainer.width.toFloat() else -webViewContainer.width.toFloat()

            // Animate the new container in
            newContainer.animate()
                .translationX(0f)
                .setDuration(duration)
                .setInterpolator(interpolator)
                .withEndAction {
                    // Trigger nativeOnPageShow after animation completes
                    if (newWebView.appId != null && newWebView.currentPath != null) {
                        try {
                            nativeOnPageShow(newWebView.appId!!, newWebView.currentPath!!)
                            Log.d(TAG, "navigateToPage: Triggered onPageShow for appId=${newWebView.appId} path=${newWebView.currentPath}")
                        } catch (e: Exception) {
                            Log.e(TAG, "Failed to call nativeOnPageShow in navigateToPage: ${e.message}")
                        }
                    }
                }
                .start()

            // Update the current WebView reference BEFORE animating old container out
            currentWebView = newWebView
            lastWebView = WeakReference(newWebView)

            // Animate the old container out AFTER new one starts coming in
            if (oldContainer != null && oldWebView != null) {
                oldContainer.animate()
                    .translationX(endXOld)
                    .setDuration(duration)
                    .setInterpolator(interpolator)
                    .withEndAction {
                        // Only remove old container after animation completes
                        if (!isDestroyed) {
                            try {
                                // Ensure old WebView is paused before removing
                                oldWebView.pause()
                                // Remove old container after animation
                                if (oldContainer.parent == webViewContainer) {
                                    webViewContainer.removeView(oldContainer)
                                    Log.d(TAG, "Removed old container after animation")
                                }
                            } catch (e: Exception) {
                                Log.e(TAG, "Error cleaning up old container: ${e.message}")
                            }
                        }
                    }
                    .start()
            } else {
                 Log.d(TAG, "No old container/webview to animate out.")
            }

            Log.d(TAG, "Navigation animation initiated for page: $targetPath")
        } catch (e: Exception) {
            Log.e(TAG, "Error navigating to page: ${e.message}")
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
     */
    private fun updateNavigationBar(config: NavigationBarConfig?, isBackNavigation: Boolean, disableAnimation: Boolean = false) {
        Log.d(TAG, "updateNavigationBar called: isBackNavigation=$isBackNavigation, disableAnimation=$disableAnimation")

        try {
            // Determine if Nav Bar should be shown at all
            if (config != null && config.hidden) {
                Log.d(TAG, "NavigationBar hidden by configuration")
                navigationBar?.hide()
                updateLayoutMargins() // Still need to update layout when hidden
                return
            }

            // Create navigation bar if it doesn't exist
            if (navigationBar == null) {
                Log.d(TAG, "Creating new NavigationBar")
                val statusBarHeight = getStatusBarHeight(this)
                Log.d(TAG, "MiniAppActivity: statusBarHeight = $statusBarHeight")
                val newNavBar = NavigationBar(this)

                // 1. Get the content height explicitly from NavigationBar's calculation.
                val navBarContentHeightPx = newNavBar.getCalculatedContentHeightPx()
                Log.d(TAG, "MiniAppActivity: navBarContentHeightPx from getCalculatedContentHeightPx() = $navBarContentHeightPx")

                val finalNavBarLayoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    navBarContentHeightPx + statusBarHeight // Total height includes status bar
                ).apply {
                    gravity = Gravity.TOP
                }
                newNavBar.layoutParams = finalNavBarLayoutParams
                Log.d(TAG, "MiniAppActivity: finalNavBarLayoutParams.height = ${finalNavBarLayoutParams.height}")

                // IMPORTANT: Set NavigationBar's own top padding to 0.
                // Status bar offset for children will be handled by newNavBar.setExternalStatusBarHeight()
                newNavBar.setPadding(newNavBar.paddingLeft, 0, newNavBar.paddingRight, newNavBar.paddingBottom)

                // Inform NavigationBar about the status bar height for its internal child layout
                newNavBar.setExternalStatusBarHeight(statusBarHeight)

                newNavBar.setOnBackButtonClickListener { handleBackButtonClick() }

                // Ensure any old navigationBar instance is removed from its parent before reassigning
                if (navigationBar != null && navigationBar?.parent != null) {
                    (navigationBar?.parent as? ViewGroup)?.removeView(navigationBar)
                }

                navigationBar = newNavBar

                // Check if rootContainer is initialized before adding the view
                if (::rootContainer.isInitialized) {
                    rootContainer.addView(navigationBar, 0)
                    rootContainer.post {
                        Log.d(TAG, "MiniAppActivity: After layout pass, navigationBar.height = ${navigationBar?.height}, navigationBar.measuredHeight = ${navigationBar?.measuredHeight}")
                    }
                } else {
                    Log.e(TAG, "Unable to add NavigationBar: rootContainer not initialized")
                }
            }

            val titleText = config?.navigationBarTitleText ?: ""
            val backgroundColor = config?.navigationBarBackgroundColor ?: NavigationBarConfig.DEFAULT_BACKGROUND_COLOR
            val textStyle = config?.navigationBarTextStyle ?: "black"
            val textColor = if (textStyle == "white") Color.WHITE else Color.BLACK

            // Initial back button visibility depends only on whether animation is disabled (i.e., is it a tab switch?)
            val showBackButton = !disableAnimation
            Log.d(TAG, "Determined initial showBackButton: $showBackButton")

            // This runs after animation completes OR immediately if animation is disabled.
            val onAnimationEnd = Runnable {
                 // If navigating back to a tab root, hide the back button.
                 if (isBackNavigation) {
                     val currentPath = currentWebView?.currentPath ?: ""
                     val isNowOnTabRoot = tabBar?.findTabIndexByPath(currentPath) != -1
                     if (isNowOnTabRoot) {
                         Log.d(TAG, "Back nav to Tab Root ($currentPath) finished, hiding back button.")
                         navigationBar?.setBackButtonVisible(false)
                     }
                 }
                 updateLayoutMargins()
            }

            navigationBar?.updateStateAndAnimate(
                title = titleText,
                bgColor = backgroundColor,
                textColor = textColor,
                showBackButton = showBackButton,
                isBackNavigation = isBackNavigation,
                disableAnimation = disableAnimation,
                onBackClickListener = { handleBackButtonClick() }, // Pass the handler method reference
                onAnimationEnd = onAnimationEnd
            )

        } catch (e: Exception) {
            Log.e(TAG, "Error updating navigation bar", e)
        }
    }

    /**
     * Handles the click event from the NavigationBar's back button.
     */
    private fun handleBackButtonClick() {
        try {
            Log.d(TAG, "NavigationBar back button clicked")
            onBackPressedDispatcher.onBackPressed()
        } catch (e: Exception) {
            Log.e(TAG, "Error during back navigation: ${e.message}")
            finish() // Finish on error
        }
    }

    // Helper to calculate the Y translation based on visible bars
    private fun calculateWebViewTranslationY(): Float {
        val navBarOffset = if (navigationBar?.visibility == View.VISIBLE) {
            navigationBar?.height ?: 0
        } else {
            0
        }
        val tabBarOffset = if (tabBar?.visibility == View.VISIBLE && tabBar?.config?.position == TabBarConfig.Position.TOP) {
            tabBar?.height ?: 0
        } else {
            0
        }
        // Calculate the required vertical translation for the container
        return (navBarOffset + tabBarOffset).toFloat()
    }

    override fun finish() {
        // Notify Rust before ending the activity
        Log.d(TAG, "Activity finishing, notifying Rust: appId=$appId")
        notifyMiniAppClosed()

        // Ensure WebView is paused
        currentWebView?.pause()

        // Call the original finish method
        super.finish()
    }

    // Handle configuration changes to prevent Activity recreation
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        Log.d(TAG, "Configuration changed, updating layout")

        // Update layout to adapt to screen orientation changes
        updateLayoutMargins()

        // Reconfigure navigation bar if needed
        val pageConfig = MiniApp.getPageConfig(appId, currentWebView?.currentPath ?: "")
        updateNavigationBar(pageConfig, false, true)
    }
}
