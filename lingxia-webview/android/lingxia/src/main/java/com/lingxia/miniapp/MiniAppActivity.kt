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

            // Explicitly set system bar colors to transparent
            activity.window.statusBarColor = Color.TRANSPARENT
            activity.window.navigationBarColor = Color.TRANSPARENT

            // Set status bar icon colors based on preference
            WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
                isAppearanceLightStatusBars = lightStatusBarIcons
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
                        val existingWebView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, targetPath)
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

        // Force navigationBar to null for recreations due to screen rotation
        navigationBar = null

        // Register broadcast receiver for close requests
        registerReceiver(closeAppReceiver, android.content.IntentFilter(ACTION_CLOSE_MINIAPP))

        // Register broadcast receiver for switch page requests
        registerReceiver(switchPageReceiver, android.content.IntentFilter(ACTION_SWITCH_PAGE))

        // Back press handler // Changed comment and added try-catch + visibility logic
        onBackPressedDispatcher.addCallback(object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                 try {
                    // Ensure current WebView stays visible
                    currentWebView?.visibility = View.VISIBLE // Changed from direct access to safe call

                    // Call Rust to handle back navigation
                    val result = nativeOnBackPressed(appId)
                    Log.d(TAG, "Back press handled by native: $result") // Added log

                    if (result > 0) {
                        return
                    }

                    // No back navigation available, close activity
                    Log.d(TAG, "No back navigation available, finishing") // Added log
                    finish()
                } catch (e: Exception) {
                    Log.e(TAG, "Error handling back press: ${e.message}")
                    // Ensure finish is called even on error
                    finish()
                }
            }
        })

        // Configure transparent system bars
        configureTransparentSystemBars(this)

        // Initialize appId from intent (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            Log.e(TAG, "Missing required parameter: appId")
            finish()
            return
        }
        // Initialize the new flag
        isDisplayingHomeMiniApp = (this.appId == MiniApp.HomeMiniAppId)

        val initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""

        // Get TabBar config from native layer
        val tabBarJson = MiniApp.nativeGetTabBarConfig(appId)
        val tabBarConfig = TabBarConfig.fromJson(tabBarJson)

        // Setup root container FIRST
        rootContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        setContentView(rootContainer)

        // Apply window insets as padding to the root container
        ViewCompat.setOnApplyWindowInsetsListener(rootContainer) { view, insets ->
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            view.setPadding(systemBars.left, 0, systemBars.right, systemBars.bottom)
            insets // Return original insets
        }

        // Setup WebView container
        setupWebViewContainer()

        // Setup TabBar
        setupTabBar(tabBarConfig)

        // Add capsule button on top (always present)
        addCapsuleButton()

        // Perform initial layout margin update (AFTER all UI setup)
        updateLayoutMargins()

        // Load initial WebView content
        setupWebViewContent(appId, initialPath)

        Log.d(TAG, "MiniAppActivity onCreate completed for appId: $appId, path: $initialPath")
    }

    private fun setupContainers() {
        // Create root container
        rootContainer = FrameLayout(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        setContentView(rootContainer)

        // Create WebView container
        webViewContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        rootContainer.addView(webViewContainer)
    }

    private fun setupTabBar(config: TabBarConfig?) {
        if (config == null) {
            Log.d(TAG, "Invalid or insufficient TabBar config, TabBar not shown.")
            return
        }

        if (tabBar == null) {
            // Create and add TabBar
            tabBar = TabBar(this).apply {
                setConfig(config)
                setOnTabSelectedListener { index, path ->
                    Log.d(TAG, "Tab clicked: index=$index, path=$path")
                    switchToTab(path)
                }
                // Apply layout params using the helper function
                applyTabBarLayoutParams(this, config)
            }
            // Add TabBar to the container AFTER the apply block completes
            rootContainer.addView(tabBar)
            Log.d(TAG, "TabBar added with ${config.list.size} items.")
        } else {
            // If TabBar already exists (e.g., during re-creation), just update its config
            tabBar?.setConfig(config)
            // Update layout params if necessary
            tabBar?.let { tb -> applyTabBarLayoutParams(tb, config) } // Use helper here
            Log.d(TAG, "TabBar config updated.")
        }

        // Initial margin update needed after TabBar is added/configured
        updateLayoutMargins()
    }

    private fun applyTabBarLayoutParams(tabBar: TabBar, config: TabBarConfig) {
        val isVertical = config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT
        val density = resources.displayMetrics.density
        val defaultTabBarSizePx = (DEFAULT_TAB_BAR_SIZE_DP * density).toInt()
        val verticalWidthPx = (DEFAULT_TAB_BAR_SIZE_DP * 1.0f * density).toInt() // Vertical TabBar width

        (tabBar.layoutParams as? FrameLayout.LayoutParams)?.apply {
            if (isVertical) {
                width = verticalWidthPx
                height = ViewGroup.LayoutParams.MATCH_PARENT
                gravity = when (config.position) {
                    TabBarConfig.Position.LEFT -> Gravity.START
                    TabBarConfig.Position.RIGHT -> Gravity.END
                    else -> Gravity.START // Should not happen
                }
            } else {
                width = ViewGroup.LayoutParams.MATCH_PARENT
                height = defaultTabBarSizePx
                gravity = when (config.position) {
                    TabBarConfig.Position.TOP -> Gravity.TOP
                    TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    else -> Gravity.BOTTOM // Should not happen
                }
            }
            // Re-assign layoutParams to ensure changes are applied, especially when modifying existing params.
            tabBar.layoutParams = this
        } ?: run {
            // Fallback for initial creation or if layoutParams are not FrameLayout.LayoutParams.
            val newLayoutParams = FrameLayout.LayoutParams(0,0)
            if (isVertical) {
                newLayoutParams.width = verticalWidthPx
                newLayoutParams.height = ViewGroup.LayoutParams.MATCH_PARENT
                newLayoutParams.gravity = when (config.position) {
                    TabBarConfig.Position.LEFT -> Gravity.START
                    TabBarConfig.Position.RIGHT -> Gravity.END
                    else -> Gravity.START // Should not happen
                }
            } else {
                newLayoutParams.width = ViewGroup.LayoutParams.MATCH_PARENT
                newLayoutParams.height = defaultTabBarSizePx
                newLayoutParams.gravity = when (config.position) {
                    TabBarConfig.Position.TOP -> Gravity.TOP
                    TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    else -> Gravity.BOTTOM // Should not happen
                }
            }
            tabBar.layoutParams = newLayoutParams
        }
    }

    private fun updateLayoutMargins() {
        val isTabBarVisible = tabBar?.visibility == View.VISIBLE
        val tabBarHeight = tabBar?.layoutParams?.height ?: 0
        val tabBarWidth = tabBar?.layoutParams?.width ?: 0

        // Adjust WebView container margins based on TabBar position
        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = 0
            bottomMargin = 0
            leftMargin = 0
            rightMargin = 0

            // Set margins based on TabBar position
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
                null -> { /* No TabBar config, do nothing */ }
            }

            webViewContainer.layoutParams = this
            // Request layout for the container itself
            webViewContainer.requestLayout()
        }

        // Apply translation to the current WebView CONTAINER
        val container = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
        container?.translationY = calculateWebViewTranslationY()
        container?.requestLayout()

        //Log.d(TAG, "Updated layout: bottomMargin=${(webViewContainer.layoutParams as FrameLayout.LayoutParams).bottomMargin}, containerTransY=${container?.translationY}")
    }

    // Helper function to find existing or create new WebView instance for a given path/page
    private fun findOrCreateWebViewForPage(appId: String, path: String): Pair<com.lingxia.miniapp.WebView?, NavigationBarConfig?> {
        var webView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, path)

        if (webView == null) {
            if (appId.isEmpty()) {
                Log.e(TAG, "findOrCreateWebViewForPage failed: Cannot create WebView, appId is missing.")
                return Pair(null, null)
            }
            webView = com.lingxia.miniapp.WebView.createWebView(
                context = this,
                appId = appId,
                path = path,
                visible = true
            )
        } else {
            Log.d(TAG, "Using existing WebView instance for page: $path")
        }

        // Get page config - Nav bar configuration is now handled by the caller
        val pageConfig = webView?.getPageConfig()

        return Pair(webView, pageConfig)
    }

    // Helper function to attach a WebView to the container and resume it
    private fun attachAndResumeWebView(view: com.lingxia.miniapp.WebView?) {
        if (view == null) {
            Log.e(TAG, "attachAndResumeWebView called with null view!")
            return
        }
        if (!isDestroyed) {
            Log.d(TAG, "Attaching and resuming WebView for path: ${view.currentPath}")

            // Check if this WebView was in an off-screen container
            val wasInOffScreenContainer = view.parent?.let { parent ->
                val parentView = parent as? ViewGroup
                parentView?.tag?.toString()?.startsWith("hiddenWebViewContainer_") == true &&
                parentView.translationX < 0  // Check if positioned off-screen
            } ?: false

            // Ensure view is visible (might have been set to GONE previously)
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

                // Now add to webview container
                webViewContainer.addView(view)

                // Only perform special handling for WebViews from off-screen containers
                if (wasInOffScreenContainer) {
                    Log.d(TAG, "WebView moved from off-screen container, ensuring proper layout")

                    // Use a simple post to ensure layout is applied after the view is added
                    view.post {
                        val containerWidth = webViewContainer.width
                        val containerHeight = webViewContainer.height

                        if (containerWidth > 0 && containerHeight > 0) {
                            Log.d(TAG, "Applying layout for WebView from off-screen container: ${containerWidth}x${containerHeight}")

                            // Use WebView's layout helper function
                            com.lingxia.miniapp.WebView.applyLayout(view, containerWidth, containerHeight)

                            // Simple invalidation to refresh the view
                            view.invalidate()
                            view.requestLayout()
                        }
                    }
                }
            } else {
                // If already in the container (e.g., initial load), ensure it's visible and resumed
                Log.d(TAG, "WebView for ${view.currentPath} already in container, ensuring resume.")
            }

            // Resume the WebView's activities
            view.resume()
        }
    }

    private fun setupWebViewContent(appId: String, path: String) {
        val initialWebView = findOrCreateWebViewForPage(appId, path)
        if (initialWebView.first == null) {
            Log.e(TAG, "Failed to find or create initial WebView for $path")
            finish(); return
        }

        // Handle the special delay logic if reusing the immediately previous WebView
        if (lastWebView?.get() == initialWebView.first) {
            pendingWebViewSetup = true
            webViewContainer.postDelayed({
                if (!isDestroyed) {
                    attachAndResumeWebView(initialWebView.first)
                    pendingWebViewSetup = false
                }
            }, 100)
        } else {
            // Attach and resume immediately for initial load or reuse of non-last view
            attachAndResumeWebView(initialWebView.first)
        }

        // Set the current WebView
        this.currentWebView = initialWebView.first

        // Update last used WebView reference
        lastWebView = WeakReference(initialWebView.first)
    }

    // Function to setup the FrameLayout that holds the WebViews
    private fun setupWebViewContainer() {
        webViewContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Margins will be set by updateLayoutMargins
        }
        rootContainer.addView(webViewContainer)
        Log.d(TAG, "WebView container added.")
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

        super.onDestroy()
    }

    // Handles switching ROOT pages associated with Tabs
    private fun switchToTab(targetPath: String) {
        Log.d(TAG, "Switching TAB to path: $targetPath, container children: ${webViewContainer.childCount}") // Added child count

        val appId = intent.getStringExtra(EXTRA_APP_ID)
        if (appId.isNullOrEmpty()) {
            Log.e(TAG, "switchToTab failed: Cannot get/create WebView, appId is missing.")
            return
        }

        // Bail early if trying to switch to the current path
        if (currentWebView?.currentPath == targetPath) {
            Log.d(TAG, "Already on this tab, no need to switch") // Added log
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
        val (targetWebView, pageConfig) = findOrCreateWebViewForPage(appId, targetPath)
        if (targetWebView == null) {
            Log.e(TAG, "switchToTab failed: findOrCreateWebViewForPage returned null for $targetPath")
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

        // Pre-position the target WebView correctly while still INVISIBLE
        // Add target view first if it's not already there
        if (targetWebView.parent != webViewContainer) {
            targetWebView.visibility = View.INVISIBLE // Keep invisible until layout pass

            if (targetWebView.parent != null) {
                (targetWebView.parent as? ViewGroup)?.removeView(targetWebView)
            }

            try {
                webViewContainer.addView(targetWebView)
                Log.d(TAG, "Added new WebView for $targetPath to container. Container now has ${webViewContainer.childCount} children")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to add WebView to container: ${e.message}")
                return
            }
        } else {
            targetWebView.bringToFront() // Ensure it's on top if reused
            Log.d(TAG, "WebView already in container, bringing to front")
        }

        // Post ensures layout calculations are done *before* we make it visible
        webViewContainer.post {
            if (isDestroyed) return@post

            // Apply translation based on current nav/tab bar state
            targetWebView.translationY = calculateWebViewTranslationY()

            // Make target visible and resume it
            targetWebView.visibility = View.VISIBLE
            targetWebView.resume()

            // Pause and remove the previous view *after* the new one is resumed and visible
            // Use a small delay to prevent visual flicker
            if (previousWebView != null && previousWebView != targetWebView) {
                webViewContainer.postDelayed({
                    if (isDestroyed) return@postDelayed
                    if (previousWebView.parent == webViewContainer) {
                         Log.d(TAG, "Pausing and removing previous tab WebView: ${previousWebView.currentPath}")
                         previousWebView.pause()
                         // Consider setting to GONE instead of INVISIBLE before remove
                         previousWebView.visibility = View.GONE
                         webViewContainer.removeView(previousWebView)
                         Log.d(TAG, "Container now has ${webViewContainer.childCount} children after removing previous WebView") // Added log

                         // Added update nav bar call after removal (disable animation)
                         updateNavigationBar(pageConfig, false, disableAnimation = true)
                    }
                }, 50) // Small delay to help smooth transition
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
            val (newWebView, pageConfig) = findOrCreateWebViewForPage(appId, targetPath)
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
                .start() // No complex end action needed here for new container

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
        val pageConfig = currentWebView?.getPageConfig()
        updateNavigationBar(pageConfig, false, true)
    }
}
