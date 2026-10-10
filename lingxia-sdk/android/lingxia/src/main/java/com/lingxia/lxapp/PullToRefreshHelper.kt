package com.lingxia.lxapp

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.os.SystemClock
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.view.animation.DecelerateInterpolator
import android.view.animation.LinearInterpolator
import android.widget.FrameLayout
import com.lingxia.app.Lingxia
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min

/**
 * LxApp-style pull-to-refresh.
 *
 * Architecture:
 * - Indicator is added to webViewContainer at index 0 (BEHIND the page in z-order)
 * - When user pulls, the page's wrapper moves down via translationY
 * - This reveals the indicator strip behind it; the strip itself is
 *   transparent, so the canvas colour behind the container shows through
 * - The spinner stays centred in the revealed area
 * - While refreshing the page holds part-way down, then springs back
 */
internal class PullToRefreshHelper(
    private val context: Context,
    private val webViewContainer: FrameLayout,
    /** Height at the top the spinner must stay clear of: the status bar on a page without a navigation bar. */
    private val topInset: () -> Int,
    private val spinnerColor: () -> Int,
    private val onRefresh: () -> Unit
) {
    companion object {
        private const val TAG = "PullToRefresh"
        private const val TRIGGER_DISTANCE_DP = 80f
        private const val MAX_PULL_DISTANCE_DP = 150f
        private const val RUBBER_BAND_COEFFICIENT = 0.55f
        // A refresh that ends at once would only flash the spinner.
        private const val MIN_VISIBLE_MS = 400L
    }

    private var isEnabled = true
    private var refreshIndicator: RefreshSpinnerView? = null
    private var isRefreshing = false
    private var isPulling = false
    private var startX = 0f
    private var startY = 0f
    private var currentPullDistance = 0f
    private var webView: View? = null
    // The wrapper last moved down, so a reset puts back the one it moved even
    // after a page swap made another wrapper current.
    private var movedWrapper: View? = null
    private var returnAnimator: ValueAnimator? = null
    private var shownAt = 0L
    // Set while the indicator lingers after a stop to honour MIN_VISIBLE_MS.
    private var pendingDismiss: Runnable? = null

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
        // A fixed-height, transparent strip at the top, as tall as the pull can go.
        refreshIndicator = RefreshSpinnerView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                maxPullDistancePx.toInt()
            ).apply {
                gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
            }
            visibility = View.GONE
            alpha = 0f
            setBackgroundColor(Color.TRANSPARENT)
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }
        ensureIndicatorAttached()
    }

    // The container is not ours: opening or closing an lxapp empties it
    // (prepareLxApp, closeLxApp), which takes the indicator out of the tree
    // while this helper survives. Own the attachment rather than relying on
    // the one-time add, or every later pull reveals an empty gap.
    private fun ensureIndicatorAttached() {
        val indicator = refreshIndicator ?: return
        if (indicator.parent === webViewContainer) return
        (indicator.parent as? ViewGroup)?.removeView(indicator)
        // Index 0 keeps it behind the page, which slides down to reveal it.
        webViewContainer.addView(indicator, 0)
    }

    fun attachToWebView(webView: View) {
        // One indicator serves every page of the activity, so a refresh ends
        // with the page that started it.
        if (isBusy() && this.webView !== webView) {
            abort()
        }
        this.webView = webView
        ensureIndicatorAttached()

        if (webView is com.lingxia.lxapp.WebView) {
            webView.pullToRefreshCallback = { event ->
                handleTouch(webView, event)
            }
        } else if (webView is com.lingxia.webview.LingXiaServoView) {
            webView.setTouchInterceptor { event -> handleTouch(webView, event) }
        }
    }

    fun setEnabled(enabled: Boolean) {
        isEnabled = enabled
        Log.d(TAG, "Pull-to-refresh enabled=$isEnabled")
        if (!isEnabled) {
            abort()
        }
    }

    fun isEnabled(): Boolean = isEnabled

    /** The indicator is busy: refreshing, or lingering after a stop. */
    private fun isBusy(): Boolean = isRefreshing || pendingDismiss != null

    private fun handleTouch(view: View, event: MotionEvent): Boolean {
        if (!isEnabled) return false
        if (event.pointerCount > 1) {
            if (!isBusy()) resetState()
            return false
        }

        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                // A released pull still springing back gives way to a new one;
                // a refresh in progress keeps its position.
                if (!isBusy() && returnAnimator?.isRunning == true) {
                    returnAnimator?.cancel()
                    resetState()
                }
                startX = event.rawX
                startY = event.rawY
                isPulling = false
                return false
            }

            MotionEvent.ACTION_MOVE -> {
                if (isBusy()) return false

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
                if (isPulling && !isBusy()) {
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
     * Update visual state: move the page's wrapper down by
     * [currentPullDistance] and centre the spinner in the revealed strip.
     */
    private fun updatePullState() {
        val wrapper = currentWrapper()
        if (wrapper !== movedWrapper) {
            movedWrapper?.translationY = 0f
            movedWrapper = wrapper
        }
        wrapper?.translationY = currentPullDistance

        val indicator = refreshIndicator ?: return
        if (currentPullDistance > 1f) {
            if (indicator.visibility != View.VISIBLE) {
                indicator.setSpinnerColor(spinnerColor())
                indicator.visibility = View.VISIBLE
            }
            val progress = min(1f, currentPullDistance / triggerDistancePx)
            indicator.alpha = if (isBusy()) 1f else min(1f, progress * 1.5f)
            indicator.setPullProgress(progress, currentPullDistance, topInset().toFloat())
        } else {
            indicator.visibility = View.GONE
            indicator.alpha = 0f
            indicator.setPullProgress(0f, 0f, 0f)
            wrapper?.translationY = 0f
        }
    }

    fun startRefreshing() {
        if (isRefreshing || !isEnabled) return

        if (pendingDismiss != null) {
            // Restarted while lingering: the spinner is still up, keep it.
            cancelPendingDismiss()
            isRefreshing = true
            announce(R.string.lx_pull_refresh_refreshing)
            onRefresh()
            return
        }

        val indicator = refreshIndicator
        if (indicator == null || currentWrapper() == null) {
            onRefresh()
            return
        }

        isRefreshing = true
        shownAt = SystemClock.uptimeMillis()
        indicator.setSpinnerColor(spinnerColor())
        indicator.visibility = View.VISIBLE
        indicator.alpha = 1f
        indicator.startLoading(reduceMotion())
        announce(R.string.lx_pull_refresh_refreshing)

        // Hold at a comfortable position
        // Hold below the status bar on a page without a navigation bar, so
        // the spinner has the same room there as under a navigation bar.
        animateToPosition(topInset() + triggerDistancePx * 0.8f)
        onRefresh()
    }

    /**
     * Ends the refresh. Returns at once; the indicator itself lingers until
     * it has been up for [MIN_VISIBLE_MS].
     */
    fun endRefreshing() {
        if (!isRefreshing) return
        isRefreshing = false

        val remaining = MIN_VISIBLE_MS - (SystemClock.uptimeMillis() - shownAt)
        if (remaining <= 0) {
            dismiss()
            return
        }
        val dismissal = Runnable {
            pendingDismiss = null
            dismiss()
        }
        pendingDismiss = dismissal
        webViewContainer.postDelayed(dismissal, remaining)
    }

    private fun dismiss() {
        refreshIndicator?.stopLoading()
        announce(R.string.lx_pull_refresh_refreshed)
        animateToPosition(0f)
    }

    private fun cancelPendingDismiss() {
        pendingDismiss?.let { webViewContainer.removeCallbacks(it) }
        pendingDismiss = null
    }

    /** Drop any refresh and put the page back now, without lingering. */
    private fun abort() {
        cancelPendingDismiss()
        isRefreshing = false
        returnAnimator?.apply {
            removeAllListeners()
            removeAllUpdateListeners()
            cancel()
        }
        resetState()
    }

    /**
     * Force reset all state - used when animation is cancelled.
     */
    private fun resetState() {
        currentPullDistance = 0f
        isPulling = false

        movedWrapper?.translationY = 0f
        movedWrapper = null
        currentWrapper()?.translationY = 0f

        refreshIndicator?.apply {
            visibility = View.GONE
            alpha = 0f
            setPullProgress(0f, 0f, 0f)
            stopLoading()
        }
    }

    private fun announce(resId: Int) {
        webViewContainer.announceForAccessibility(Lingxia.localizedString(context, resId))
    }

    /** "Remove animations": the spinner holds still instead of turning. */
    private fun reduceMotion(): Boolean =
        Settings.Global.getFloat(
            context.contentResolver,
            Settings.Global.ANIMATOR_DURATION_SCALE,
            1f
        ) == 0f

    /**
     * Smooth animation to target position - NO bounce.
     */
    private fun animateToPosition(targetPosition: Float) {
        // Replaced, not ended: a return to 0 cut short by a new refresh must
        // not reset the indicator the new refresh has just started.
        returnAnimator?.apply {
            removeAllListeners()
            removeAllUpdateListeners()
            cancel()
        }

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
 * The transparent strip behind the page with a circular spinner centred in
 * its revealed part: an arc that grows with the pull, then turns while
 * loading. Drawn straight on the strip, like the system indicator on iOS.
 */
private class RefreshSpinnerView(context: Context) : View(context) {
    private val density = context.resources.displayMetrics.density
    private val radius = 10f * density
    private val arcPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = 2.5f * density
        strokeCap = Paint.Cap.ROUND
    }
    private val arcBounds = RectF()

    private var progress = 0f
    private var revealed = 0f
    private var inset = 0f
    private var isLoading = false
    private var isStatic = false
    private var phase = 0f
    private var spinAnimator: ValueAnimator? = null

    init {
        setWillNotDraw(false)
    }

    fun setSpinnerColor(color: Int) {
        arcPaint.color = color
        invalidate()
    }

    /**
     * [p] is the pull progress to the trigger; [pullDistance] the revealed
     * height; [topInset] the part of it the status bar covers.
     */
    fun setPullProgress(p: Float, pullDistance: Float, topInset: Float) {
        progress = p.coerceIn(0f, 1f)
        revealed = pullDistance
        inset = topInset
        invalidate()
    }

    fun startLoading(reduceMotion: Boolean) {
        if (isLoading) return
        isLoading = true
        isStatic = reduceMotion
        phase = 0f
        if (!reduceMotion) {
            spinAnimator = ValueAnimator.ofFloat(0f, 1f).apply {
                duration = 1332L
                repeatCount = ValueAnimator.INFINITE
                interpolator = LinearInterpolator()
                addUpdateListener {
                    phase = it.animatedValue as Float
                    invalidate()
                }
                start()
            }
        }
        invalidate()
    }

    fun stopLoading() {
        spinAnimator?.cancel()
        spinAnimator = null
        isLoading = false
        isStatic = false
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        val cx = width / 2f
        // Centred in the strip the page has uncovered, below the status bar
        // once the strip is taller than it.
        val cy = when {
            revealed <= 0f -> 32f * density
            revealed > inset -> inset + (revealed - inset) / 2f
            else -> revealed / 2f
        }
        arcBounds.set(cx - radius, cy - radius, cx + radius, cy + radius)

        if (isLoading && !isStatic) {
            // One turn per cycle while the arc breathes between 20 and 270 degrees.
            val sweep = 20f + 250f * (0.5f - 0.5f * cos(phase * 2f * Math.PI).toFloat())
            val start = phase * 720f - 90f
            arcPaint.alpha = 255
            canvas.drawArc(arcBounds, start, sweep, false, arcPaint)
        } else if (isLoading) {
            arcPaint.alpha = 255
            canvas.drawArc(arcBounds, -90f, 270f, false, arcPaint)
        } else {
            // Pulling: the arc grows to 270 degrees and turns slightly; it is
            // fully opaque once the pull would trigger.
            arcPaint.alpha = if (progress >= 1f) 255 else (90 + 120 * progress).toInt()
            canvas.drawArc(arcBounds, -90f + 90f * progress, 270f * progress, false, arcPaint)
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        stopLoading()
    }
}
