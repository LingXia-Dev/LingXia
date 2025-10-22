package com.lingxia.lxapp.util

import android.provider.Settings
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Helpers for dealing with bottom navigation bar insets.
 */
object WindowInsetsUtils {

    @JvmStatic
    /**
     * Insets that content should respect. Gesture navigation returns 0 so UI stays flush,
     * while three-button navigation adds the bar height plus a small cushion.
     */
    fun getBottomInset(container: View): Int {
        val insets = ViewCompat.getRootWindowInsets(container) ?: return 0
        return resolveContentInset(container, insets)
    }

    /**
     * Insets for sizing backgrounds that are allowed to extend under the navbar. Gesture
     * navigation still returns 0; three-button navigation keeps the full bar height so the
     * surface can cover it completely.
     */
    @JvmStatic
    fun getStableBottomInset(container: View): Int {
        val insets = ViewCompat.getRootWindowInsets(container) ?: return 0
        return resolveStableInset(container, insets)
    }

    @JvmStatic
    fun getEffectiveContentInset(container: View): Int {
        val insets = ViewCompat.getRootWindowInsets(container) ?: return 0
        return resolveContentInset(container, insets)
    }

    @JvmStatic
    fun applyBottomMargin(container: View, target: View, baseBottomMarginPx: Int = 0) {
        fun setMargin(bottomInset: Int) {
            val lp = target.layoutParams
            if (lp is ViewGroup.MarginLayoutParams) {
                val desired = baseBottomMarginPx + bottomInset
                if (lp.bottomMargin != desired) {
                    lp.bottomMargin = desired
                    target.layoutParams = lp
                }
            }
        }

        val initial = getEffectiveContentInset(container)
        setMargin(initial)
        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val resolved = resolveContentInset(container, insets)
            setMargin(resolved)
            insets
        }
        // Ensure we actually request insets when attached
        if (ViewCompat.isAttachedToWindow(container)) {
            ViewCompat.requestApplyInsets(container)
        } else {
            container.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    v.removeOnAttachStateChangeListener(this)
                    ViewCompat.requestApplyInsets(v)
                }
                override fun onViewDetachedFromWindow(v: View) {}
            })
        }

        // Also re-evaluate after first layout pass
        container.post {
            val postInset = getEffectiveContentInset(container)
            setMargin(postInset)
        }
    }

    @JvmStatic
    fun applyBottomPadding(container: View, target: View, basePaddingBottomPx: Int = 0) {
        fun setPadding(bottomInset: Int) {
            val desired = basePaddingBottomPx + bottomInset
            if (target.paddingBottom != desired) {
                target.setPadding(target.paddingLeft, target.paddingTop, target.paddingRight, desired)
            }
        }

        setPadding(getEffectiveContentInset(container))
        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            setPadding(resolveContentInset(container, insets))
            insets
        }
        ViewCompat.requestApplyInsets(container)
    }

    private fun resolveContentInset(view: View, insets: WindowInsetsCompat): Int {
        if (isGestureNavigation(view, insets)) return 0

        val visible = visibleInset(insets)
        val stable = stableInset(insets)
        val fallback = navBarHeightFallback(view)
        val candidate = maxOf(visible, stable, fallback, 0)
        return candidate + extraSpacingPx(view)
    }

    private fun resolveStableInset(view: View, insets: WindowInsetsCompat): Int {
        if (isGestureNavigation(view, insets)) return 0
        val stable = stableInset(insets)
        return maxOf(stable, navBarHeightFallback(view), 0)
    }

    private fun isGestureNavigation(view: View, insets: WindowInsetsCompat): Boolean {
        val navMode = resolveNavigationMode(view)
        val navVisible = insets.isVisible(WindowInsetsCompat.Type.navigationBars())
        val gestureInset = insets.getInsets(WindowInsetsCompat.Type.systemGestures()).bottom
        val visible = visibleInset(insets)
        val stable = stableInset(insets)

        // Prefer explicit mode hints when available
        when (navMode) {
            2 -> {
                return true                // gesture navigation
            }
            0, 1 -> {
                return false               // 3-button (or legacy 2-button) navigation
            }
        }

        // Heuristics when navMode is unavailable/unknown (no measurement fallback)
        if (stable > 0 || (navVisible && visible > 0)) return false
        return gestureInset > 0
    }

    private fun visibleInset(insets: WindowInsetsCompat): Int {
        val nav = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
        return maxOf(nav, bars, 0)
    }

    private fun stableInset(insets: WindowInsetsCompat): Int {
        val nav = insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.navigationBars()).bottom
        val bars = insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.systemBars()).bottom
        return maxOf(nav, bars, 0)
    }

    private fun navBarHeightFallback(view: View): Int {
        val candidates = listOf(
            "navigation_bar_height",
            "navigation_bar_height_default",
            "navigation_bar_frame_height",
            "navigation_bar_height_landscape"
        )
        for (name in candidates) {
            val resId = view.resources.getIdentifier(name, "dimen", "android")
            if (resId > 0) {
                val value = view.resources.getDimensionPixelSize(resId)
                if (value > 0) {
                    return value
                }
            }
        }
        return approxNavHeight(view)
    }

    private fun approxNavHeight(view: View): Int {
        val density = view.resources.displayMetrics.density
        return (52f * density + 0.5f).toInt()
    }

    private fun extraSpacingPx(view: View): Int {
        val density = view.resources.displayMetrics.density
        return (density * 4f + 0.5f).toInt()
    }

    private fun resolveNavigationMode(view: View): Int? {
        val contentResolver = view.context.contentResolver
        var mode: Int? = null
        try {
            mode = Settings.Secure.getInt(contentResolver, "navigation_mode")
        } catch (_: Settings.SettingNotFoundException) {
        } catch (_: SecurityException) {
        }
        if (mode == null) {
            mode = navBarInteractionModeFallback(view)
        }
        // Do not aggressively remap OEM-specific values; leave unknowns as null for heuristics.
        return mode
    }

    private fun navBarInteractionModeFallback(view: View): Int? {
        val resId = view.resources.getIdentifier("config_navBarInteractionMode", "integer", "android")
        return if (resId > 0) view.resources.getInteger(resId) else null
    }
}
