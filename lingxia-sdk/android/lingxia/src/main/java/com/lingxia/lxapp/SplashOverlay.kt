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
    /// The campaign's fade-in and the face's lift. Both match iOS and Harmony
    /// to the millisecond: one launch experience, three renderers.
    private const val FADE_MS = 200L
    private const val DISMISS_MS = 300L

    /**
     * Fallback minimum time the cover must be *seen*, used only when the
     * configured hold cannot be read (the runtime is not up, so there is no
     * app config yet). Matches the framework default.
     */
    private const val DEFAULT_MIN_VISIBLE_MS = 600L

    private val mainHandler = Handler(Looper.getMainLooper())
    private var overlay: View? = null
    private var shownThisProcess = false
    private var homeReadySeen = false

    /** One launch face per process, shared by the bootstrap and home activities. */
    private var launchResolved = false
    private var launchBackground: Int? = null
    private var launchCover: Drawable? = null

    /** Whether the runtime has been told this launch has a face. */
    private var launchFaceMarked = false

    /** The campaign countdown's tick, so dismissal can stop it. */
    private var campaignTick: Runnable? = null

    /** Exact campaign deadline; the visible counter is deliberately separate. */
    private var campaignDismiss: Runnable? = null

    /** The launch watchdog, held by reference so a campaign can call it off. */
    private var watchdog: Runnable? = null

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
        val (view, _) = buildCoverView(activity, cover)
        activity.setContentView(view)
        hookReveal(activity)
        android.util.Log.i("SplashOverlay", "bootstrap cover shown")
        return true
    }

    /**
     * Tell the runtime the launch face is up, once the native library exists.
     *
     * The face itself was already drawn from resources — nothing here can
     * change it, and nothing should: art picked at runtime cannot match a
     * frame the system composed before this process started. What the runtime
     * learns is which appearance bucket that frame came from, so a campaign
     * can match it. A pinned appearance reaches the bucket itself through
     * [UiModeManager], set when the user picks it.
     */
    fun markLaunchFace(activity: Activity) {
        if (launchFaceMarked || launchBackground == null) return
        launchFaceMarked = true
        val dark = (activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        runCatching { NativeApi.splashMarkLaunchFace(dark) }
    }

    /**
     * Runtime signal (via [LxApp.showSplashCampaign]): the home page is ready
     * and the host has a screen of its own to show first. The launch layer
     * stays up and takes the campaign's art with a skippable countdown, so
     * there is never a gap between the two screens.
     */
    fun showCampaign(imagePath: String, durationMs: Int) {
        mainHandler.post { startCampaign(imagePath, durationMs) }
    }

    private fun startCampaign(imagePath: String, durationMs: Int) {
        val art = Drawable.createFromPath(imagePath)
        if (art == null) {
            homeReadySeen = true
            dismiss()
            return
        }
        val frame = (overlay as? ViewGroup) ?: createCampaignFrame()
        if (frame == null) {
            homeReadySeen = true
            dismiss()
            return
        }
        // Placeholder-only launches hold Android's system splash instead of
        // creating an app overlay. Build the campaign frame first, then release
        // that hold: the system frame exits directly onto the campaign rather
        // than silently dropping a valid host choice.
        homeReadySeen = true
        val image = ImageView(frame.context).apply {
            setImageDrawable(art)
            scaleType = ImageView.ScaleType.CENTER_CROP
            alpha = 0f
        }
        frame.addView(
            image,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        )
        val skip = SplashSkipButton(frame.context).apply {
            alpha = 0f
            seconds = Math.max(1, Math.ceil(durationMs / 1000.0).toInt())
            setOnClickListener { dismiss(force = true) }
        }
        frame.addView(skip, skip.topEndLayoutParams())
        // Fades in, unlike the launch face: this beat is content arriving, and
        // a cut here would read as the launch stuttering.
        image.animate().alpha(1f).setDuration(FADE_MS).start()
        skip.animate().alpha(1f).setDuration(FADE_MS).start()

        // The campaign owns the screen from here, with a bounded countdown of
        // its own, so the launch watchdog must not fire underneath it.
        watchdog?.let { mainHandler.removeCallbacks(it) }
        watchdog = null

        val tick = object : Runnable {
            override fun run() {
                if (skip.seconds > 1) {
                    skip.seconds -= 1
                    mainHandler.postDelayed(this, 1_000L)
                }
            }
        }
        campaignTick = tick
        mainHandler.postDelayed(tick, 1_000L)
        val deadline = Runnable { dismiss(force = true) }
        campaignDismiss = deadline
        mainHandler.postDelayed(deadline, durationMs.coerceAtLeast(0).toLong())
    }

    private fun createCampaignFrame(): FrameLayout? {
        val activity = LxApp.getCurrentActivity() ?: return null
        val decor = activity.window?.decorView as? ViewGroup ?: return null
        return FrameLayout(activity).also { frame ->
            launchBackground?.let(frame::setBackgroundColor)
            frame.isClickable = true
            decor.addView(
                frame,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
            overlay = frame
        }
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
        val watchdog = Runnable { dismiss(force = true) }
        this.watchdog = watchdog
        mainHandler.postDelayed(watchdog, TIMEOUT_MS)
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
     * the configured hold, re-measured from that draw, absorbs the small overlap.
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

    /**
     * The configured hold, in milliseconds.
     *
     * The core holds the ready signal for this long too, but it starts its
     * clock where the runtime learns the cover is going up — and on Android
     * that is under the system splash window, which on 12+ covers the
     * activity (overlay included) until first frame and then runs its own
     * exit. Whatever of the hold elapses in there is time the user did not
     * spend looking at the cover, so the overlay measures the same duration
     * again from the cover's own first draw.
     *
     * Read here rather than cached at attach: it resolves from the app config,
     * which the runtime loads long after the bootstrap activity drew the
     * cover. By the time a non-forced dismissal can happen the runtime has
     * signalled home-ready, so the value is there.
     */
    private fun minVisibleMs(): Long = runCatching { NativeApi.splashMinDurationMs() }
        .getOrDefault(DEFAULT_MIN_VISIBLE_MS)
        .coerceAtLeast(0L)

    private fun dismiss(force: Boolean = false) {
        campaignTick?.let { mainHandler.removeCallbacks(it) }
        campaignTick = null
        campaignDismiss?.let { mainHandler.removeCallbacks(it) }
        campaignDismiss = null
        watchdog?.let { mainHandler.removeCallbacks(it) }
        watchdog = null
        val view = overlay ?: return
        if (!force) {
            // Hold until the cover has been on screen long enough — the
            // system splash may still be covering it, or has only just left.
            val minVisible = minVisibleMs()
            val since = android.os.SystemClock.uptimeMillis() - visibleAt
            if (visibleAt < 0 || since < minVisible) {
                val delay = if (visibleAt < 0) 150L else minVisible - since
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

/**
 * The campaign's countdown, and the only way past it. A skip control that is
 * hard to hit is the same as no skip control, so it is a full pill with a
 * generous tap target, clear of the status bar.
 */
private class SplashSkipButton(context: Context) : android.widget.TextView(context) {
    private val skipLabel: String = context.resources
        .getIdentifier("lx_splash_skip", "string", context.packageName)
        .takeIf { it != 0 }
        ?.let { context.getString(it) }
        ?: "Skip"

    var seconds: Int = 0
        set(value) {
            field = value
            text = "$skipLabel ${Math.max(0, value)}"
        }

    init {
        setTextColor(android.graphics.Color.WHITE)
        textSize = 13f
        val pad = (12 * resources.displayMetrics.density).toInt()
        val padV = (6 * resources.displayMetrics.density).toInt()
        setPadding(pad, padV, pad, padV)
        background = android.graphics.drawable.GradientDrawable().apply {
            cornerRadius = 14 * resources.displayMetrics.density
            setColor(android.graphics.Color.argb(102, 0, 0, 0))
        }
        isClickable = true
    }

    /** Top-trailing, clear of the status bar the launch layer draws under. */
    fun topEndLayoutParams(): FrameLayout.LayoutParams {
        val margin = (16 * resources.displayMetrics.density).toInt()
        val top = (statusBarHeight() + 12 * resources.displayMetrics.density).toInt()
        return FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply {
            gravity = android.view.Gravity.TOP or android.view.Gravity.END
            marginEnd = margin
            topMargin = top
        }
    }

    private fun statusBarHeight(): Float {
        val id = resources.getIdentifier("status_bar_height", "dimen", "android")
        return if (id > 0) resources.getDimensionPixelSize(id).toFloat() else 0f
    }
}
