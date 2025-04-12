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

class MiniAppActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"
        const val EXTRA_TAB_BAR_CONFIG = "tabBarConfig"
        internal const val DEFAULT_NAV_BAR_HEIGHT_DP = 44
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
    }

    private lateinit var appId: String
    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    private var tabBar: TabBar? = null
    private var navigationBar: NavigationBar? = null
    private var isDestroyed = false
    private var pendingWebViewSetup = false

    // Tracks the currently visible WebView instance
    private var currentWebView: com.lingxia.miniapp.WebView? = null

    // Broadcast receiver for receiving mini app close requests
    private val closeAppReceiver = object : android.content.BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == "com.lingxia.CLOSE_MINIAPP_ACTION") {
                val targetAppId = intent.getStringExtra("appId")
                if (::appId.isInitialized && targetAppId == appId) {
                    Log.d(TAG, "Received close request for appId: $appId")
                    finish()
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Register broadcast receiver for close requests
        registerReceiver(closeAppReceiver, android.content.IntentFilter("com.lingxia.CLOSE_MINIAPP_ACTION"))

        // Handle back button presses with the modern approach
        onBackPressedDispatcher.addCallback(object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                // Notify the Rust side to handle back press and determine behavior based on return value
                val result = nativeOnBackPressed(appId)

                // If Rust side has handled the back press, no further action needed
                if (result > 0) {
                    return
                }

                // If Rust side didn't handle it, finish the activity
                finish()
            }
        })

        // Enable Edge-to-Edge using WindowCompat
        WindowCompat.setDecorFitsSystemWindows(window, false)
        // Explicitly set system bar colors to transparent
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT // Optional, for consistency
        // Set status bar icon colors
        setStatusBarAppearance(/* isLight= */ true)

        // Initialize appId from intent (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            Log.e(TAG, "Missing required parameter: appId")
            finish()
            return // Exit onCreate if appId is missing
        }

        val initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""
        val tabBarJson = intent.getStringExtra(EXTRA_TAB_BAR_CONFIG)
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

    // Helper to set status bar icon color (optional)
    private fun setStatusBarAppearance(isLight: Boolean) {
        // Use the newer approach to control appearance
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = isLight
        }
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
                    performWebViewSwitch(path)
                }
                // Apply layout params
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                ).apply {
                    gravity = when (config.position) {
                        TabBarConfig.Position.TOP -> Gravity.TOP
                        TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    }
                }
            }
            // Add TabBar to the container AFTER the apply block completes
            rootContainer.addView(tabBar)
            Log.d(TAG, "TabBar added with ${config.list.size} items.")
        } else {
            // If TabBar already exists (e.g., during re-creation), just update its config
            tabBar?.setConfig(config)
            Log.d(TAG, "TabBar config updated.")
        }

        // Initial margin update needed after TabBar is added/configured
        updateLayoutMargins()

        // Demo: Show TabBar API usage after a short delay
        // rootContainer.postDelayed({
            // Demo API calls - keep or remove as needed
            // tabBar?.showTabBarRedDot(1)
            // tabBar?.setTabBarBadge(2, "99+")
        // }, 500)
    }

    private fun updateLayoutMargins() {
        val isTabBarVisible = tabBar?.visibility == View.VISIBLE
        val isNavBarVisible = navigationBar?.visibility == View.VISIBLE
        val tabBarHeight = tabBar?.layoutParams?.height ?: 0
        val navBarHeight = navigationBar?.layoutParams?.height ?: 0

        // Adjust WebView container margins ONLY based on Nav/Tab bars
        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            // Allow webview to extend all the way to the top
            // This means it will be drawn under the status bar
            topMargin = 0

            // NavBar now includes status bar height and is positioned at the top
            val navBarOffset = if (isNavBarVisible) {
                navBarHeight
            } else {
                0
            }

            // If we have a visible TabBar at the top, add its height
            val tabBarOffset = if (isTabBarVisible && tabBar?.config?.position == TabBarConfig.Position.TOP) {
                tabBarHeight
            } else {
                0
            }

            // Apply the appropriate translation to move content below visible UI elements
            currentWebView?.translationY = (navBarOffset + tabBarOffset).toFloat()

            // Bottom margin is TabBar height if visible and at bottom, otherwise 0
            bottomMargin = if (isTabBarVisible && tabBar?.config?.position == TabBarConfig.Position.BOTTOM) {
                tabBarHeight
            } else {
                0
            }

            webViewContainer.layoutParams = this
            webViewContainer.requestLayout()
            Log.d(TAG, "Updated webViewContainer margins and translations: navBarOffset=$navBarOffset, tabBarOffset=$tabBarOffset, bottom=$bottomMargin")
        }
    }

    // Helper function to find existing or create new WebView instance for a given path/page
    private fun findOrCreateWebViewForPage(appId: String, path: String): Pair<com.lingxia.miniapp.WebView?, NavigationBarConfig?> {
        var webView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, path)

        if (webView == null) {
            if (appId.isEmpty()) {
                Log.e(TAG, "findOrCreateWebViewForPage failed: Cannot create WebView, appId is missing.")
                return Pair(null, null)
            }
            webView = com.lingxia.miniapp.WebView(this).apply {
                handleWebViewCreated(appId, path)
            }
        } else {
            Log.d(TAG, "Reusing existing WebView instance for page: $path")
            (webView.parent as? ViewGroup)?.removeView(webView)
        }

        // Get and apply page config
        val pageConfig = webView?.getPageConfig()
        pageConfig?.let { config ->
            // Create navigation bar if it doesn't exist and page needs it
            if (navigationBar == null && !config.hidden) {
                val navBarHeight = DEFAULT_NAV_BAR_HEIGHT_DP * resources.displayMetrics.density
                val statusBarHeight = getStatusBarHeight(this)

                navigationBar = NavigationBar(this).apply {
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        navBarHeight.toInt() + statusBarHeight
                    ).apply {
                        gravity = Gravity.TOP
                        topMargin = 0
                    }
                    setPadding(paddingLeft, statusBarHeight, paddingRight, paddingBottom)
                }
                rootContainer.addView(navigationBar)
            }

            // Configure navigation bar based on page config
            navigationBar?.let { navBar ->
                config.navigationBarTitleText?.let { navBar.setTitle(it) }
                val textColor = if (config.navigationBarTextStyle == "white") Color.WHITE else Color.BLACK
                config.navigationBarBackgroundColor?.let { bgColor ->
                    navBar.setColor(bgColor, textColor)
                }
                navBar.visibility = if (config.hidden) View.GONE else View.VISIBLE
                updateLayoutMargins()
            }
        }

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
            // Ensure view is visible (might have been set to GONE previously)
            view.visibility = View.VISIBLE

            // Add to webview container if not already added
            if (view.parent != webViewContainer) {
                // We already removed from old parent in findOrCreateWebViewForPage if reused
                webViewContainer.addView(view)
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

        // Unregister broadcast receiver
        try {
            unregisterReceiver(closeAppReceiver)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to unregister receiver: ${e.message}")
        }

        currentWebView?.let { view ->
            Log.d(TAG, "Cleaning up current WebView in onDestroy")
            view.pause()
            webViewContainer.removeView(view)
            view.visibility = View.GONE
        }
        currentWebView = null
        super.onDestroy()
    }

    // Core logic to switch WebView views
    private fun performWebViewSwitch(targetPath: String) {
        // Keep essential start log
        Log.d(TAG, "Performing WebView switch for path: $targetPath")

        val appId = intent.getStringExtra(EXTRA_APP_ID)
        if (appId.isNullOrEmpty()) {
             Log.e(TAG, "performWebViewSwitch failed: Cannot get/create WebView, appId is missing.")
             return
        }

        // Bail early if trying to switch to the current path
        if (currentWebView?.currentPath == targetPath) {
            Log.d(TAG, "Already on path $targetPath, ignoring switch")
            return
        }

        // Capture reference to previous WebView before changing anything
        val previousWebView = currentWebView

        // First prep the UI changes before touching WebViews
        val targetIndex = tabBar?.findTabIndexByPath(targetPath) ?: -1

        // Find or create target WebView (do this before UI updates to allow pre-positioning)
        val (targetWebView, pageConfig) = findOrCreateWebViewForPage(appId, targetPath)
        if (targetWebView == null) {
            Log.e(TAG, "performWebViewSwitch failed: findOrCreateWebViewForPage returned null for $targetPath")
            return
        }

        // Get page-specific navigation bar config
        val shouldShowNavBar = pageConfig?.hidden != true

        // Cache current and target nav bar visibility states
        val navBarWasVisible = navigationBar?.visibility == View.VISIBLE
        val navBarShouldBeVisible = shouldShowNavBar

        // Pre-position the target WebView correctly while still INVISIBLE
        if (targetWebView.parent != webViewContainer) {
            targetWebView.visibility = View.INVISIBLE
            webViewContainer.addView(targetWebView)
            targetWebView.translationY = if (shouldShowNavBar) {
                (navigationBar?.height ?: 0).toFloat()
            } else {
                0f
            }
        }

        // Set current WebView to target for tracking
        currentWebView = targetWebView

        // CRITICAL: Update navigation bar BEFORE making target WebView visible
        navigationBar?.let { navBar ->
            if (navBarShouldBeVisible != navBarWasVisible) {
                if (shouldShowNavBar) {
                    pageConfig?.let { config ->
                        config.navigationBarTitleText?.let { navBar.setTitle(it) }
                        config.navigationBarBackgroundColor?.let { bgColor ->
                            val textColor = if (config.navigationBarTextStyle == "white") Color.WHITE else Color.BLACK
                            navBar.setColor(bgColor, textColor)
                        }
                    }
                }
                // In tab switching scenario, back button should always be hidden
                navBar.setBackButtonVisible(false)
                navBar.visibility = if (navBarShouldBeVisible) View.VISIBLE else View.GONE
                updateLayoutMargins()
            }
        }

        // Use a post with zero delay to ensure UI thread completes current layout pass
        webViewContainer.post {
            // Make target WebView visible AFTER navigation bar and layout updated
            targetWebView.visibility = View.VISIBLE

            // Resume the target WebView
            targetWebView.resume()

            // Create a secondary post to remove previous view AFTER new view is visible
            // This creates a slight overlap that prevents flashing
            webViewContainer.postDelayed({
                if (previousWebView != null && previousWebView != targetWebView) {
                    previousWebView.pause()
                    previousWebView.visibility = View.INVISIBLE // Hide before removing
                    webViewContainer.removeView(previousWebView)
                }
            }, 50) // Small delay helps with transition
        }

        // Update TabBar UI first (without triggering listener)
        tabBar?.setSelectedIndex(targetIndex, notifyListener = false)
    }

    // Public function for programmatic switching (e.g., from native side)
    fun switchTab(targetPath: String) {
        val targetIndex = tabBar?.findTabIndexByPath(targetPath) ?: -1
        if (targetIndex == -1) {
            Log.e(TAG, "switchToTab failed: Path '$targetPath' not found in TabBar items.")
            return
        }

        val currentSelectedIndex = tabBar?.getSelectedIndex() ?: -1
        if (isDestroyed || targetIndex == currentSelectedIndex) {
            Log.w(TAG, "switchTab ignored: destroyed=$isDestroyed or targetIndex == currentSelectedIndex ($targetIndex == $currentSelectedIndex)")
            return
        }

        // Update TabBar UI first (without triggering listener)
        tabBar?.setSelectedIndex(targetIndex, notifyListener = false)

        // Perform the actual view switching logic
        performWebViewSwitch(targetPath)
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
}
