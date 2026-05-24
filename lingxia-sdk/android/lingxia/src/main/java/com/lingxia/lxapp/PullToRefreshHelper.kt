package com.lingxia.lxapp

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.animation.DecelerateInterpolator
import android.widget.FrameLayout
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/**
 * LxApp-style pull-to-refresh.
 *
 * Architecture:
 * - Indicator is added to webViewContainer at index 0 (BEHIND WebView in z-order)
 * - When user pulls, only the WebView moves down via translationY
 * - This reveals the indicator area behind the WebView
 * - Indicator is positioned to stay centered in the revealed area
 * - NO bounce animation - smooth return only
 */
internal class PullToRefreshHelper(
    private val context: Context,
    private val webViewContainer: FrameLayout,
    private val onRefresh: () -> Unit
) {
    companion object {
        private const val TAG = "PullToRefresh"
        private const val TRIGGER_DISTANCE_DP = 80f
        private const val MAX_PULL_DISTANCE_DP = 150f
        private const val RUBBER_BAND_COEFFICIENT = 0.55f
    }

    private var isEnabled = true
    private var refreshIndicator: RefreshIndicator? = null
    private var isRefreshing = false
    private var isPulling = false
    private var startX = 0f
    private var startY = 0f
    private var currentPullDistance = 0f
    private var webView: View? = null
    private var returnAnimator: ValueAnimator? = null

    private val density = context.resources.displayMetrics.density
    private val triggerDistancePx = TRIGGER_DISTANCE_DP * density
    private val maxPullDistancePx = MAX_PULL_DISTANCE_DP * density
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    init {
        setupRefreshIndicator()
    }

    private fun currentWrapper(): View? {
        return webViewContainer.findViewWithTag<View>("current_webview_container")
            ?: (webView?.parent as? View)?.takeIf { it.parent == webViewContainer }
            ?: webView
    }

    private fun setupRefreshIndicator() {
        // Indicator is a fixed-height strip at the top
        val indicatorHeightPx = (MAX_PULL_DISTANCE_DP * density).toInt()

        refreshIndicator = RefreshIndicator(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                indicatorHeightPx
            ).apply {
                gravity = android.view.Gravity.TOP or android.view.Gravity.CENTER_HORIZONTAL
            }
            visibility = View.GONE  // Use GONE instead of INVISIBLE to not affect layout
            alpha = 0f
            // Explicitly set transparent background
            setBackgroundColor(Color.TRANSPARENT)
        }
        // Add indicator to webViewContainer at index 0 (behind WebView)
        webViewContainer.addView(refreshIndicator, 0)
    }

    fun attachToWebView(webView: View) {
        this.webView = webView

        if (webView is com.lingxia.lxapp.WebView) {
            webView.pullToRefreshCallback = { event ->
                handleTouch(webView, event)
            }
        }
    }

    fun setEnabled(enabled: Boolean) {
        isEnabled = enabled
        Log.d(TAG, "Pull-to-refresh enabled=$isEnabled")
        if (!isEnabled) {
            isRefreshing = false
            resetState()
        }
    }

    fun isEnabled(): Boolean = isEnabled

    private fun handleTouch(view: View, event: MotionEvent): Boolean {
        if (!isEnabled) return false
        if (event.pointerCount > 1) {
            resetState()
            return false
        }

        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                // Cancel any running animation and force reset state
                if (returnAnimator?.isRunning == true) {
                    returnAnimator?.cancel()
                    // Manually reset state since onAnimationEnd won't be called on cancel
                    resetState()
                }
                startX = event.rawX
                startY = event.rawY
                isPulling = false
                return false
            }

            MotionEvent.ACTION_MOVE -> {
                if (isRefreshing) return false

                val isAtTop = !view.canScrollVertically(-1)
                val deltaX = event.rawX - startX
                val deltaY = event.rawY - startY

                val isVerticalDrag = abs(deltaY) > abs(deltaX)

                if (!isPulling && isAtTop && deltaY > touchSlop && isVerticalDrag) {
                    isPulling = true
                }

                if (isPulling) {
                    val rawPull = max(0f, deltaY - touchSlop)

                    if (rawPull > 0) {
                        currentPullDistance = rubberBandClamp(rawPull, maxPullDistancePx)
                        updatePullState()
                        return true
                    } else {
                        isPulling = false
                        currentPullDistance = 0f
                        updatePullState()
                    }
                }

                return false
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (isPulling && !isRefreshing) {
                    if (currentPullDistance >= triggerDistancePx) {
                        startRefreshing()
                    } else {
                        animateToPosition(0f)
                    }
                    isPulling = false
                    return true
                }
                isPulling = false
            }
        }
        return false
    }

    /**
     * Rubber band effect: progressive resistance as you pull further.
     */
    private fun rubberBandClamp(distance: Float, maxDistance: Float): Float {
        val x = distance / maxDistance
        return maxDistance * (1f - kotlin.math.exp(-RUBBER_BAND_COEFFICIENT * x)) / (1f - kotlin.math.exp(-RUBBER_BAND_COEFFICIENT))
    }

    /**
     * Update visual state: move the WebView's container down, position indicator in revealed area.
     */
    private fun updatePullState() {
        // Find the WebView's wrapper container (the direct child of webViewContainer)
        // This is usually the FrameLayout tagged "current_webview_container"
        val webViewWrapper = currentWrapper()

        // Move the wrapper down - this reveals the indicator behind it
        webViewWrapper?.translationY = currentPullDistance

        refreshIndicator?.let { indicator ->
            if (currentPullDistance > 1f) {
                if (indicator.visibility != View.VISIBLE) {
                    indicator.visibility = View.VISIBLE
                }

                val progress = min(1f, currentPullDistance / triggerDistancePx)

                // Fade in
                indicator.alpha = min(1f, progress * 1.5f)

                // Indicator stays at top (translationY = 0), no need to move it
                // The webview wrapper moving down will reveal it
                indicator.translationY = 0f

                indicator.setPullProgress(progress, currentPullDistance)
            } else {
                indicator.visibility = View.GONE  // Use GONE to not affect layout
                indicator.alpha = 0f
                indicator.translationY = 0f
                indicator.setPullProgress(0f)
                webViewWrapper?.translationY = 0f
            }
        }
    }

    fun startRefreshing() {
        if (isRefreshing || !isEnabled) return

        // Check if WebView is attached
        if (currentWrapper() == null) {
            onRefresh()
            return
        }

        isRefreshing = true

        // Show indicator and start animation
        refreshIndicator?.let { indicator ->
            indicator.visibility = View.VISIBLE
            indicator.alpha = 1f
            indicator.startLoading()
        } ?: return

        // Hold at a comfortable position
        val refreshPosition = triggerDistancePx * 0.8f
        animateToPosition(refreshPosition)
        onRefresh()
    }

    fun endRefreshing() {
        if (!isRefreshing) return

        isRefreshing = false
        refreshIndicator?.stopLoading()
        animateToPosition(0f)
    }

    /**
     * Force reset all state - used when animation is cancelled.
     */
    private fun resetState() {
        currentPullDistance = 0f
        isPulling = false

        currentWrapper()?.translationY = 0f

        refreshIndicator?.apply {
            visibility = View.GONE
            alpha = 0f
            translationY = 0f
            setPullProgress(0f, 0f)
            stopLoading()  // stopLoading() handles isLoading check internally
        }
    }

    /**
     * Smooth animation to target position - NO bounce.
     */
    private fun animateToPosition(targetPosition: Float) {
        returnAnimator?.cancel()

        returnAnimator = ValueAnimator.ofFloat(currentPullDistance, targetPosition).apply {
            duration = 250
            interpolator = DecelerateInterpolator(2f)

            addUpdateListener { animation ->
                currentPullDistance = animation.animatedValue as Float
                updatePullState()
            }

            addListener(object : android.animation.AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: android.animation.Animator) {
                    if (targetPosition == 0f) {
                        resetState()
                    }
                }

                override fun onAnimationCancel(animation: android.animation.Animator) {
                    // Also reset on cancel to prevent stuck state
                    if (targetPosition == 0f) {
                        resetState()
                    }
                }
            })

            start()
        }
    }
}

