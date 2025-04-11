package com.lingxia.miniapp

import android.app.Activity
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

class MiniAppActivity : Activity() {
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"
        const val EXTRA_TAB_BAR_CONFIG = "tabBarConfig"
        private const val DEFAULT_TAB_BAR_SIZE_DP = 56

        private var lastWebView: WeakReference<com.lingxia.miniapp.WebView>? = null

        // Native method for handling mini app hidden event
        @JvmStatic
        external fun nativeOnMiniAppHidden(appId: String, path: String): Int
    }

    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    private var tabBar: TabBar? = null
    private var isDestroyed = false
    private var pendingWebViewSetup = false

    // Tracks the currently visible WebView instance
    private var currentWebView: com.lingxia.miniapp.WebView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        try {
            // Configure transparent status bar and navigation bar
            MiniApp.configureTransparentSystemBars(
                activity = this,
                lightStatusBars = true,
                lightNavigationBars = false,
                showStatusBars = true,
                showNavigationBars = false
            )

            // Get required parameters, finish if missing
            val appId = intent.getStringExtra(EXTRA_APP_ID)
            val path = intent.getStringExtra(EXTRA_PATH) ?: ""

            if (appId.isNullOrEmpty()) {
                Log.e(TAG, "Missing required parameter: appId")
                finish()
                return
            }

            Log.d(TAG, "Creating WebView for appId: $appId, path: $path")

            // Create and setup layout containers
            setupContainers()

            // Setup TabBar if config exists
            setupTabBar(intent.getStringExtra(EXTRA_TAB_BAR_CONFIG))

            // Add capsule button
            addCapsuleButton()

            // Setup WebView
            setupWebViewContent(appId, path)

        } catch (e: Exception) {
            Log.e(TAG, "Error in onCreate: ${e.message}")
            finish()
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

    private fun setupTabBar(configJson: String?) {
        if (configJson.isNullOrEmpty()) {
            return
        }

        try {
            val config = TabBarConfig.fromJson(configJson)
            if (config == null) {
                Log.d(TAG, "Invalid or insufficient TabBar config")
                return
            }

            // Create and add TabBar
            tabBar = TabBar(this).apply {
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                ).apply {
                    gravity = when (config.position) {
                        TabBarConfig.Position.TOP -> Gravity.TOP
                        TabBarConfig.Position.BOTTOM -> Gravity.BOTTOM
                    }
                }
                setConfig(config)

                // Set visibility change listener
                setOnVisibilityChangedListener { isVisible ->
                    updateWebViewContainerMargins(config.position, isVisible)
                }

                // Set tab selection listener
                setOnTabSelectedListener { index, path ->
                    // When user clicks a tab, directly perform the switch logic
                    performWebViewSwitch(path)
                }
            }
            // Add TabBar to the container AFTER the apply block completes
            rootContainer.addView(tabBar)

            // Initial margin update
            updateWebViewContainerMargins(config.position, config.visible)

            // Demo: Show TabBar API usage after a short delay to ensure TabBar is fully initialized
            rootContainer.postDelayed({
                tabBar?.let { bar ->
                    if (bar.visibility == View.VISIBLE) {
                        bar.showTabBarRedDot(0)
                        bar.setTabBarBadge(2, "98")
                    }
                }
            }, 500)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to setup TabBar: ${e.message}")
        }
    }

    private fun updateWebViewContainerMargins(position: TabBarConfig.Position, isTabBarVisible: Boolean) {
        (webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            if (isTabBarVisible) {
                when (position) {
                    TabBarConfig.Position.TOP -> {
                        topMargin = (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                        bottomMargin = 0
                    }
                    TabBarConfig.Position.BOTTOM -> {
                        topMargin = 0
                        bottomMargin = (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                    }
                }
            } else {
                topMargin = 0
                bottomMargin = 0
            }
            webViewContainer.layoutParams = this
        }
    }

    // Helper function to find existing or create new WebView instance for a given path/page
    private fun findOrCreateWebViewForPage(appId: String, path: String): com.lingxia.miniapp.WebView? {
        var webView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, path)

        if (webView == null) {
            if (appId.isEmpty()) {
                 Log.e(TAG, "findOrCreateWebViewForPage failed: Cannot create WebView, appId is missing.")
                 return null
            }
            webView = com.lingxia.miniapp.WebView(this).apply {
                 handleWebViewCreated(appId, path)
            }
        } else {
             Log.d(TAG, "Reusing existing WebView instance for page: $path")
             (webView.parent as? ViewGroup)?.removeView(webView)
        }
        return webView
    }

    // Helper function to attach a WebView to the container and resume it
    private fun attachAndResumeWebView(view: com.lingxia.miniapp.WebView?) {
        if (view == null) {
            Log.e(TAG, "attachAndResumeWebView called with null view!")
            return
        }
        if (!isDestroyed) {
            Log.d(TAG, "Attaching and resuming WebView for path: ${view.currentPath}") // Assuming WebView has currentPath property
            // Ensure view is visible (might have been set to GONE previously)
            view.visibility = View.VISIBLE

            // Add to webview container if not already added
            if (view.parent != webViewContainer) {
                // We already removed from old parent in findOrCreateWebViewForPage if reused
                webViewContainer.addView(view)
            } else {
                // If already in the container (e.g., initial load), ensure it's visible and resumed
                Log.w(TAG, "WebView for ${view.currentPath} already in container, ensuring resume.")
            }

            // Resume the WebView's activities
            view.resume()
        }
    }

    private fun setupWebViewContent(appId: String, path: String) {
        val initialWebView = findOrCreateWebViewForPage(appId, path)
        if (initialWebView == null) {
            Log.e(TAG, "Failed to find or create initial WebView for $path")
            finish(); return
        }

        // Handle the special delay logic if reusing the immediately previous WebView
        if (lastWebView?.get() == initialWebView) {
            pendingWebViewSetup = true
            webViewContainer.postDelayed({
                if (!isDestroyed) {
                    attachAndResumeWebView(initialWebView)
                    pendingWebViewSetup = false
                }
            }, 100)
        } else {
            // Attach and resume immediately for initial load or reuse of non-last view
            attachAndResumeWebView(initialWebView)
        }

        // Set the current WebView
        this.currentWebView = initialWebView

        // Update last used WebView reference
        lastWebView = WeakReference(initialWebView)
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
        // Create capsule container
        val capsule = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL

            // Set capsule background
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                setColor(Color.parseColor("#FFFFFF"))
                cornerRadius = 20f * resources.displayMetrics.density
                setStroke((0.5f * resources.displayMetrics.density).toInt(), Color.parseColor("#DDDDDD"))
            }

            // Set capsule layout parameters
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                (36 * resources.displayMetrics.density).toInt()
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                // Consider status bar height
                val statusBarHeight = resources.getIdentifier("status_bar_height", "dimen", "android")
                    .takeIf { it > 0 }
                    ?.let { resources.getDimensionPixelSize(it) }
                    ?: (24 * resources.displayMetrics.density).toInt()

                topMargin = statusBarHeight + (8 * resources.displayMetrics.density).toInt()
                rightMargin = (12 * resources.displayMetrics.density).toInt()
            }

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
                handleMiniAppHidden()
                finish()
            }
        }

        // Add views to capsule
        capsule.addView(btnMore)
        capsule.addView(divider)
        capsule.addView(btnClose)

        // Add capsule to root container to ensure it's always on top
        rootContainer.addView(capsule)
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

    private fun handleMiniAppHidden() {
        intent.getStringExtra(EXTRA_APP_ID)?.let { appId ->
            intent.getStringExtra(EXTRA_PATH)?.let { path ->
                nativeOnMiniAppHidden(appId, path)
            }
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        handleMiniAppHidden()
        currentWebView?.pause()
        finish()
    }

    override fun onDestroy() {
        isDestroyed = true
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

        val targetWebView = findOrCreateWebViewForPage(appId, targetPath)
        if (targetWebView == null) {
            Log.e(TAG, "performWebViewSwitch failed: findOrCreateWebViewForPage returned null for $targetPath")
            return
        }

        val previousWebView = currentWebView

        // Update the current WebView state *before* manipulating views
        currentWebView = targetWebView

        // Switch views in the container
        if (previousWebView != targetWebView && previousWebView != null) {
            previousWebView.pause()
            webViewContainer.removeView(previousWebView)
        }

        // Use helper function to attach and resume the target WebView
        attachAndResumeWebView(targetWebView)
        Log.d(TAG, "Perform WebView switch completed for path: $targetPath")
    }

    // Public function for programmatic switching (e.g., from Rust via wx.switchTab)
    // Renamed from switchToTab
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
}
