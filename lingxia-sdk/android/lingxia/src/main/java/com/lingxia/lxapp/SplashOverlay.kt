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
import com.lingxia.app.NativeApi

/**
 * Runtime half of the launch screen: renders the cover — bundled, or the
 * hook's pick for this launch — as the app's very first frame, so the OS
 * splash (brand color + app icon) exits straight onto it and the launch
 * reads "tap the icon, see the cover". The runtime boots underneath the
 * cover, never in front of it: the bootstrap activity shows the cover as its
 * own content before initialization, and the home activity re-covers itself
 * until the home page's first render. With no cover configured the first
 * draw is suspended instead, so the OS splash stays up until home is ready.
 * Released by the runtime's onHomeFirstReady signal, with a timeout fallback
 * so a broken page never leaves it stuck.
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash_image` drawable); when
 * the color is absent the launch screen is disabled entirely.
 */
internal object SplashOverlay {
    private const val TIMEOUT_MS = 6_000L
    private const val FADE_MS = 250L
    private const val DISMISS_MS = 300L

    /**
     * Minimum time the cover must be *seen*. The core already holds the ready
     * signal, but it measures from process start — on Android 12+ the system
     * splash window covers the activity (overlay included) until first frame,
     * so that hold can elapse while the cover is still hidden and the user
     * never sees it. Measured here from the cover's first draw, which the
     * splash's default exit follows within a beat.
     */
    private const val MIN_VISIBLE_MS = 600L

    private val mainHandler = Handler(Looper.getMainLooper())
    private var overlay: View? = null
    private var shownThisProcess = false
    private var homeReadySeen = false

    /** One launch face per process, shared by the bootstrap and home activities. */
    private var launchResolved = false
    private var launchBackground: Int? = null
    private var launchCover: Drawable? = null

    /** The bootstrap cover's image, so a late hook pick can land on it. */
    private var bootstrapImage: ImageView? = null
    private var selectionDone = false

    /** Whether the system-splash reveal is already being tracked. */
    private var revealHooked = false

    /** Uptime when the cover became visible; unset while the system splash covers it. */
    private var visibleAt = -1L

    /** Whether this cold start is showing a cover (drives seamless activity handoff). */
    fun coverActive(): Boolean = launchCover != null && !homeReadySeen

    /** Queued for the moment the cover starts lifting. */
    private val onCoverGone = mutableListOf<() -> Unit>()

    /** Queued for after the cover is off the screen entirely. */
    private val onCoverRemoved = mutableListOf<() -> Unit>()

    /** True from the cover's first frame until it is off the screen. */
    private var lifting = false

    /**
     * Whether the cover still covers the screen — including the fade-out,
     * where it is no longer *the* overlay but is very much still visible.
     * Anything drawing underneath during that window has to match it.
     */
    fun coverOnScreen(): Boolean = overlay != null || lifting

    /**
     * Defer [action] until the cover starts lifting, and report whether it was
     * deferred. The cover is full-bleed, so while it is up the system bars
     * belong to it, not to the page underneath — the host reads the return
     * value to know it should leave them alone for now.
     *
     * Returns false, having run nothing, when no cover is on screen.
     */
    fun doOnCoverGone(action: () -> Unit): Boolean {
        if (overlay == null) return false
        onCoverGone += action
        return true
    }

    /**
     * Defer [action] until the cover has left the screen, and report whether
     * it was deferred. The frame the cover is detached on is the one where
     * nothing else may have composited yet, so whatever shows through then
     * must still match the cover — restore it only after this fires.
     */
    fun doAfterCoverRemoved(action: () -> Unit): Boolean {
        if (!coverOnScreen()) return false
        onCoverRemoved += action
        return true
    }

    /**
     * Bootstrap half, called before anything native exists: make the bundled
     * cover the bootstrap activity's own content, from resources alone, so
     * the first frame the system splash exits onto is already the cover —
     * while the native library load, the hook, and the runtime boot all run
     * underneath it. Without a cover the first draw is suspended instead —
     * the system splash (brand color + app icon) stays put. Returns whether
     * a cover is on screen.
     */
    fun attachBootstrap(activity: Activity): Boolean {
        resolveResources(activity)
        if (launchBackground == null || homeReadySeen) return false
        val cover = launchCover
        if (cover == null) {
            holdFirstDraw(activity)
            return false
        }
        val (view, image) = buildCoverView(activity, cover)
        bootstrapImage = image
        activity.setContentView(view)
        hookReveal(activity)
        android.util.Log.i("SplashOverlay", "bootstrap cover shown")
        return true
    }

