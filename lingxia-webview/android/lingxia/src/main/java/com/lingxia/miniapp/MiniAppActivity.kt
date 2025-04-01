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

        private var lastWebView: WeakReference<com.lingxia.miniapp.WebView>? = null
    }

    private var webView: com.lingxia.miniapp.WebView? = null
    private lateinit var rootContainer: FrameLayout
    private lateinit var webViewContainer: FrameLayout
    private var isDestroyed = false
    private var pendingWebViewSetup = false

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

            // Add capsule button
            addCapsuleButton()

            // Try to get existing WebView, create new one if not available
            webView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, path)?.also { existingWebView ->
                Log.d(TAG, "Reusing existing WebView for appId: $appId")
                // Remove from previous parent view
                (existingWebView.parent as? ViewGroup)?.removeView(existingWebView)

                // If this is the last used WebView, wait a moment before setting up
                if (lastWebView?.get() == existingWebView) {
                    pendingWebViewSetup = true
                    webViewContainer.postDelayed({
                        if (!isDestroyed) {
                            setupWebView(existingWebView, path)
                            pendingWebViewSetup = false
                        }
                    }, 100)
                } else {
                    setupWebView(existingWebView, path)
                }
            } ?: com.lingxia.miniapp.WebView(this).apply {
                Log.d(TAG, "Creating new WebView for appId: $appId")
                registerWebViewToNative(appId, path)
                setupWebView(this, null)
            }

            // Update last used WebView
            webView?.let { view ->
                lastWebView = WeakReference(view)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error in onCreate: ${e.message}")
            e.printStackTrace()
            finish()
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
            val radius = bounds.width() / 4.2f  // Adjust circle size

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

    private fun setupWebView(view: com.lingxia.miniapp.WebView, path: String?) {
        if (!isDestroyed) {
            // Reset WebView state
            view.visibility = View.VISIBLE

            // Set new path
            intent.getStringExtra(EXTRA_APP_ID)?.let { appId ->
                if (!path.isNullOrEmpty()) {
                    view.registerWebViewToNative(appId, path)
                }
            }

            // Add to webview container
            if (view.parent != webViewContainer) {
                webViewContainer.addView(view)
            }

            // Resume WebView
            view.resume()
        }
    }

    override fun onResume() {
        super.onResume()
        if (!pendingWebViewSetup) {
            webView?.visibility = View.VISIBLE
            webViewContainer.visibility = View.VISIBLE
            webView?.resume()
        }
    }

    override fun onPause() {
        super.onPause()
        webView?.pause()
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        webView?.pause()
        finish()
    }

    override fun onDestroy() {
        isDestroyed = true
        webView?.let { view ->
            view.pause()
            webViewContainer.removeView(view)
            view.visibility = View.GONE
        }
        webView = null
        super.onDestroy()
    }
}