/**
 * Minimal 3-dots indicator.
 */
private class RefreshIndicator(context: Context) : View(context) {
    private val density = context.resources.displayMetrics.density
    private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#888888")  // Neutral gray
        style = Paint.Style.FILL
    }

    private var progress = 0f
    private var isLoading = false
    private var startTime = 0L

    init {
        // Ensure no background
        setBackgroundColor(Color.TRANSPARENT)
        setWillNotDraw(false)
    }

    private val updateRunnable = object : Runnable {
        override fun run() {
            if (isLoading) {
                invalidate()
                postDelayed(this, 16L)
            }
        }
    }

    fun setPullProgress(p: Float, pullDistance: Float = 0f) {
        progress = p
        // Store the current pull distance to position dots correctly
        currentPullDistance = pullDistance
        invalidate()
    }

    private var currentPullDistance = 0f

    fun startLoading() {
        if (isLoading) return
        isLoading = true
        startTime = System.currentTimeMillis()
        post(updateRunnable)
    }

    fun stopLoading() {
        isLoading = false
        removeCallbacks(updateRunnable)
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        // Don't call super.onDraw to avoid default background drawing

        val cx = width / 2f
        // Position dots in the center of the currently revealed area
        val cy = if (currentPullDistance > 0f) {
            currentPullDistance / 2f
        } else {
            40f * density  // Default when loading programmatically
        }
        val dotRadius = 3.5f * density  // Slightly larger for visibility
        val spacing = 12f * density

        if (isLoading) {
            // Marquee animation: 3 dots lighting up sequentially
            val time = System.currentTimeMillis() - startTime
            val cycle = 600  // Full cycle duration in ms
            val phase = (time % cycle).toFloat() / cycle  // 0.0 to 1.0

            for (i in -1..1) {
                // Each dot has its turn in the cycle (3 dots = 1/3 cycle each)
                val dotIndex = i + 1  // 0, 1, 2
                val dotPhase = (phase * 3f - dotIndex).rem(3f)  // Stagger by 1/3 cycle
                
                // Fade in/out effect: bright when it's this dot's turn
                val brightness = if (dotPhase < 1f) {
                    // This dot's turn: fade in then stay bright
                    min(1f, dotPhase * 3f)
                } else {
                    // Not this dot's turn: dim
                    0.3f
                }
                
                dotPaint.alpha = (brightness * 255).toInt().coerceIn(75, 255)
                canvas.drawCircle(cx + i * spacing, cy, dotRadius, dotPaint)
            }
        } else {
            // Pull progress: 3 dots with scale animation
            val scale = progress.coerceIn(0f, 1f)
            // Make dots more visible - start at 50% alpha even at 0 progress
            dotPaint.alpha = ((100 + 155 * scale).toInt()).coerceIn(100, 255)

            for (i in -1..1) {
                // All 3 dots scale together, no stagger for simplicity
                // Slight bounce effect as you pull
                val dotScale = 0.5f + 0.5f * scale  // Scale from 0.5 to 1.0
                canvas.drawCircle(cx + i * spacing, cy, dotRadius * dotScale, dotPaint)
            }
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        stopLoading()
    }
}
