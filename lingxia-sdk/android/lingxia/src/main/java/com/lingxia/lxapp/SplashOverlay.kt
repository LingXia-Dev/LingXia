package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.graphics.drawable.Drawable
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import androidx.core.content.ContextCompat

/**
 * Runtime half of the launch screen: covers the home activity from creation
 * until the home page's first render, showing the full-screen splash image
 * (aspect-fill) over the background color the static launch window used.
 * Dismissed by the runtime's onHomeFirstReady signal, with a timeout fallback
 * so a broken page never leaves it stuck.
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash_image` drawable); when
 * the color is absent the overlay is disabled entirely.
 */
internal object SplashOverlay {
    private const val TIMEOUT_MS = 6_000L
    private const val FADE_MS = 250L

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

    /**
     * Second beat of the cover (via [LxApp.applySplashCover]): the host's Rust
     * hook picked a different image for this launch. Selection needs the
     * initialized runtime, so it necessarily lands after the bundled cover is
     * already up — crossfade so the swap reads as intentional. Ignored once
     * dismissal has begun.
     */
    fun applyCover(path: String) {
        mainHandler.post {
            val container = overlay as? FrameLayout ?: return@post
            val drawable = Drawable.createFromPath(path) ?: run {
                android.util.Log.w("SplashOverlay", "splash cover unreadable: $path")
                return@post
            }
            val incoming = ImageView(container.context).apply {
                setImageDrawable(drawable)
                scaleType = ImageView.ScaleType.CENTER_CROP
                alpha = 0f
            }
            container.addView(
                incoming,
                FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
            incoming.animate().alpha(1f).setDuration(FADE_MS).start()
        }
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

    /** Theme- and time-dependent art is the runtime hook's job; this only loads what the build produced. */
    private fun resolveImage(context: Context): Drawable? {
        val id = context.resources.getIdentifier(
            "lingxia_splash_image", "drawable", context.packageName
        )
        if (id == 0) return null
        return ContextCompat.getDrawable(context, id)
    }
}
