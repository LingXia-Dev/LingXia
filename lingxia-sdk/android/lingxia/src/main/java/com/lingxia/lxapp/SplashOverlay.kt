package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.content.res.Configuration
import android.graphics.drawable.Drawable
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import androidx.core.content.ContextCompat
import java.io.File

/**
 * Runtime half of the launch screen: covers the home activity from creation
 * until the home page's first render, showing the full-screen splash image
 * (aspect-fill) over the background color the static launch window used.
 * Dismissed by the runtime's onHomeFirstReady signal, with a timeout fallback
 * so a broken page never leaves it stuck.
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash_image` drawable); when
 * the color is absent the overlay is disabled entirely. An online-updated
 * image dropped under `<filesDir>/lingxia/splash/{light,dark}.png` takes
 * precedence over the bundled drawable on the next launch.
 */
internal object SplashOverlay {
    private const val TIMEOUT_MS = 6_000L
    private const val FADE_MS = 250L
    private const val CACHE_DIR = "lingxia/splash"

    private val mainHandler = Handler(Looper.getMainLooper())
    private var overlay: View? = null
    private var shownThisProcess = false
    private var homeReadySeen = false

    /** Attach over the home activity on a cold start when splash resources exist. */
    fun attachIfNeeded(activity: Activity, appId: String) {
        if (shownThisProcess || homeReadySeen) return
        if (appId != LxApp.homeAppId) return
        val backgroundColor = resolveBackgroundColor(activity) ?: return
        val decor = activity.window?.decorView as? ViewGroup ?: return

        val view = FrameLayout(activity).apply {
            setBackgroundColor(backgroundColor)
            // Swallow touches so nothing underneath is tappable while visible.
            isClickable = true
            resolveImage(activity)?.let { splash ->
                addView(
                    ImageView(activity).apply {
                        setImageDrawable(splash)
                        scaleType = ImageView.ScaleType.CENTER_CROP
                    },
                    FrameLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                )
            }
        }
        decor.addView(
            view,
            ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        )
        overlay = view
        shownThisProcess = true
        mainHandler.postDelayed({ dismiss() }, TIMEOUT_MS)
    }

    /** Runtime signal (via [LxApp.onHomeFirstReady]): home page rendered its first frame. */
    fun notifyHomeReady() {
        homeReadySeen = true
        mainHandler.post { dismiss() }
    }

    private fun dismiss() {
        val view = overlay ?: return
        overlay = null
        view.animate()
            .alpha(0f)
            .setDuration(FADE_MS)
            .withEndAction { (view.parent as? ViewGroup)?.removeView(view) }
            .start()
    }

    private fun resolveBackgroundColor(context: Context): Int? {
        val id = context.resources.getIdentifier(
            "lingxia_splash_background", "color", context.packageName
        )
        if (id == 0) return null
        return ContextCompat.getColor(context, id)
    }

    private fun resolveImage(context: Context): Drawable? {
        val dark = isNight(context)
        val cached = buildList {
            if (dark) add("dark.png")
            add("light.png")
        }
            .map { File(context.filesDir, "$CACHE_DIR/$it") }
            .firstOrNull { it.exists() }
        if (cached != null) {
            Drawable.createFromPath(cached.absolutePath)?.let { return it }
        }
        val id = context.resources.getIdentifier(
            "lingxia_splash_image", "drawable", context.packageName
        )
        if (id == 0) return null
        return ContextCompat.getDrawable(context, id)
    }

    private fun isNight(context: Context): Boolean {
        val mode = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
        return mode == Configuration.UI_MODE_NIGHT_YES
    }
}