    /**
     * Deferred half of the launch decision: the hook needs the native
     * library, so it runs with the boot — under the cover, never in front of
     * it. A pick that differs from the bundled cover lands with a quick
     * crossfade, the same beat as a campaign layer arriving over a brand
     * frame.
     */
    fun applyHookSelection(activity: Activity) {
        if (selectionDone || launchBackground == null) return
        selectionDone = true
        val dark = (activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val dataDir = activity.applicationContext.filesDir.absolutePath
        val picked = NativeApi.splashSelectCover(dataDir, dark)
        if (picked.isEmpty()) return
        val drawable = Drawable.createFromPath(picked) ?: return
        launchCover = drawable
        val image = bootstrapImage ?: return
        image.animate()
            .alpha(0f)
            .setDuration(FADE_MS / 2)
            .withEndAction {
                image.setImageDrawable(drawable)
                image.animate().alpha(1f).setDuration(FADE_MS / 2).start()
            }
            .start()
        android.util.Log.i("SplashOverlay", "hook cover applied")
    }

    /** Attach over the home activity on a cold start when splash resources exist. */
    fun attachIfNeeded(activity: Activity, appId: String) {
        if (shownThisProcess || homeReadySeen) return
        if (appId != LxApp.homeAppId) {
            android.util.Log.i(
                "SplashOverlay",
                "skipped: appId=$appId home=${LxApp.homeAppId}"
            )
            return
        }
        resolveResources(activity)
        if (launchBackground == null) {
            android.util.Log.i("SplashOverlay", "skipped: no splash background resource")
            return
        }
        val decor = activity.window?.decorView as? ViewGroup
        if (decor == null) {
            android.util.Log.i("SplashOverlay", "skipped: no decor view")
            return
        }

        val cover = launchCover
        if (cover == null) {
            // No cover configured: suspend the first draw so the OS splash
            // (brand color + app icon) stays on screen until the home page
            // is ready — the launcher zoom, the splash, and its exit into
            // real content are then all composed by the OS.
            holdFirstDraw(activity)
            shownThisProcess = true
            android.util.Log.i("SplashOverlay", "holding system splash until home ready")
            return
        }

        // The cover rides this activity's very first frame too, so the
        // handoff from the bootstrap activity underneath is invisible.
        val (view, _) = buildCoverView(activity, cover)
        decor.addView(
            view,
            ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        )
        overlay = view
        shownThisProcess = true
        hookReveal(activity)
        android.util.Log.i("SplashOverlay", "cover overlay shown")
        mainHandler.postDelayed({ dismiss(force = true) }, TIMEOUT_MS)
    }

    /** Runtime signal (via [LxApp.onHomeFirstReady]): home page rendered its first frame. */
    fun notifyHomeReady() {
        homeReadySeen = true
        mainHandler.post { dismiss() }
    }

    /**
     * Full bleed over the brand color, fully opaque — the system splash's
     * own exit is the only transition onto it, so nothing may still be
     * fading when that reveal happens.
     */
    private fun buildCoverView(activity: Activity, cover: Drawable): Pair<View, ImageView> {
        val image = ImageView(activity).apply {
            setImageDrawable(cover)
            scaleType = ImageView.ScaleType.CENTER_CROP
        }
        val frame = FrameLayout(activity).apply {
            launchBackground?.let { setBackgroundColor(it) }
            // Swallow touches so nothing underneath is tappable while visible.
            isClickable = true
            addView(
                image,
                FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
        }
        return frame to image
    }

    /**
     * Resolve the launch face from resources alone, once per process: the
     * background color and the bundled cover. Deliberately no native calls —
     * this runs on the first-frame path, where every millisecond is the
     * system splash lingering on screen.
     */
    private fun resolveResources(activity: Activity) {
        if (launchResolved) return
        launchResolved = true
        launchBackground = resolveBackgroundColor(activity)
        if (launchBackground == null) return
        launchCover = resolveBundledCover(activity)
    }

    /**
     * Track when the cover reaches the user's eyes: its first draw. The
     * system splash may sit over it a beat longer, but it exits by its own
     * default animation — never by an app-taken exit listener, which some
     * OEM splash implementations simply never invoke, leaving the splash
     * stuck on screen over a cover no one sees. The splash is a plain
     * brand-color frame (blank icon slot), so its default exit is invisible;
     * [MIN_VISIBLE_MS] absorbs the small overlap.
     */
    private fun hookReveal(activity: Activity) {
        if (revealHooked) return
        revealHooked = true
        val content = activity.findViewById<View>(android.R.id.content)
        content.viewTreeObserver.addOnPreDrawListener(
            object : android.view.ViewTreeObserver.OnPreDrawListener {
                override fun onPreDraw(): Boolean {
                    content.viewTreeObserver.removeOnPreDrawListener(this)
                    if (visibleAt < 0) {
                        visibleAt = android.os.SystemClock.uptimeMillis()
                    }
                    return true
                }
            }
        )
    }

    /** Suspend an activity's first draw until home is ready (or the timeout). */
    private fun holdFirstDraw(activity: Activity) {
        val content = activity.findViewById<View>(android.R.id.content)
        val start = android.os.SystemClock.uptimeMillis()
        content.viewTreeObserver.addOnPreDrawListener(
            object : android.view.ViewTreeObserver.OnPreDrawListener {
                override fun onPreDraw(): Boolean {
                    val timedOut =
                        android.os.SystemClock.uptimeMillis() - start > TIMEOUT_MS
                    if (homeReadySeen || timedOut) {
                        content.viewTreeObserver.removeOnPreDrawListener(this)
                        return true
                    }
                    return false
                }
            }
        )
    }

    private fun dismiss(force: Boolean = false) {
        val view = overlay ?: return
        if (!force) {
            // Hold until the cover has been on screen long enough — the
            // system splash may still be covering it, or has only just left.
            val since = android.os.SystemClock.uptimeMillis() - visibleAt
            if (visibleAt < 0 || since < MIN_VISIBLE_MS) {
                val delay = if (visibleAt < 0) 150L else MIN_VISIBLE_MS - since
                mainHandler.postDelayed({ dismiss() }, delay)
                return
            }
        }
        overlay = null
        lifting = true
        onCoverGone.forEach { it() }
        onCoverGone.clear()
        // Everything the cover holds — the `lifting` flag, and the deferred
        // canvas/bar restores queued in `onCoverRemoved` — must end exactly
        // once, and the restores must actually run. The old drain never did:
        // it was a `view.post` issued right after `removeView`, and a detached
        // view's `post` (API 24+) parks the runnable in the view's run queue
        // until its next attach — which for a removed cover is never. So the
        // launch colour owned the canvas for the rest of the process, and
        // every navigation bared a near-black strip where the bars move.
        var finished = false
        val finishDismiss = finishDismiss@{
            if (finished) return@finishDismiss
            finished = true
            lifting = false
            (view.parent as? ViewGroup)?.removeView(view)
            // A beat later, so the page beneath has composited and the restore
            // cannot be seen — via the main handler, never the removed view.
            val drain = Runnable {
                onCoverRemoved.forEach { it() }
                onCoverRemoved.clear()
            }
            if (view.isAttachedToWindow) view.post(drain) else mainHandler.post(drain)
        }
        // The cover lifts away: a slight zoom under the fade reads as depth —
        // the home page is beneath it, not after it.
        view.animate()
            .alpha(0f)
            .scaleX(1.06f)
            .scaleY(1.06f)
            .setDuration(DISMISS_MS)
            .setInterpolator(android.view.animation.DecelerateInterpolator())
            .withEndAction { finishDismiss() }
            .start()
        // Hardening, not the fix: should any future path cancel the fade
        // before its end action, this still ends the cover's tenure exactly
        // once. A completed fade has already finished and this is a no-op.
        mainHandler.postDelayed({ finishDismiss() }, DISMISS_MS + 100L)
    }

    /** The launch background, or null when this host configures no splash. */
    fun backgroundColor(context: Context): Int? = resolveBackgroundColor(context)

    private fun resolveBackgroundColor(context: Context): Int? {
        val id = context.resources.getIdentifier(
            "lingxia_splash_background", "color", context.packageName
        )
        if (id == 0) return null
        return ContextCompat.getColor(context, id)
    }

    private fun resolveBundledCover(context: Context): Drawable? {
        val id = context.resources.getIdentifier(
            "lingxia_splash_image", "drawable", context.packageName
        )
        if (id == 0) return null
        return ContextCompat.getDrawable(context, id)
    }
}
