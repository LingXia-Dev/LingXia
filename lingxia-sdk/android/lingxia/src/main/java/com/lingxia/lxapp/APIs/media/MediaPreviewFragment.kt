package com.lingxia.lxapp.APIs.media

import android.app.ActivityManager
import android.content.ComponentCallbacks2
import android.content.res.Configuration
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.TypedValue
import android.util.LruCache
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.exifinterface.media.ExifInterface
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import com.lingxia.app.LxApp
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.R
import java.io.File
import java.io.ByteArrayOutputStream
import java.lang.ref.WeakReference
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.Future
import kotlin.math.max
import org.json.JSONObject

internal class MediaPreviewFragment : Fragment() {
    private enum class PreviewAdvance {
        MANUAL,
        NEXT,
        LOOP;

        companion object {
            fun fromRaw(value: String?): PreviewAdvance = when (value?.trim()?.lowercase()) {
                "next" -> NEXT
                "loop" -> LOOP
                else -> MANUAL
            }
        }
    }

    private var viewPager: ViewPager2? = null
    private var previewAdapter: PreviewPagerAdapter? = null
    private var indicatorText: TextView? = null
    private var closeButton: ImageButton? = null
    private var pageChangeCallback: ViewPager2.OnPageChangeCallback? = null
    private var totalItems: Int = 0
    private var windowUiSnapshot: ImmersiveWindowUi.Snapshot? = null
    private var previewItems: List<PreviewItem> = emptyList()
    private var callbackId: Long = 0L
    /**
     * Callback id for the "first pixel composited" signal that backs the
     * JS-side `PreviewMediaHandle.presented` Promise. Fired exactly once
     * for the first item that becomes visually ready (image loaded or video
     * first-frame). Zero once signaled, to make the fire idempotent.
     */
    private var presentedCallbackId: Long = 0L
    private var currentIndex: Int = 0
    private var currentPagerPosition: Int = 0
    private var advance: PreviewAdvance = PreviewAdvance.MANUAL
    private var showIndexIndicator: Boolean = false
    private var finished = false
    private val mainHandler = Handler(Looper.getMainLooper())
    private var imageAutoRunnable: Runnable? = null
    private var imageAutoRunnablePagerPosition: Int = RecyclerView.NO_POSITION
    private var previewRoot: View? = null
    private var transitionOverlay: PreviewVideoPosterView? = null
    private var transitionOverlayBitmap: Bitmap? = null
    private var transitionOverlayOwnsBitmap: Boolean = false
    private var initialContentRevealed: Boolean = false
    private var pendingSwitchPrefetch: Future<*>? = null
    private var pendingSwitchPrefetchGeneration: Long = 0L
    private var pendingUpcomingPrefetch: Future<*>? = null
    private var runtimeProfile: PreviewRuntimeProfile = PreviewRuntimeProfile.default()

    // Single LxMediaPlayer instance shared across all video pages. Its view
    // is parented to sharedPlayerHost ONCE in onCreateView and never
    // re-parented — this is what prevents the surface destroy/recreate that
    // produced black flicker between videos.
    //
    // Multi-source orchestration is delegated to the player's playlist
    // controller (LxMediaPlaylistController). MediaPreview just builds the
    // video subset of items once, calls applyPlaylist, then drives navigation
    // via playlistGoToIndex on each page change. Manual update(src) per page
    // is gone — the controller is the sole owner of "who plays next".
    private var sharedPlayerHost: FrameLayout? = null
    private var sharedPlayer: LxMediaPlayer? = null
    /**
     * One-time guard for `applyPlaylist`. Set on the first time we mount the
     * shared player on a video page, cleared in [cleanupPreviewResources].
     * Assumes [previewItems] is fixed for the lifetime of the fragment (it
     * is sourced from arguments in [onCreate] and never mutated). If that
     * assumption ever breaks, this gate plus [sharedPlaylistItems] both need
     * to be re-derived on the change.
     */
    private var sharedPlaylistApplied: Boolean = false
    private var sharedPlaylistItems: List<LxMediaPlaylistItem> = emptyList()
    /**
     * For each preview index in [previewItems], the corresponding playlist
     * index in [sharedPlaylistItems]; -1 for non-video items. Built once in
     * [computeVideoPlaylist].
     */
    private var videoPlaylistIndexByPreviewIndex: IntArray = IntArray(0)
    private var sharedPlayerScrollState: Int = ViewPager2.SCROLL_STATE_IDLE
    // Monotonically incremented on each video activation. Captured by
    // posted runnables (eventSink → onSharedPlayerFirstFrame → host.post
    // chain) so that stale callbacks from a prior activation cannot reveal
    // the host or hide the overlay for the wrong source — a single boolean
    // "handled" flag was racy: if activation A posts a frame event, then
    // activation B begins before A's runnable runs, A's callback would
    // see B as current, latch handled=true on B, and silently swallow B's
    // own first-frame event.
    private var sharedPlayerActivationGen: Long = 0L
    // Pager position the current activation gen was minted for. The gen
    // only bumps when we transition to a *different* video page —
    // re-invocations of applySharedPlayerForCurrentItem on the same page
    // (handlePageSelected then scroll-state IDLE both call us; programmatic
    // setCurrentItem can trigger an extra IDLE; etc.) must NOT bump gen,
    // otherwise the seek-once-per-gen invariant decays into "seek on every
    // re-invocation" and playback keeps restarting from 0.
    private var sharedPlayerActivationPager: Int = Int.MIN_VALUE
    // The activation gen that has already consumed its first-frame event.
    // Equal to current `sharedPlayerActivationGen` ⇒ reveal already done
    // for this activation; smaller ⇒ a fresh activation hasn't been
    // handled yet.
    private var sharedPlayerFirstFrameGen: Long = -1L
    // The activation gen for which we've already issued a seek-to-0 +
    // play(). Re-invocations of applySharedPlayerForCurrentItem within
    // the same activation (scroll-state-IDLE rebounce, visual-ready, …)
    // must not seek again or playback gets restarted mid-stream and
    // never reaches the end.
    private var sharedPlayerSeekedGen: Long = -1L
    private var lastLoggedPlayerTimeUpdateMs: Long = Long.MIN_VALUE
    // Prewarm state for image→video transitions: we tell the shared player to
    // load the target video *before* swapping pages, then swap atomically on
    // the player's firstframerendered event so the user never sees the new
    // page's black background. -1L = no prewarm pending.
    private var pendingPrewarmGen: Long = -1L
    private var pendingPrewarmCommit: (() -> Unit)? = null
    private var pendingPrewarmTimeout: Runnable? = null
    private var suppressScrollHostHideForPrewarmGen: Long = -1L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        runtimeProfile = PreviewRuntimeProfile.from(requireContext())
        previewItems = readPreviewItems()
        callbackId = arguments?.getLong(ARG_CALLBACK_ID, 0L) ?: 0L
        presentedCallbackId = arguments?.getLong(ARG_PRESENTED_CALLBACK_ID, 0L) ?: 0L
        advance = PreviewAdvance.fromRaw(arguments?.getString(ARG_ADVANCE))
        currentIndex = clampIndex(arguments?.getInt(ARG_START_INDEX, 0) ?: 0)
        currentPagerPosition = initialPagerPosition(currentIndex)
        showIndexIndicator = arguments?.getBoolean(ARG_SHOW_INDEX_INDICATOR, false) ?: false
    }

    override fun onCreateView(
        inflater: android.view.LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val context = requireContext()
        val root = FrameLayout(context).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            visibility = View.VISIBLE
            alpha = if (shouldGateInitialRevealUntilVideoFrame()) 0f else 1f
        }
        previewRoot = root

        if (previewItems.isEmpty()) {
            root.post { finishPreview("error") }
            return root
        }

        totalItems = previewItems.size

        val adapter = PreviewPagerAdapter(
            items = previewItems,
            loopEnabled = shouldUseLoopPager(),
            loopSingleItemVideo = shouldLoopSingleItemVideo(),
            runtimeProfile = runtimeProfile,
            userInputEnabled = advance == PreviewAdvance.MANUAL,
            onDismiss = { finishPreview("manual") },
            onVideoTerminal = { position, terminal -> onVideoTerminal(position, terminal) },
            onItemVisualReady = { position -> onItemVisualReady(position) }
        )
        previewAdapter = adapter

        val pager = ViewPager2(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            this.adapter = adapter
            offscreenPageLimit = 1
            isUserInputEnabled = advance == PreviewAdvance.MANUAL
            setCurrentItem(currentPagerPosition, false)
        }
        adapter.attachToViewPager(pager)
        viewPager = pager
        root.addView(pager)

        val playerHost = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            visibility = View.GONE
        }
        sharedPlayerHost = playerHost
        root.addView(playerHost)

        val overlay = PreviewVideoPosterView(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            setBackgroundColor(Color.BLACK)
            visibility = View.GONE
        }
        transitionOverlay = overlay
        root.addView(overlay)
        previewItems.getOrNull(currentIndex)?.let { showTransitionOverlayForTargetVisual(it) }

        val topBar = createTopBar(context, totalItems)
        root.addView(topBar)

        updateIndicator(currentIndex)
        val callback = object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                handlePageSelected(position)
            }
            override fun onPageScrollStateChanged(state: Int) {
                onPagerScrollStateChanged(state)
            }
        }
        pageChangeCallback = callback
        pager.registerOnPageChangeCallback(callback)
        beginInitialRevealGate()
        root.post {
            if (!finished) {
                handlePageSelected(currentPagerPosition)
            }
        }

        return root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        if (previewItems.isEmpty()) return

        val activity = requireActivity()
        val window = activity.window
        if (windowUiSnapshot == null) {
            windowUiSnapshot = ImmersiveWindowUi.capture(window)
        }
        ImmersiveWindowUi.apply(window, keepScreenOn = false)

        activity.onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    finishPreview("manual")
                }
            }
        )
    }

    override fun onDestroyView() {
        if (!finished) {
            finished = true
            sendPreviewResult("interrupted")
        }
        clearAutoRunnables()
        cleanupPreviewResources()
        PreviewPagerAdapter.clearVisualCaches()
        super.onDestroyView()
    }

    private fun sendPreviewResult(reason: String) {
        // Always settle `presented` before sending the completion result.
        // Degenerate path: session ends (manual close, error) before any
        // item rendered. The JS-side fallback already wakes presented on
        // completion, but native side fires too so the message ordering
        // stays predictable.
        signalPresentedOnce()
        if (callbackId <= 0L) return
        val result = JSONObject()
            .put("reason", reason)
            .put("lastIndex", currentIndex)
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    /**
     * Fire the JS-side `presented` Promise exactly once. Safe to call from
     * multiple visual-ready paths (image loaded vs video first frame) — the
     * second call is a no-op. Always called with a `{}` payload because the
     * presented Promise carries no useful value, just the timing signal.
     */
    private fun signalPresentedOnce() {
        val id = presentedCallbackId
        if (id <= 0L) return
        presentedCallbackId = 0L
        NativeApi.onCallback(id, true, "{}")
    }

    private fun cleanupPreviewResources() {
        cancelPendingPrewarm()
        pageChangeCallback?.let { callback ->
            viewPager?.unregisterOnPageChangeCallback(callback)
        }
        pageChangeCallback = null
        previewAdapter?.release()
        // Force ViewPager to detach/recycle pages so any per-page resources release.
        viewPager?.adapter = null
        previewAdapter = null
        viewPager = null
        sharedPlayer?.detach()
        sharedPlayer = null
        sharedPlaylistApplied = false
        sharedPlaylistItems = emptyList()
        videoPlaylistIndexByPreviewIndex = IntArray(0)
        suppressScrollHostHideForPrewarmGen = -1L
        sharedPlayerHost = null
        previewRoot = null
        hideTransitionOverlay()
        transitionOverlay = null
        indicatorText = null
        closeButton = null

        restoreWindowUiIfNeeded()
    }

    private fun clearAutoRunnables() {
        clearAutoTimer()
        cancelPendingSwitchPrefetch()
        cancelPendingUpcomingPrefetch()
    }

    private fun clearAutoTimer() {
        imageAutoRunnable?.let(mainHandler::removeCallbacks)
        imageAutoRunnable = null
        imageAutoRunnablePagerPosition = RecyclerView.NO_POSITION
    }

    private fun restoreWindowUiIfNeeded() {
        activity?.window?.let { window ->
            windowUiSnapshot?.let { snapshot ->
                ImmersiveWindowUi.restore(window, snapshot)
            }
        }
        windowUiSnapshot = null
    }

    private fun createTopBar(context: Context, itemCount: Int): View {
        val topContainer = FrameLayout(context).apply {
            setBackgroundColor(Color.TRANSPARENT)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.TOP)
        }

        val dpMargin = TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            16f,
            context.resources.displayMetrics
        ).toInt()
        val topOffset = statusBarHeight(context) + dpMargin

        indicatorText = TextView(context).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            textAlignment = View.TEXT_ALIGNMENT_CENTER
            setShadowLayer(4f, 0f, 0f, Color.parseColor("#66000000"))
            layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply {
                gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
                setMargins(dpMargin, topOffset, dpMargin, dpMargin)
            }
            visibility = if (showIndexIndicator && itemCount > 0) View.VISIBLE else View.GONE
        }

        indicatorText?.let(topContainer::addView)
        closeButton = createCloseButton(context).also(topContainer::addView)
        return topContainer
    }

    /// Mirror of the iOS preview close button: 36dp circular semi-transparent
    /// black chip pinned to the top-right with a white X glyph. Uses the
    /// existing `icon_close_x` vector (originally `#1F1F1F`) tinted white.
    private fun createCloseButton(context: Context): ImageButton {
        val displayMetrics = context.resources.displayMetrics
        fun dp(value: Float): Int = TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value,
            displayMetrics
        ).toInt()
        val statusInset = statusBarHeight(context)
        val chipBackground = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(Color.argb(115, 0, 0, 0))
        }
        val edgeMargin = dp(12f)
        return ImageButton(context).apply {
            background = chipBackground
            setImageResource(R.drawable.icon_close_x)
            imageTintList = ColorStateList.valueOf(Color.WHITE)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(8f), dp(8f), dp(8f), dp(8f))
            contentDescription = context.getString(android.R.string.cancel)
            // Default hidden; `updateCloseButtonVisibility()` shows it on
            // video pages once the page selection settles. Avoids a brief
            // flash on image-first previews.
            visibility = View.GONE
            layoutParams = FrameLayout.LayoutParams(dp(36f), dp(36f)).apply {
                gravity = Gravity.TOP or Gravity.END
                topMargin = statusInset + dp(2f)
                rightMargin = edgeMargin
            }
            setOnClickListener { finishPreview("manual") }
        }
    }

    private fun updateIndicator(position: Int) {
        if (!showIndexIndicator || totalItems <= 0) {
            indicatorText?.visibility = View.GONE
            return
        }
        indicatorText?.visibility = View.VISIBLE
        indicatorText?.text = "${position + 1}/$totalItems"
    }

    private fun readPreviewItems(): List<PreviewItem> {
        val args = arguments ?: return emptyList()
        @Suppress("DEPRECATION")
        val raw = args.getSerializable(ARG_PAYLOADS)
        val payloads: List<PreviewMediaPayload> = when (raw) {
            is Array<*> -> raw.filterIsInstance<PreviewMediaPayload>()
            is List<*> -> raw.filterIsInstance<PreviewMediaPayload>()
            else -> emptyList()
        }

        return payloads.map { payload ->
            val normalizedUri = normalizeUri(payload.path)
            PreviewItem(
                uri = normalizedUri,
                mediaType = MediaPreviewType.fromInt(payload.type),
                rotate = payload.rotate,
                objectFit = payload.objectFit?.let { LxMediaObjectFit.fromString(it) },
                durationMs = payload.durationMs?.takeIf { it > 0L }
            )
        }
    }

    private fun handlePageSelected(position: Int) {
        clearAutoTimer()
        currentPagerPosition = position
        currentIndex = realIndexFor(position)
        logPlaybackState("page_selected")
        updateIndicator(currentIndex)
        updateCloseButtonVisibility()
        previewAdapter?.onPageSelected(position)
        applySharedPlayerForCurrentItem()
        scheduleCurrentItemBehaviorWhenVisualReady("page_selected")
        prefetchUpcomingVisual(position)
        // When ViewPager scrolls to a holder that was already bound during
        // a previous neighbor-prefetch (e.g. LOOP back from video → image[0]
        // where the image holder was bound while the video was activating),
        // no new onItemVisualReady fires for the current position. Without
        // an explicit hide here, the terminal-advance transitionOverlay
        // (showing the next-item poster set in onVideoTerminal) keeps
        // covering the page beneath — masking every subsequent in-place
        // bitmap swap so the user perceives the cycle as stuck on one image.
        val item = previewItems.getOrNull(currentIndex)
        if (item?.mediaType != MediaPreviewType.VIDEO &&
            previewAdapter?.isPositionVisualReady(position) == true
        ) {
            hideTransitionOverlay()
        }
    }

    /// Match iOS / Harmony: chip is gated on (video page AND inline player
    /// controls currently visible). Images use their own dismiss gestures
    /// inside the adapter, and a video page only reveals close when the user
    /// taps to expose play/pause/scrub — same affordance as the controls bar.
    private fun updateCloseButtonVisibility() {
        val item = previewItems.getOrNull(currentIndex)
        val isVideo = item?.mediaType == MediaPreviewType.VIDEO
        val controlsShown = sharedPlayer?.isControlsVisible() == true
        val showChip = isVideo && controlsShown
        closeButton?.visibility = if (showChip) View.VISIBLE else View.GONE
    }

    private fun onPagerScrollStateChanged(state: Int) {
        sharedPlayerScrollState = state
        logPlaybackState("scroll_state", "state=${scrollStateName(state)}")
        // Hide the player view while the user (or programmatic settle) is
        // mid-swipe so the underlying ViewPager pages — which carry the
        // first-frame placeholders — remain visible during the gesture.
        // Once the page settles we re-evaluate visibility based on the
        // newly-current item.
        when (state) {
            ViewPager2.SCROLL_STATE_DRAGGING,
            ViewPager2.SCROLL_STATE_SETTLING -> {
                // User-initiated drag invalidates any in-flight prewarm: we
                // were prewarming a *specific* target index, but the user is
                // now choosing a different page themselves.
                cancelPendingPrewarm()
                if (suppressScrollHostHideForPrewarmGen != sharedPlayerActivationGen) {
                    sharedPlayerHost?.visibility = View.INVISIBLE
                }
            }
            ViewPager2.SCROLL_STATE_IDLE -> {
                applySharedPlayerForCurrentItem()
            }
        }
    }

    private fun ensureSharedPlayer(): LxMediaPlayer {
        sharedPlayer?.let { return it }
        val context = requireContext()
        val player = LxMediaPlayer(
            context.applicationContext,
            eventSink = { payload ->
                val event = payload["event"] as? String
                logSharedPlayerEvent(payload)
                if (event == "firstframerendered" || event == "playing") {
                    // Capture the activation gen at the posting site, not at
                    // runnable execution. Otherwise a stale event from a
                    // previous video activation (still queued in mainHandler
                    // when the user moves on to the next page) would observe
                    // the new gen at execution and latch a reveal for the
                    // wrong source — the new activation's real first-frame
                    // event then no-ops because firstFrameGen is already
                    // latched. The runnable matches against the captured
                    // gen so it can drop itself if a newer activation took
                    // over before it ran.
                    val expectedGen = sharedPlayerActivationGen
                    val expectedPagerPosition = sharedPlayerActivationPager
                    mainHandler.post {
                        if (expectedGen != sharedPlayerActivationGen) return@post
                        // Prewarm first-frame: the activation's pager position is
                        // the *future* target (we haven't swapped yet). Run the
                        // commit instead of the normal reveal path.
                        if (expectedGen == pendingPrewarmGen) {
                            pendingPrewarmCommit?.invoke()
                            return@post
                        }
                        if (expectedPagerPosition == currentPagerPosition) {
                            onSharedPlayerFirstFrame(expectedGen, expectedPagerPosition)
                        }
                    }
                }
            },
            typedEventSink = { event ->
                when (event) {
                    is LxMediaEvent.Ended,
                    is LxMediaEvent.Error -> {
                        // Capture activation gen at the posting site. On
                        // Android 5 a stale ended/error from the previous
                        // source can arrive after we've already moved on to
                        // the next page; running onVideoTerminal against
                        // *that* page's currentPagerPosition would either
                        // advance the wrong item (ended) or — worse —
                        // close the whole new preview session (error).
                        val expectedGen = sharedPlayerActivationGen
                        val expectedPagerPosition = sharedPlayerActivationPager
                        val terminal = if (event is LxMediaEvent.Error) "error" else "ended"
                        mainHandler.post {
                            if (expectedGen == sharedPlayerActivationGen &&
                                expectedPagerPosition == currentPagerPosition
                            ) {
                                onSharedPlayerTerminal(expectedPagerPosition, terminal)
                            }
                        }
                    }
                    else -> Unit
                }
            }
        )
        // Use the preview-level close button (createTopBar) instead of the
        // player's built-in one so the visual treatment matches iOS: top-right
        // 36dp circular semi-transparent chip with a white X glyph. Keeping
        // the player's plain top-left X here would diverge from iOS.
        player.setShowCloseButton(false)
        player.setShowFullscreenButton(false)
        player.setShowLoadingIndicator(false)
        player.setSuppressAutoShowControls(true)
        // Mirror the preview-level close button against the inline controls
        // overlay — when the user taps the video to reveal play/pause/scrub,
        // the close affordance appears in the same beat; when controls hide
        // (auto-hide or another tap), so does close. No persistent overlay.
        player.setOnControlsVisibilityChanged { _ ->
            updateCloseButtonVisibility()
        }
        sharedPlayerHost?.let { player.attach(it) }
        sharedPlayer = player
        return player
    }

    /**
     * Build the video subset of the preview list as a playlist of items, plus
     * the preview-index → playlist-index lookup. Called once per preview;
     * idempotent (only computes when not yet built).
     */
    private fun computeVideoPlaylist() {
        if (sharedPlaylistItems.isNotEmpty() ||
            videoPlaylistIndexByPreviewIndex.isNotEmpty()
        ) return
        val map = IntArray(previewItems.size) { -1 }
        val items = ArrayList<LxMediaPlaylistItem>()
        for ((i, p) in previewItems.withIndex()) {
            if (p.mediaType != MediaPreviewType.VIDEO) continue
            map[i] = items.size
            items.add(
                LxMediaPlaylistItem(
                    url = p.uri.toString(),
                    objectFit = p.objectFit ?: LxMediaObjectFit.CONTAIN,
                    rotateDegrees = p.rotate
                )
            )
        }
        sharedPlaylistItems = items
        videoPlaylistIndexByPreviewIndex = map
    }

    private fun applySharedPlayerForCurrentItem() {
        if (sharedPlayerScrollState != ViewPager2.SCROLL_STATE_IDLE) return
        val host = sharedPlayerHost ?: return
        val item = previewItems.getOrNull(currentIndex) ?: run {
            host.visibility = View.INVISIBLE
            sharedPlayer?.pause()
            return
        }
        when (item.mediaType) {
            MediaPreviewType.VIDEO -> {
                // Only bump the gen when this is a different video page
                // than the previous activation — handlePageSelected and
                // scroll-state IDLE both route here, so the same page
                // legitimately re-enters this branch and must not bump.
                if (sharedPlayerActivationPager != currentPagerPosition) {
                    sharedPlayerActivationGen += 1L
                    sharedPlayerActivationPager = currentPagerPosition
                    logPlaybackState("video_activate", "playlistIdx=pending")
                }
                computeVideoPlaylist()
                if (sharedPlaylistItems.isEmpty()) {
                    // Defensive: video items list is empty although the page is
                    // typed VIDEO. Hide the host so we don't accidentally show
                    // stale player content on top of the (now placeholder-only)
                    // ViewPager page.
                    host.visibility = View.INVISIBLE
                    return
                }

                val playlistIdx = videoPlaylistIndexByPreviewIndex
                    .getOrNull(currentIndex)
                    ?.takeIf { it >= 0 }
                    ?: run {
                        // Unreachable in practice (mediaType==VIDEO implies a
                        // mapping exists), but be explicit instead of letting
                        // host visibility drift to whatever it was before.
                        host.visibility = View.INVISIBLE
                        return
                    }

                val player = ensureSharedPlayer()

                // First time we see a video page: feed the whole video subset
                // to the player's playlist controller. Element-level config
                // (loop, controls, etc.) is set once; per-item objectFit /
                // rotateDegrees travel inside each playlist item.
                //
                // autoAdvance = false: ViewPager is the source of truth for
                // "what plays next" in preview. Without this the controller
                // would jump to the next playlist item on Ended/Error and
                // bleed audio behind whatever (possibly image) page the user
                // is now on. See LxMediaPlaylistController.autoAdvance.
                if (!sharedPlaylistApplied) {
                    player.update(
                        LxMediaPlayerConfig(
                            poster = null,
                            autoplay = false,
                            loop = shouldLoopSingleItemVideo(),
                            controls = true,
                        )
                    )
                    player.applyPlaylist(
                        items = sharedPlaylistItems,
                        autoAdvance = false,
                        startingIndex = playlistIdx,
                    )
                    sharedPlaylistApplied = true
                }

                // Hide the host while the controller swaps source; promoted
                // back to VISIBLE on firstframerendered so the holder's
                // first-frame placeholder beneath bridges the decode gap.
                //
                // Exception: if we already revealed for this activation gen
                // (a prewarm completed before the page swap), don't hide
                // again — handlePageSelected re-enters this method after a
                // prewarm commit and that re-entry must not blank the screen
                // back out.
                if (sharedPlayerFirstFrameGen != sharedPlayerActivationGen) {
                    host.animate().cancel()
                    host.alpha = 0f
                    host.visibility = View.INVISIBLE
                }
                player.playlistGoToIndex(playlistIdx)
                logPlaybackState("video_apply", "playlistIdx=$playlistIdx")
                // Seek to head exactly once per activation. Relying on the
                // player's internal hasEnded flag isn't reliable on Android
                // 5: ExoPlayer's state transitions there can route through
                // an intermediate Play event that clears hasEnded before we
                // re-enter the video page, leaving the source parked at the
                // end of stream with hasEnded=false — a bare play() then
                // no-ops and the user sees a frozen last frame. Gating on
                // sharedPlayerActivationGen also keeps the seek from firing
                // on every re-invocation of applySharedPlayerForCurrentItem
                // within a single activation (scroll-state IDLE rebounce,
                // visual-ready callback, …) which would otherwise restart
                // playback mid-stream and prevent it from ever finishing.
                if (sharedPlayerSeekedGen != sharedPlayerActivationGen) {
                    sharedPlayerSeekedGen = sharedPlayerActivationGen
                    logPlaybackState("video_seek_start", "playlistIdx=$playlistIdx")
                    player.seek(0.0)
                }
                logPlaybackState("video_play", "playlistIdx=$playlistIdx")
                player.play()
            }
            MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> {
                if (sharedPlayerActivationPager != Int.MIN_VALUE) {
                    sharedPlayerActivationGen += 1L
                    sharedPlayerActivationPager = Int.MIN_VALUE
                    logPlaybackState("video_deactivate_for_non_video")
                }
                sharedPlayer?.pause()
                logPlaybackState("non_video_pause_player")
                // INVISIBLE (not GONE) so the player's TextureView keeps its
                // SurfaceTexture alive — switching back to a video page later
                // can resume rendering immediately without a surface rebind.
                host.animate().cancel()
                host.alpha = 0f
                host.visibility = View.INVISIBLE
            }
        }
    }

    private fun onSharedPlayerFirstFrame(expectedGen: Long, expectedPagerPosition: Int) {
        val gen = expectedGen
        val currentItem = previewItems.getOrNull(currentIndex)
        if (currentItem?.mediaType != MediaPreviewType.VIDEO) return
        if (expectedPagerPosition != currentPagerPosition) return
        // Already revealed for this activation? bail.
        if (gen == sharedPlayerFirstFrameGen) return
        val host = sharedPlayerHost ?: return
        host.post {
            if (finished ||
                gen != sharedPlayerActivationGen ||
                expectedPagerPosition != currentPagerPosition
            ) return@post
            // Re-check at deferred-execution time: a stale runnable from a
            // prior activation can't reach here (the eventSink wrapper drops
            // mismatched gens before posting), but the firstFrameGen latch
            // must wait until we actually commit to the reveal — otherwise
            // a drag mid-flight makes willReveal=false here, we latch
            // firstFrameGen anyway, and no subsequent firstframerendered
            // for the same activation can re-trigger the reveal.
            if (gen == sharedPlayerFirstFrameGen) return@post
            val item = previewItems.getOrNull(currentIndex)
            val willReveal = item?.mediaType == MediaPreviewType.VIDEO &&
                sharedPlayerScrollState == ViewPager2.SCROLL_STATE_IDLE
            if (willReveal) {
                sharedPlayerFirstFrameGen = gen
                logPlaybackState("video_first_frame", "position=$expectedPagerPosition gen=$gen")
                host.animate().cancel()
                host.alpha = 0f
                host.visibility = View.VISIBLE
                val revealFrames = if (normalizePreviewRotation(item.rotate) == 0) 1 else 3
                postAfterAnimationFrames(host, revealFrames) {
                    if (!finished && gen == sharedPlayerActivationGen) {
                        revealPreviewRoot()
                        host.alpha = 1f
                        // First video frame is on screen: settle `presented`.
                        signalPresentedOnce()
                        host.postDelayed({
                            if (!finished && gen == sharedPlayerActivationGen) {
                                hideTransitionOverlay()
                            }
                        }, 50L)
                    }
                }
            }
            previewAdapter?.notifyVideoRenderedAt(expectedPagerPosition)
        }
    }

    private fun postAfterAnimationFrames(view: View, frames: Int, action: () -> Unit) {
        if (frames <= 0) {
            action()
            return
        }
        view.postOnAnimation {
            postAfterAnimationFrames(view, frames - 1, action)
        }
    }

    private fun onSharedPlayerTerminal(position: Int, terminal: String) {
        val item = previewItems.getOrNull(currentIndex)
        if (item?.mediaType != MediaPreviewType.VIDEO) return
        logPlaybackState("video_terminal", "position=$position terminal=$terminal")
        onVideoTerminal(position, terminal)
    }

    private fun scheduleCurrentItemBehaviorWhenVisualReady(reason: String) {
        val item = previewItems.getOrNull(currentIndex) ?: return
        if (item.durationMs == null || item.durationMs <= 0L || advance == PreviewAdvance.MANUAL) {
            return
        }
        if (previewAdapter?.isPositionVisualReady(currentPagerPosition) == true) {
            scheduleCurrentItemBehavior(reason)
            return
        }
    }

    private fun scheduleCurrentItemBehavior(reason: String) {
        val item = previewItems.getOrNull(currentIndex) ?: return
        if (imageAutoRunnablePagerPosition == currentPagerPosition && imageAutoRunnable != null) {
            return
        }
        clearAutoTimer()

        val timeoutMs = item.durationMs
        if (timeoutMs != null && timeoutMs > 0L && advance != PreviewAdvance.MANUAL) {
            val scheduledPagerPosition = currentPagerPosition
            val runnable = Runnable {
                imageAutoRunnable = null
                imageAutoRunnablePagerPosition = RecyclerView.NO_POSITION
                if (finished || scheduledPagerPosition != currentPagerPosition) {
                    return@Runnable
                }
                logPlaybackState("auto_advance_fire", "timeoutMs=$timeoutMs")
                advanceFromCurrentItem()
            }
            imageAutoRunnable = runnable
            imageAutoRunnablePagerPosition = scheduledPagerPosition
            logPlaybackState("auto_advance_schedule", "timeoutMs=$timeoutMs reason=$reason")
            mainHandler.postDelayed(runnable, timeoutMs)
        }
    }

    private fun onVideoTerminal(position: Int, terminal: String) {
        if (finished || position != currentPagerPosition) return
        logPlaybackState("video_terminal_handle", "position=$position terminal=$terminal")
        when (terminal) {
            "error" -> {
                clearAutoRunnables()
                finishPreview("error")
            }
            else -> {
                val item = previewItems.getOrNull(currentIndex)
                if (item?.durationMs != null && item.durationMs > 0L && advance != PreviewAdvance.MANUAL) {
                    return
                }
                if (advance == PreviewAdvance.LOOP && previewItems.size == 1) {
                    clearAutoRunnables()
                    logPlaybackState("single_video_loop_restart")
                    sharedPlayer?.seek(0.0)
                    sharedPlayer?.play()
                    return
                }
                showTerminalAdvanceOverlay()
                clearAutoRunnables()
                advanceFromCurrentItem()
            }
        }
    }

    private fun advanceFromCurrentItem() {
        logPlaybackState("advance_from_current")
        when (advance) {
            PreviewAdvance.MANUAL -> Unit
            PreviewAdvance.NEXT -> {
                if (currentIndex < previewItems.lastIndex) {
                    advanceToNextItem()
                } else {
                    finishPreview("completed")
                }
            }
            PreviewAdvance.LOOP -> {
                if (previewItems.isEmpty()) {
                    finishPreview("completed")
                    return
                }
                if (previewItems.size == 1) {
                    scheduleCurrentItemBehaviorWhenVisualReady("single_loop")
                    return
                }
                advanceToNextItem()
            }
        }
    }

    private fun advanceToNextItem() {
        val targetPagerPosition = currentPagerPosition + 1
        val targetIndex = realIndexFor(targetPagerPosition)
        if (tryAdvanceImageInPlace(targetPagerPosition, targetIndex)) {
            return
        }
        advanceToPagerPosition(targetPagerPosition)
    }

    private fun tryAdvanceImageInPlace(targetPagerPosition: Int, targetIndex: Int): Boolean {
        val current = previewItems.getOrNull(currentIndex) ?: return false
        val target = previewItems.getOrNull(targetIndex) ?: return false
        if (!isInPlaceImageAdvanceCandidate(current) || !isInPlaceImageAdvanceCandidate(target)) {
            return false
        }
        val context = context ?: return false
        val targetBitmap = PreviewPagerAdapter.getCachedLocalImage(context, target.uri)
        if (targetBitmap == null) {
            advanceImageInPlaceAfterDecode(context, targetPagerPosition, targetIndex, target)
            return true
        }
        return replaceCurrentImageInPlace(targetPagerPosition, targetIndex, target, targetBitmap)
    }

    private fun advanceImageInPlaceAfterDecode(
        context: Context,
        targetPagerPosition: Int,
        targetIndex: Int,
        target: PreviewItem
    ) {
        cancelPendingSwitchPrefetch()
        val generation = pendingSwitchPrefetchGeneration
        // Capture the source pager position at scheduling time. If the user
        // (or auto-advance) moves to a different page while the async decode
        // is in flight, the in-place replace must NOT land on whichever
        // image happens to be current now — `replaceCurrentImage` just
        // overwrites the current holder's bitmap and would silently corrupt
        // the wrong page.
        val sourcePagerPosition = currentPagerPosition
        pendingSwitchPrefetch = PreviewPagerAdapter.prefetchItemVisual(
            context = context,
            item = target,
            runtimeProfile = runtimeProfile
        ) { success ->
            if (finished || generation != pendingSwitchPrefetchGeneration) return@prefetchItemVisual
            pendingSwitchPrefetch = null
            if (currentPagerPosition != sourcePagerPosition) {
                // User moved past the source page while we were decoding —
                // the in-place advance for the original target is stale.
                return@prefetchItemVisual
            }
            val bitmap = PreviewPagerAdapter.getCachedLocalImage(context, target.uri)
            if (bitmap != null && replaceCurrentImageInPlace(targetPagerPosition, targetIndex, target, bitmap)) {
                return@prefetchItemVisual
            }
            Log.i(
                LOG_TAG,
                "auto_image_inplace_fallback_pager success=$success cacheHit=${bitmap != null} " +
                    "targetIndex=$targetIndex uri=${target.uri}"
            )
            advanceToPagerPosition(targetPagerPosition)
        }
    }

    private fun replaceCurrentImageInPlace(
        targetPagerPosition: Int,
        targetIndex: Int,
        target: PreviewItem,
        targetBitmap: Bitmap
    ): Boolean {
        val replaceResult = previewAdapter?.replaceCurrentImage(target, targetBitmap)
            ?: InPlaceImageReplaceResult.NO_ADAPTER
        if (replaceResult != InPlaceImageReplaceResult.APPLIED) {
            Log.i(
                LOG_TAG,
                "auto_image_inplace_replace_failed reason=$replaceResult " +
                    "targetIndex=$targetIndex pager=$targetPagerPosition uri=${target.uri}"
            )
            return false
        }

        cancelPendingSwitchPrefetch()
        currentPagerPosition = targetPagerPosition
        currentIndex = targetIndex
        updateIndicator(currentIndex)
        scheduleCurrentItemBehavior("inplace_image")
        prefetchUpcomingVisual(currentPagerPosition)
        return true
    }

    private fun isInPlaceImageAdvanceCandidate(item: PreviewItem): Boolean {
        return item.mediaType != MediaPreviewType.VIDEO && isLocalUri(item.uri)
    }

    private fun beginInitialRevealGate() {
        if (shouldGateInitialRevealUntilVideoFrame()) {
            initialContentRevealed = false
            previewRoot?.alpha = 0f
            return
        }
        initialContentRevealed = true
        previewRoot?.visibility = View.VISIBLE
        previewRoot?.alpha = 1f
    }

    private fun shouldGateInitialRevealUntilVideoFrame(): Boolean {
        if (initialContentRevealed) return false
        return previewItems.getOrNull(currentIndex)?.mediaType == MediaPreviewType.VIDEO
    }

    private fun revealPreviewRoot() {
        if (initialContentRevealed) return
        initialContentRevealed = true
        previewRoot?.visibility = View.VISIBLE
        previewRoot?.alpha = 1f
    }

    private fun onItemVisualReady(position: Int) {
        if (position == currentPagerPosition) {
            val item = previewItems.getOrNull(currentIndex)
            if (!initialContentRevealed && item?.mediaType != MediaPreviewType.VIDEO) {
                revealPreviewRoot()
            }
            // For video items, the placeholder being ready does not mean the
            // player has decoded its first frame yet. Keep the transition
            // overlay up until the shared player's firstframerendered event
            // fires (handled in onSharedPlayerFirstFrame). For images, the
            // visual is fully on-screen now, so we can hide it immediately
            // and signal the JS-side `presented` Promise — deferred by one
            // animation frame so the bitmap has actually been composited to
            // screen before the consumer's .then() runs. The video path
            // already goes through postAfterAnimationFrames, so this keeps
            // the two paths' timing guarantees in sync.
            if (item?.mediaType != MediaPreviewType.VIDEO) {
                hideTransitionOverlay()
                val root = previewRoot
                if (root != null) {
                    root.postOnAnimation {
                        if (!finished) signalPresentedOnce()
                    }
                } else {
                    signalPresentedOnce()
                }
            }
            scheduleCurrentItemBehavior("visual_ready")
        }
    }

    private fun showTerminalAdvanceOverlay() {
        val targetPagerPosition = when (advance) {
            PreviewAdvance.MANUAL -> return
            PreviewAdvance.NEXT -> {
                if (currentIndex >= previewItems.lastIndex) return
                currentPagerPosition + 1
            }
            PreviewAdvance.LOOP -> {
                if (previewItems.size <= 1) return
                currentPagerPosition + 1
            }
        }
        val target = previewItems.getOrNull(realIndexFor(targetPagerPosition)) ?: return
        showTransitionOverlayForSwitchTarget(target)
    }

    private fun cancelPendingUpcomingPrefetch() {
        pendingUpcomingPrefetch?.cancel(true)
        pendingUpcomingPrefetch = null
    }

    private fun cancelPendingSwitchPrefetch() {
        pendingSwitchPrefetch?.cancel(true)
        pendingSwitchPrefetch = null
        pendingSwitchPrefetchGeneration += 1L
    }

    private fun prefetchUpcomingVisual(currentPosition: Int) {
        cancelPendingUpcomingPrefetch()
        if (previewItems.isEmpty()) {
            return
        }
        val targetPagerPosition = when {
            shouldUseLoopPager() -> currentPosition + 1
            currentIndex < previewItems.lastIndex -> currentPosition + 1
            else -> return
        }
        val target = previewItems.getOrNull(realIndexFor(targetPagerPosition)) ?: return
        if (!shouldPrefetchUpcomingVisual(target)) {
            return
        }
        val context = context ?: return
        // Player engine is shared across pages; no per-page prewarm needed.
        // Visual prefetch (first-frame for video, decoded bitmap for image)
        // still runs so the placeholder is ready before the page is visible.
        pendingUpcomingPrefetch = PreviewPagerAdapter.prefetchItemVisual(
            context = context,
            item = target,
            runtimeProfile = runtimeProfile
        )
    }

    private fun showTransitionOverlayFromCurrentVisual(): Boolean {
        val overlay = transitionOverlay ?: return false
        // Pick the snapshot source based on what the user is *actually* looking
        // at right now. The shared player's TextureView is kept INVISIBLE (not
        // GONE) across non-video pages to preserve its SurfaceTexture, so
        // `snapshotCurrentPlaybackFrame()` will still return pixels — but they
        // are the LAST FRAME of the previously-played video. Using that on an
        // image→video transition surfaces a stale (often dark) frame that the
        // user perceives as a black flash.
        val currentItem = previewItems.getOrNull(currentIndex)
        val bitmap = when (currentItem?.mediaType) {
            MediaPreviewType.VIDEO ->
                sharedPlayer?.snapshotCurrentPlaybackFrame()
                    ?: previewAdapter?.snapshotCurrentVisualBitmap()
            else ->
                previewAdapter?.snapshotCurrentVisualBitmap()
        } ?: return false
        clearTransitionOverlayBitmap()
        transitionOverlayBitmap = bitmap
        transitionOverlayOwnsBitmap = true
        // Snapshot already has the visible rotation baked into pixels — stretch to fill.
        overlay.setPreviewRotationDegrees(0)
        overlay.setPreviewObjectFit(LxMediaObjectFit.FILL)
        overlay.setImageBitmap(bitmap)
        overlay.visibility = View.VISIBLE
        return true
    }

    private fun showTransitionOverlayForTargetVisual(target: PreviewItem): Boolean {
        val overlay = transitionOverlay ?: return false
        val context = context ?: return false
        val bitmap = when (target.mediaType) {
            MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN ->
                PreviewPagerAdapter.getCachedLocalImage(context, target.uri)
            MediaPreviewType.VIDEO ->
                PreviewPagerAdapter.getCachedVideoFirstFrame(context, target.uri)
        } ?: return false

        clearTransitionOverlayBitmap()
        transitionOverlayBitmap = bitmap
        transitionOverlayOwnsBitmap = false
        // Cached frame is raw, un-oriented — apply the item's rotation / fit so
        // the overlay matches what the page (and the player, for video) will render.
        overlay.setPreviewRotationDegrees(target.rotate)
        overlay.setPreviewObjectFit(target.objectFit)
        overlay.setImageBitmap(bitmap)
        overlay.visibility = View.VISIBLE
        return true
    }

    private fun clearTransitionOverlayBitmap() {
        if (transitionOverlayOwnsBitmap) {
            transitionOverlayBitmap?.recycle()
        }
        transitionOverlayBitmap = null
        transitionOverlayOwnsBitmap = false
    }

    private fun hideTransitionOverlay() {
        transitionOverlay?.setImageDrawable(null)
        transitionOverlay?.visibility = View.GONE
        clearTransitionOverlayBitmap()
    }

    private fun showTransitionOverlayForSwitchTarget(target: PreviewItem): Boolean {
        if (showTransitionOverlayForTargetVisual(target)) return true
        return showTransitionOverlayFromCurrentVisual()
    }

    private fun switchToPagerPositionWithVisualOverlay(targetPagerPosition: Int, target: PreviewItem) {
        showTransitionOverlayForSwitchTarget(target)
        viewPager?.setCurrentItem(targetPagerPosition, false)
    }

    private fun advanceToPagerPosition(targetPagerPosition: Int) {
        val targetIndex = realIndexFor(targetPagerPosition)
        val target = previewItems.getOrNull(targetIndex)
        if (target == null) {
            return
        }
        // Image → video: pre-warm the shared player so first-frame is rendered
        // *before* we hand the user the new page. Otherwise the page's black
        // container background shows through until the player catches up — an
        // overlay snapshot can hide the gap but the moment we hide it the
        // visual still pops abruptly.
        if (shouldPrewarmTargetVideoBeforeSwap(target)) {
            prewarmVideoBeforeSwap(targetPagerPosition, target)
            return
        }
        if (shouldPrefetchBeforePagerSwitch(target)) {
            val context = context
            if (context != null) {
                cancelPendingSwitchPrefetch()
                val generation = pendingSwitchPrefetchGeneration
                pendingSwitchPrefetch = PreviewPagerAdapter.prefetchItemVisual(
                    context = context,
                    item = target,
                    runtimeProfile = runtimeProfile
                ) {
                    if (finished || generation != pendingSwitchPrefetchGeneration) return@prefetchItemVisual
                    pendingSwitchPrefetch = null
                    switchToPagerPositionWithVisualOverlay(targetPagerPosition, target)
                }
                return
            }
        }
        switchToPagerPositionWithVisualOverlay(targetPagerPosition, target)
    }

    private fun shouldPrewarmTargetVideoBeforeSwap(target: PreviewItem): Boolean {
        if (target.mediaType != MediaPreviewType.VIDEO) return false
        if (sharedPlayerHost == null) return false
        val current = previewItems.getOrNull(currentIndex) ?: return false
        // Video → video already cross-fades through the shared player itself.
        // Only image → video benefits from prewarm.
        if (current.mediaType == MediaPreviewType.VIDEO) return false
        // A non-empty playlist must exist for the target to be playable.
        if (sharedPlaylistItems.isEmpty()) {
            computeVideoPlaylist()
            if (sharedPlaylistItems.isEmpty()) return false
        }
        val targetIndex = previewItems.indexOf(target).takeIf { it >= 0 } ?: return false
        val playlistIdx = videoPlaylistIndexByPreviewIndex.getOrNull(targetIndex) ?: return false
        return playlistIdx >= 0
    }

    private fun prewarmVideoBeforeSwap(targetPagerPosition: Int, target: PreviewItem) {
        val host = sharedPlayerHost ?: run {
            switchToPagerPositionWithVisualOverlay(targetPagerPosition, target)
            return
        }
        val targetIndex = realIndexFor(targetPagerPosition)
        computeVideoPlaylist()
        val playlistIdx = videoPlaylistIndexByPreviewIndex
            .getOrNull(targetIndex)?.takeIf { it >= 0 } ?: run {
                switchToPagerPositionWithVisualOverlay(targetPagerPosition, target)
                return
            }

        // Defensive cover for the swap: if prewarm somehow stalls and we time
        // out into the immediate-swap fallback, the overlay snapshot bridges
        // the gap. While prewarm is in flight the user keeps seeing the
        // current image — overlay is harmless on top of it.
        showTransitionOverlayFromCurrentVisual()

        cancelPendingPrewarm()

        sharedPlayerActivationGen += 1L
        val prewarmGen = sharedPlayerActivationGen
        // Targeting the *future* page so the eventSink's
        // expectedPagerPosition routing knows this gen belongs to a prewarm.
        sharedPlayerActivationPager = targetPagerPosition
        // Reset the seek gate so the prewarm itself can seek to 0 once.
        // The post-swap re-entry will see seekedGen == activationGen and skip
        // seeking again, preserving the existing "seek-once-per-activation"
        // invariant.
        sharedPlayerSeekedGen = -1L
        logPlaybackState("prewarm_start", "playlistIdx=$playlistIdx gen=$prewarmGen")

        val timeoutRunnable = Runnable {
            if (finished || pendingPrewarmGen != prewarmGen) return@Runnable
            Log.w(TAG, "Prewarm timed out; falling back to immediate swap")
            clearPendingPrewarm()
            switchToPagerPositionWithVisualOverlay(targetPagerPosition, target)
        }
        pendingPrewarmTimeout = timeoutRunnable
        pendingPrewarmCommit = commit@{
            if (finished || pendingPrewarmGen != prewarmGen) return@commit
            clearPendingPrewarm()
            logPlaybackState("prewarm_commit", "gen=$prewarmGen")
            // Mark first-frame already-rendered for this activation so the
            // post-swap re-entry of applySharedPlayerForCurrentItem skips the
            // host-hide branch.
            sharedPlayerFirstFrameGen = prewarmGen
            suppressScrollHostHideForPrewarmGen = prewarmGen
            host.animate().cancel()
            host.alpha = 0f
            host.visibility = View.VISIBLE
            // Swap pages atomically. Player is already covering with the
            // target's first frame; the new page underneath is invisible.
            viewPager?.setCurrentItem(targetPagerPosition, false)
            // Keep the player in the composition for a few frames before
            // revealing it. On slower TVs firstframerendered/playing can
            // arrive before the TextureView's visible buffer is latched.
            val revealFrames = if (normalizePreviewRotation(target.rotate) == 0) 1 else 3
            postAfterAnimationFrames(host, revealFrames) {
                if (!finished &&
                    prewarmGen == sharedPlayerActivationGen &&
                    targetPagerPosition == currentPagerPosition
                ) {
                    host.alpha = 1f
                    suppressScrollHostHideForPrewarmGen = -1L
                    host.postDelayed({
                        if (!finished &&
                            prewarmGen == sharedPlayerActivationGen &&
                            targetPagerPosition == currentPagerPosition
                        ) {
                            hideTransitionOverlay()
                        }
                    }, 50L)
                } else if (suppressScrollHostHideForPrewarmGen == prewarmGen) {
                    suppressScrollHostHideForPrewarmGen = -1L
                }
            }
            previewAdapter?.notifyVideoRenderedAt(targetPagerPosition)
        }
        pendingPrewarmGen = prewarmGen
        mainHandler.postDelayed(timeoutRunnable, PREWARM_TIMEOUT_MS)

        // Keep the player view attached and in the composition during prewarm,
        // but transparent while the image page remains on screen. This gives
        // TextureView a chance to latch the decoded frame before the swap.
        host.animate().cancel()
        host.alpha = 0f
        host.visibility = View.VISIBLE

        val player = ensureSharedPlayer()
        if (!sharedPlaylistApplied) {
            player.update(
                LxMediaPlayerConfig(
                    poster = null,
                    autoplay = false,
                    loop = shouldLoopSingleItemVideo(),
                    controls = true,
                )
            )
            player.applyPlaylist(
                items = sharedPlaylistItems,
                autoAdvance = false,
                startingIndex = playlistIdx,
            )
            sharedPlaylistApplied = true
        }
        player.playlistGoToIndex(playlistIdx)
        sharedPlayerSeekedGen = prewarmGen
        player.seek(0.0)
        player.play()
    }

    private fun cancelPendingPrewarm() {
        pendingPrewarmTimeout?.let { mainHandler.removeCallbacks(it) }
        clearPendingPrewarm()
    }

    private fun clearPendingPrewarm() {
        pendingPrewarmGen = -1L
        pendingPrewarmCommit = null
        pendingPrewarmTimeout = null
    }

    private fun shouldPrefetchBeforePagerSwitch(item: PreviewItem): Boolean {
        val context = context ?: return false
        return when (item.mediaType) {
            MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN ->
                isLocalUri(item.uri) && PreviewPagerAdapter.getCachedLocalImage(context, item.uri) == null
            MediaPreviewType.VIDEO ->
                isLocalUri(item.uri) &&
                    runtimeProfile.enableLocalVideoFirstFrameExtraction &&
                    PreviewPagerAdapter.getCachedVideoFirstFrame(context, item.uri) == null
        }
    }

    private fun shouldPrefetchUpcomingVisual(item: PreviewItem): Boolean {
        if (runtimeProfile.enableUpcomingVisualPrefetch) return true
        return runtimeProfile.isConstrainedDevice && shouldConstrainedPrefetchImage(item)
    }

    private fun shouldConstrainedPrefetchImage(item: PreviewItem): Boolean {
        return item.mediaType != MediaPreviewType.VIDEO && isLocalUri(item.uri)
    }

    private fun finishPreview(reason: String) {
        if (finished) return
        finished = true
        clearAutoRunnables()
        hideTransitionOverlay()
        sendPreviewResult(reason)
        if (!isAdded) {
            cleanupPreviewResources()
            return
        }

        // Restore system UI before removing the overlay to avoid a visible close flash.
        restoreWindowUiIfNeeded()

        val fm = parentFragmentManager
        if (fm.isStateSaved) {
            fm.beginTransaction()
                .setReorderingAllowed(true)
                .remove(this)
                .commitAllowingStateLoss()
            return
        }

        fm.popBackStack(TAG, FragmentManager.POP_BACK_STACK_INCLUSIVE)
    }

    private fun clampIndex(position: Int): Int {
        if (previewItems.isEmpty()) return 0
        return position.coerceIn(0, previewItems.lastIndex)
    }

    private fun shouldUseLoopPager(): Boolean {
        return advance == PreviewAdvance.LOOP && previewItems.size > 1
    }

    private fun shouldLoopSingleItemVideo(): Boolean {
        return advance == PreviewAdvance.LOOP && previewItems.size == 1
    }

    private fun realIndexFor(position: Int): Int {
        if (previewItems.isEmpty()) return 0
        if (!shouldUseLoopPager()) return clampIndex(position)
        val size = previewItems.size
        val normalized = position % size
        return if (normalized >= 0) normalized else normalized + size
    }

    private fun initialPagerPosition(startIndex: Int): Int {
        if (!shouldUseLoopPager()) {
            return clampIndex(startIndex)
        }
        val size = previewItems.size
        val midpoint = Int.MAX_VALUE / 2
        val base = midpoint - (midpoint % size)
        return base + clampIndex(startIndex)
    }

    private fun logPlaybackState(event: String, extra: String? = null) {
        val item = previewItems.getOrNull(currentIndex)
        val uriTail = item?.uri?.toString()?.let { value ->
            if (value.length <= 72) value else value.takeLast(72)
        } ?: "-"
        val host = sharedPlayerHost
        val details = buildString {
            append("event=").append(event)
            append(" pager=").append(currentPagerPosition)
            append(" index=").append(currentIndex).append('/').append(previewItems.size)
            append(" type=").append(item?.mediaType ?: "-")
            append(" advance=").append(advance)
            append(" scroll=").append(scrollStateName(sharedPlayerScrollState))
            append(" gen=").append(sharedPlayerActivationGen)
            append(" activationPager=").append(sharedPlayerActivationPager)
            append(" seekedGen=").append(sharedPlayerSeekedGen)
            append(" host=").append(visibilityName(host?.visibility))
            append(" hostAlpha=").append(host?.alpha ?: -1f)
            append(" hostSize=").append(host?.width ?: -1).append('x').append(host?.height ?: -1)
            append(" controls=").append(sharedPlayer?.isControlsVisible() == true)
            append(" uri=").append(uriTail)
            if (!extra.isNullOrBlank()) {
                append(' ').append(extra)
            }
        }
        Log.i(LOG_TAG, details)
    }

    private fun logSharedPlayerEvent(payload: Map<String, Any>) {
        val event = payload["event"] as? String ?: return
        val detail = payload["detail"] as? Map<*, *>
        if (event == "timeupdate") {
            val currentMs = ((detail?.get("currentTime") as? Number)?.toDouble()?.times(1000.0))?.toLong()
            if (currentMs != null &&
                lastLoggedPlayerTimeUpdateMs != Long.MIN_VALUE &&
                currentMs >= lastLoggedPlayerTimeUpdateMs &&
                currentMs - lastLoggedPlayerTimeUpdateMs < PLAYER_TIMEUPDATE_LOG_INTERVAL_MS
            ) {
                return
            }
            lastLoggedPlayerTimeUpdateMs = currentMs ?: lastLoggedPlayerTimeUpdateMs
        }

        val compactDetail = when (event) {
            "timeupdate" -> {
                val current = detail?.get("currentTime")
                val duration = detail?.get("duration")
                "current=$current duration=$duration"
            }
            "seeked" -> "time=${detail?.get("time")}"
            "error" -> "code=${detail?.get("code")} message=${detail?.get("message")}"
            else -> detail?.toString() ?: "{}"
        }
        logPlaybackState("player_event", "playerEvent=$event detail=$compactDetail")
    }

    private fun scrollStateName(state: Int): String = when (state) {
        ViewPager2.SCROLL_STATE_IDLE -> "IDLE"
        ViewPager2.SCROLL_STATE_DRAGGING -> "DRAGGING"
        ViewPager2.SCROLL_STATE_SETTLING -> "SETTLING"
        else -> state.toString()
    }

    private fun visibilityName(visibility: Int?): String = when (visibility) {
        View.VISIBLE -> "VISIBLE"
        View.INVISIBLE -> "INVISIBLE"
        View.GONE -> "GONE"
        null -> "null"
        else -> visibility.toString()
    }

    companion object {
        private const val ARG_PAYLOADS = "arg_payloads"
        private const val ARG_START_INDEX = "arg_start_index"
        private const val ARG_ADVANCE = "arg_advance"
        private const val ARG_SHOW_INDEX_INDICATOR = "arg_show_index_indicator"
        private const val ARG_CALLBACK_ID = "arg_callback_id"
        private const val ARG_PRESENTED_CALLBACK_ID = "arg_presented_callback_id"
        private const val TAG = "MediaPreviewOverlay"
        private const val LOG_TAG = "LingXia.MediaPreview"
        private const val PLAYER_TIMEUPDATE_LOG_INTERVAL_MS = 5_000L
        // Cap on how long we'll wait for the shared player to emit a
        // firstframerendered event during an image→video prewarm before
        // giving up and falling back to the legacy immediate-swap path. On
        // a flaky network / corrupt source we'd otherwise stall the
        // auto-advance indefinitely.
        private const val PREWARM_TIMEOUT_MS = 2_500L

        fun show(
            activity: AppCompatActivity,
            payloads: Array<PreviewMediaPayload>,
            startIndex: Int,
            advance: String,
            showIndexIndicator: Boolean,
            callbackId: Long,
            presentedCallbackId: Long
        ) {
            val fm = activity.supportFragmentManager
            (fm.findFragmentByTag(TAG) as? MediaPreviewFragment)?.finishPreview("interrupted")

            val firstItem = payloads
                .getOrNull(startIndex.coerceIn(0, payloads.lastIndex))
                ?.let { previewItemFromPayload(it) }
            if (firstItem != null) {
                val runtimeProfile = PreviewRuntimeProfile.from(activity)
                PreviewPagerAdapter.prefetchItemVisual(
                    context = activity,
                    item = firstItem,
                    runtimeProfile = runtimeProfile
                ) {
                    activity.runOnUiThread {
                        showNow(
                            activity = activity,
                            payloads = payloads,
                            startIndex = startIndex,
                            advance = advance,
                            showIndexIndicator = showIndexIndicator,
                            callbackId = callbackId,
                            presentedCallbackId = presentedCallbackId
                        )
                    }
                }
                return
            }

            showNow(
                activity = activity,
                payloads = payloads,
                startIndex = startIndex,
                advance = advance,
                showIndexIndicator = showIndexIndicator,
                callbackId = callbackId,
                presentedCallbackId = presentedCallbackId
            )
        }

        private fun showNow(
            activity: AppCompatActivity,
            payloads: Array<PreviewMediaPayload>,
            startIndex: Int,
            advance: String,
            showIndexIndicator: Boolean,
            callbackId: Long,
            presentedCallbackId: Long
        ) {
            val fm = activity.supportFragmentManager
            val fragment = MediaPreviewFragment().apply {
                arguments = Bundle().apply {
                    putSerializable(ARG_PAYLOADS, ArrayList(payloads.toList()))
                    putInt(ARG_START_INDEX, startIndex)
                    putString(ARG_ADVANCE, advance)
                    putBoolean(ARG_SHOW_INDEX_INDICATOR, showIndexIndicator)
                    putLong(ARG_CALLBACK_ID, callbackId)
                    putLong(ARG_PRESENTED_CALLBACK_ID, presentedCallbackId)
                }
            }

            fm.beginTransaction()
                .setReorderingAllowed(true)
                .add(android.R.id.content, fragment, TAG)
                .addToBackStack(TAG)
                .commitAllowingStateLoss()
            fm.executePendingTransactions()
        }

        private fun previewItemFromPayload(payload: PreviewMediaPayload): PreviewItem {
            return PreviewItem(
                uri = normalizeUri(payload.path),
                mediaType = MediaPreviewType.fromInt(payload.type),
                rotate = payload.rotate,
                objectFit = payload.objectFit?.let { LxMediaObjectFit.fromString(it) },
                durationMs = payload.durationMs?.takeIf { it > 0L }
            )
        }

        fun close(activity: AppCompatActivity, callbackId: Long) {
            val fragment = activity.supportFragmentManager.findFragmentByTag(TAG) as? MediaPreviewFragment
            if (fragment != null && fragment.callbackId == callbackId) {
                fragment.finishPreview("interrupted")
            }
        }
    }
}

private data class PreviewItem(
    val uri: Uri,
    val mediaType: MediaPreviewType,
    val rotate: Int?,
    val objectFit: LxMediaObjectFit?,
    val durationMs: Long?
)

private fun normalizePreviewRotation(value: Int?): Int {
    val raw = value ?: return 0
    val normalized = raw % 360
    return if (normalized < 0) normalized + 360 else normalized
}

private data class PreviewRuntimeProfile(
    val isConstrainedDevice: Boolean,
    val enableVisualPrefetchBeforeSwitch: Boolean,
    val enableUpcomingVisualPrefetch: Boolean,
    val enableLocalVideoFirstFrameExtraction: Boolean
) {
    companion object {
        fun default(): PreviewRuntimeProfile = PreviewRuntimeProfile(
            isConstrainedDevice = false,
            enableVisualPrefetchBeforeSwitch = true,
            enableUpcomingVisualPrefetch = true,
            enableLocalVideoFirstFrameExtraction = true
        )

        fun from(context: Context): PreviewRuntimeProfile {
            val appContext = context.applicationContext
            val manager = appContext.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
            val isLowRamDevice = manager?.isLowRamDevice == true
            val memoryClass = manager?.memoryClass ?: 256
            val constrained = isLowRamDevice || memoryClass <= 256
            return PreviewRuntimeProfile(
                isConstrainedDevice = constrained,
                enableVisualPrefetchBeforeSwitch = true,
                enableUpcomingVisualPrefetch = true,
                enableLocalVideoFirstFrameExtraction = true
            )
        }
    }
}

private enum class InPlaceImageReplaceResult {
    APPLIED,
    NO_ADAPTER,
    NO_PAGER,
    NO_VISIBLE_HOLDER,
    HOLDER_REJECTED
}

private enum class MediaPreviewType(val value: Int) {
    IMAGE(0),
    VIDEO(1),
    UNKNOWN(-1);

    companion object {
        fun fromInt(value: Int): MediaPreviewType = when (value) {
            1 -> VIDEO
            0 -> IMAGE
            else -> UNKNOWN
        }
    }
}

private fun statusBarHeight(context: Context): Int {
    val resourceId = context.resources.getIdentifier("status_bar_height", "dimen", "android")
    return if (resourceId > 0) context.resources.getDimensionPixelSize(resourceId) else 0
}

private fun normalizeUri(raw: String?): Uri {
    if (raw.isNullOrBlank()) return Uri.EMPTY
    val trimmed = raw.trim()
    val resolved = resolveLxUriIfNeeded(trimmed) ?: trimmed
    return try {
        val parsed = Uri.parse(resolved)
        if (parsed.scheme.isNullOrEmpty()) {
            val file = File(resolved)
            Uri.fromFile(file)
        } else {
            parsed
        }
    } catch (_: Exception) {
        Uri.EMPTY
    }
}

private fun resolveLxUriIfNeeded(input: String): String? {
    if (!input.startsWith("lx://", ignoreCase = true)) {
        return null
    }
    val appId = LxApp.getCurrentActivity()?.getAppId() ?: return null
    val resolved = NativeApi.resolveLxUri(appId, input)
    return resolved?.takeIf { it.isNotBlank() }
}

private class PreviewPagerAdapter(
    private val items: List<PreviewItem>,
    private val loopEnabled: Boolean,
    private val loopSingleItemVideo: Boolean,
    private val runtimeProfile: PreviewRuntimeProfile,
    private val userInputEnabled: Boolean,
    private val onDismiss: () -> Unit,
    private val onVideoTerminal: (Int, String) -> Unit,
    private val onItemVisualReady: (Int) -> Unit
) : RecyclerView.Adapter<PreviewPagerAdapter.MediaViewHolder>() {

    private var viewPager: ViewPager2? = null
    private var currentPosition: Int = RecyclerView.NO_POSITION
    private var pendingHidePosition: Int = RecyclerView.NO_POSITION
    private var visibleHolder: MediaViewHolder? = null

    fun notifyVideoRenderedAt(position: Int) {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView ?: return
        val holder = recyclerView.findViewHolderForAdapterPosition(position) as? MediaViewHolder
            ?: return
        holder.markVideoRendered()
    }

    fun attachToViewPager(pager: ViewPager2) {
        viewPager = pager
    }

    fun release() {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView
        recyclerView?.let { rv ->
            for (index in 0 until rv.childCount) {
                val holder = rv.getChildViewHolder(rv.getChildAt(index))
                if (holder is MediaViewHolder) {
                    holder.onHidden()
                    holder.reset()
                }
            }
        }
        viewPager = null
        currentPosition = RecyclerView.NO_POSITION
        pendingHidePosition = RecyclerView.NO_POSITION
        visibleHolder = null
    }

    fun snapshotCurrentVisualBitmap(): Bitmap? {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView ?: return null
        val holder = recyclerView.findViewHolderForAdapterPosition(currentPosition) as? MediaViewHolder
            ?: return null
        return holder.snapshotVisualBitmap()
    }

    fun isPositionVisualReady(position: Int): Boolean {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView ?: return false
        val holder = recyclerView.findViewHolderForAdapterPosition(position) as? MediaViewHolder
            ?: return false
        return holder.isVisualReadyForDisplay()
    }

    fun replaceCurrentImage(item: PreviewItem, bitmap: Bitmap): InPlaceImageReplaceResult {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView
            ?: return InPlaceImageReplaceResult.NO_PAGER
        val holder = findCurrentImageHolder(recyclerView)
            ?: return InPlaceImageReplaceResult.NO_VISIBLE_HOLDER
        return if (holder.replaceImage(item, bitmap)) {
            visibleHolder = holder
            InPlaceImageReplaceResult.APPLIED
        } else {
            InPlaceImageReplaceResult.HOLDER_REJECTED
        }
    }

    fun onPageSelected(position: Int) {
        if (position == currentPosition) return

        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView
        val previousPosition = currentPosition

        currentPosition = position
        pendingHidePosition = previousPosition
        val currentHolder = recyclerView
            ?.findViewHolderForAdapterPosition(position)
        if (currentHolder is MediaViewHolder) {
            visibleHolder = currentHolder
            currentHolder.onVisible()
            if (currentHolder.isVisualReadyForDisplay()) {
                hidePreviousHolderIfNeeded(recyclerView, previousPosition)
            }
        }
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): MediaViewHolder {
        val container = FrameLayout(parent.context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            setBackgroundColor(Color.BLACK)
        }
        return MediaViewHolder(
            container = container,
            runtimeProfile = runtimeProfile,
            onZoomStateChanged = { zoomed -> viewPager?.isUserInputEnabled = userInputEnabled && !zoomed },
            onDismiss = onDismiss,
            onVideoTerminal = { _ -> },
            onVisualReady = { }
        )
    }

    override fun onBindViewHolder(holder: MediaViewHolder, position: Int) {
        holder.setVideoTerminalHandler { terminal ->
            val adapterPosition = holder.bindingAdapterPosition
            if (adapterPosition != RecyclerView.NO_POSITION) {
                onVideoTerminal(adapterPosition, terminal)
            }
        }
        holder.setVisualReadyHandler {
            val adapterPosition = holder.bindingAdapterPosition
            if (adapterPosition != RecyclerView.NO_POSITION) {
                onItemVisualReady(adapterPosition)
                if (adapterPosition == currentPosition) {
                    hidePreviousHolderIfNeeded(
                        viewPager?.getChildAt(0) as? RecyclerView,
                        pendingHidePosition
                    )
                }
            }
        }
        holder.bind(
            items[realIndexFor(position)],
            loopSingleItemVideo
        )
        if (position == currentPosition) {
            visibleHolder = holder
            holder.onVisible()
        }
    }

    override fun getItemCount(): Int = if (loopEnabled) Int.MAX_VALUE else items.size

    override fun onViewAttachedToWindow(holder: MediaViewHolder) {
        super.onViewAttachedToWindow(holder)
        if (holder.bindingAdapterPosition == currentPosition) {
            visibleHolder = holder
            holder.onVisible()
        }
    }

    override fun onViewRecycled(holder: MediaViewHolder) {
        if (visibleHolder == holder) {
            visibleHolder = null
        }
        holder.onHidden()
        holder.reset()
        super.onViewRecycled(holder)
    }

        private fun realIndexFor(position: Int): Int {
            if (items.isEmpty()) return 0
            if (!loopEnabled) return position.coerceIn(0, items.lastIndex)
            val normalized = position % items.size
            return if (normalized >= 0) normalized else normalized + items.size
        }

    private fun hidePreviousHolderIfNeeded(
        recyclerView: RecyclerView?,
        previousPosition: Int
    ) {
        if (previousPosition == RecyclerView.NO_POSITION) {
            pendingHidePosition = RecyclerView.NO_POSITION
            return
        }
        if (previousPosition == currentPosition) {
            pendingHidePosition = RecyclerView.NO_POSITION
            return
        }
        val previousHolder = recyclerView
            ?.findViewHolderForAdapterPosition(previousPosition)
        if (previousHolder is MediaViewHolder) {
            previousHolder.onHidden()
            pendingHidePosition = RecyclerView.NO_POSITION
        } else {
            pendingHidePosition = previousPosition
        }
    }

    private fun findCurrentImageHolder(recyclerView: RecyclerView): MediaViewHolder? {
        visibleHolder
            ?.takeIf { it.isAttachedForPreview() && it.canReplaceImageInPlace() }
            ?.let { return it }

        (recyclerView.findViewHolderForAdapterPosition(currentPosition) as? MediaViewHolder)
            ?.takeIf { it.canReplaceImageInPlace() }
            ?.let { return it }

        for (index in 0 until recyclerView.childCount) {
            val holder = recyclerView.getChildViewHolder(recyclerView.getChildAt(index))
            if (holder is MediaViewHolder && holder.canReplaceImageInPlace()) {
                return holder
            }
        }
        return null
    }

    class MediaViewHolder(
        private val container: FrameLayout,
        private val runtimeProfile: PreviewRuntimeProfile,
        private val onZoomStateChanged: (Boolean) -> Unit,
        private val onDismiss: () -> Unit,
        private var onVideoTerminal: (String) -> Unit,
        private var onVisualReady: () -> Unit
    ) : RecyclerView.ViewHolder(container) {
        private var currentLoader: Future<*>? = null
        private var boundItem: PreviewItem? = null
        private var imageView: ZoomImageView? = null
        private var loopVideoPlayback: Boolean = false
        private var videoFrameView: ImageView? = null
        private var frameLoadGeneration: Long = 0L
        private var visualReadyForDisplay: Boolean = false
        private var displayGeneration: Long = 0L

        fun bind(
            item: PreviewItem,
            loopVideoPlayback: Boolean
        ) {
            reset()
            val generation = displayGeneration + 1L
            displayGeneration = generation
            container.removeAllViews()
            boundItem = item
            this.loopVideoPlayback = loopVideoPlayback
            when (item.mediaType) {
                MediaPreviewType.VIDEO -> bindVideoFramePlaceholder(item, displayGeneration = generation)
                MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> bindImage(item, generation)
            }
        }

        fun markVideoRendered() {
            // Shared player has rendered its first frame for the current
            // video; no per-holder bookkeeping is needed in this design — the
            // fragment-level player view sits on top of the holder, so the
            // placeholder beneath is already covered. Visibility is also
            // already reported via onVideoFrameReady when the placeholder
            // bitmap is loaded.
            // Kept as an explicit hook in case future overlays need it.
        }

        fun setVideoTerminalHandler(handler: (String) -> Unit) {
            onVideoTerminal = handler
        }

        fun setVisualReadyHandler(handler: () -> Unit) {
            onVisualReady = handler
        }

        fun isVisualReadyForDisplay(): Boolean = visualReadyForDisplay

        fun isAttachedForPreview(): Boolean = itemView.parent != null

        fun canReplaceImageInPlace(): Boolean {
            return imageView != null && boundItem?.mediaType != MediaPreviewType.VIDEO
        }

        fun snapshotVisualBitmap(): Bitmap? {
            val width = container.width
            val height = container.height
            if (width <= 0 || height <= 0) return null
            for (scale in floatArrayOf(1f, 0.5f, 0.25f)) {
                val scaledWidth = max(1, (width * scale).toInt())
                val scaledHeight = max(1, (height * scale).toInt())
                val bitmap = try {
                    Bitmap.createBitmap(scaledWidth, scaledHeight, Bitmap.Config.RGB_565)
                } catch (_: OutOfMemoryError) {
                    null
                } catch (_: Exception) {
                    null
                } ?: continue
                try {
                    val canvas = Canvas(bitmap)
                    if (scale != 1f) {
                        canvas.scale(scale, scale)
                    }
                    container.draw(canvas)
                    return bitmap
                } catch (_: OutOfMemoryError) {
                    bitmap.recycle()
                } catch (_: Exception) {
                    bitmap.recycle()
                }
            }
            return null
        }

        fun reset() {
            currentLoader?.cancel(true)
            currentLoader = null
            boundItem = null
            imageView = null
            videoFrameView = null
            frameLoadGeneration += 1L
            visualReadyForDisplay = false
            clearImageReferences(container)
            container.removeAllViews()
        }

        fun replaceImage(item: PreviewItem, bitmap: Bitmap): Boolean {
            if (item.mediaType == MediaPreviewType.VIDEO) return false
            val zoomImageView = imageView ?: return false
            currentLoader?.cancel(true)
            currentLoader = null
            val generation = displayGeneration + 1L
            displayGeneration = generation
            boundItem = item
            visualReadyForDisplay = false
            zoomImageView.setPreviewRotationDegrees(item.rotate)
            zoomImageView.setPreviewObjectFit(item.objectFit)
            zoomImageView.setImageBitmap(bitmap)
            onZoomStateChanged(false)
            notifyVisualReady(generation)
            return true
        }

        private fun notifyVisualReady(displayGeneration: Long) {
            container.postOnAnimation {
                if (displayGeneration != this.displayGeneration) return@postOnAnimation
                container.postOnAnimation {
                    if (displayGeneration != this.displayGeneration) return@postOnAnimation
                    visualReadyForDisplay = true
                    onVisualReady()
                }
            }
        }

        private fun bindImage(item: PreviewItem, displayGeneration: Long) {
            val context = container.context
            val zoomImageView = ZoomImageView(context)
            imageView = zoomImageView
            val progressBar = ProgressBar(context).apply {
                layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.CENTER)
                visibility = View.GONE
            }

            container.addView(zoomImageView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            container.addView(progressBar)
            zoomImageView.setTapToDismissEnabled(true)
            zoomImageView.setDismissListener(onDismiss)
            zoomImageView.setOnScaleStateListener { zoomed -> onZoomStateChanged(zoomed) }
            zoomImageView.setPreviewRotationDegrees(item.rotate)
            zoomImageView.setPreviewObjectFit(item.objectFit)

            val uri = item.uri

            if (uri == Uri.EMPTY) {
                zoomImageView.setImageResource(android.R.drawable.ic_dialog_alert)
                notifyVisualReady(displayGeneration)
                return
            }

            if (isLocalUri(uri)) {
                ImageLoader.getCachedLocalImage(context, uri)?.let { cached ->
                    zoomImageView.setImageBitmap(cached)
                    notifyVisualReady(displayGeneration)
                    return
                }
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadLocal(
                    context,
                    uri,
                    zoomImageView,
                    progressBar,
                    onComplete = { notifyVisualReady(displayGeneration) }
                )
            } else {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadRemote(
                    uri.toString(),
                    zoomImageView,
                    progressBar,
                    onComplete = { notifyVisualReady(displayGeneration) }
                )
            }
        }

        private fun bindVideoFramePlaceholder(
            item: PreviewItem,
            displayGeneration: Long
        ) {
            currentLoader?.cancel(true)
            currentLoader = null
            val generation = frameLoadGeneration + 1L
            frameLoadGeneration = generation

            val context = container.context
            val mediaUri = item.uri

            val frameView = PreviewVideoPosterView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                setPreviewObjectFit(item.objectFit)
                setPreviewRotationDegrees(item.rotate)
            }
            videoFrameView = frameView
            container.addView(
                frameView,
                FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            )

            if (mediaUri == Uri.EMPTY) {
                // Reset orientation for the alert glyph so it isn't rotated.
                frameView.setPreviewRotationDegrees(0)
                frameView.setPreviewObjectFit(LxMediaObjectFit.CONTAIN)
                frameView.setImageResource(android.R.drawable.ic_dialog_alert)
                notifyVisualReady(displayGeneration)
                return
            }

            if (isLocalUri(mediaUri)) {
                if (!runtimeProfile.enableLocalVideoFirstFrameExtraction) {
                    notifyVisualReady(displayGeneration)
                    return
                }
                ImageLoader.getCachedVideoFirstFrame(context, mediaUri)?.let { cached ->
                    frameView.setImageBitmap(cached)
                    notifyVisualReady(displayGeneration)
                    return
                }
                currentLoader = ImageLoader.loadVideoFirstFrame(
                    context = context,
                    uri = mediaUri,
                    target = frameView,
                    onComplete = {
                        if (generation == frameLoadGeneration) {
                            notifyVisualReady(displayGeneration)
                        }
                    }
                )
                return
            }

            // Remote video: no local first-frame extraction; rely on the
            // shared player's poster/buffering display once it takes over.
            notifyVisualReady(displayGeneration)
        }

        private fun clearImageReferences(view: View?) {
            view ?: return
            if (view is ImageView) {
                view.setImageDrawable(null)
            }
            if (view is ViewGroup) {
                for (index in 0 until view.childCount) {
                    clearImageReferences(view.getChildAt(index))
                }
            }
        }

        fun onVisible() {
            // Player lifecycle is now driven by MediaPreviewFragment via the
            // shared LxMediaPlayer. Holders only own the placeholder; nothing
            // to do here.
        }

        fun onHidden() {
            // Mirror of onVisible: fragment-level player handles pause/seek
            // when the current item changes. Holder just keeps its placeholder.
        }
    }

    companion object ImageLoader {
        private const val MAX_REMOTE_IMAGE_BYTES = 12 * 1024 * 1024
        private const val MAX_LOCAL_CONTENT_IMAGE_BYTES = 12 * 1024 * 1024
        private const val MIN_LOCAL_IMAGE_CACHE_BYTES = 2 * 1024 * 1024
        private const val MAX_LOCAL_IMAGE_CACHE_BYTES = 16 * 1024 * 1024
        private const val CONSTRAINED_MIN_LOCAL_IMAGE_CACHE_BYTES = 1 * 1024 * 1024
        private const val CONSTRAINED_MAX_LOCAL_IMAGE_CACHE_BYTES = 8 * 1024 * 1024
        private const val LOCAL_IMAGE_CACHE_DIVISOR = 96L
        private const val CONSTRAINED_LOCAL_IMAGE_CACHE_DIVISOR = 256L
        private const val CONSTRAINED_HEAP_THRESHOLD_BYTES = 256L * 1024L * 1024L
        // Video first-frame cache lives in LocalVideoFrameCache (shared with
        // LxMediaPlayer). Preview only owns the local-image LRU.
        private val mainHandler = Handler(Looper.getMainLooper())
        @Volatile
        private var memoryCallbacksRegistered: Boolean = false
        private val localImageCache = object : LruCache<String, Bitmap>(
            resolveLocalImageCacheSizeBytes()
        ) {
            override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount.coerceAtLeast(1)
        }
        private var pinnedLocalImageKey: String? = null
        private var pinnedLocalImage: Bitmap? = null

        private val executor = Executors.newFixedThreadPool(resolveDecodeThreadCount()) { runnable ->
            Thread(runnable, "LingXiaPreviewImage").apply { isDaemon = true }
        }

        private fun useConstrainedStrategy(): Boolean {
            if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.LOLLIPOP_MR1) {
                return true
            }
            val maxHeapBytes = Runtime.getRuntime().maxMemory()
            return maxHeapBytes in 1 until CONSTRAINED_HEAP_THRESHOLD_BYTES
        }

        private fun resolveDecodeThreadCount(): Int {
            return if (useConstrainedStrategy()) 1 else 2
        }

        private fun resolveLocalImageCacheSizeBytes(): Int {
            val constrained = useConstrainedStrategy()
            val minCache = if (constrained) {
                CONSTRAINED_MIN_LOCAL_IMAGE_CACHE_BYTES
            } else {
                MIN_LOCAL_IMAGE_CACHE_BYTES
            }
            val maxCache = if (constrained) {
                CONSTRAINED_MAX_LOCAL_IMAGE_CACHE_BYTES
            } else {
                MAX_LOCAL_IMAGE_CACHE_BYTES
            }
            val divisor = if (constrained) {
                CONSTRAINED_LOCAL_IMAGE_CACHE_DIVISOR
            } else {
                LOCAL_IMAGE_CACHE_DIVISOR
            }
            val maxHeapBytes = Runtime.getRuntime().maxMemory().coerceAtLeast(minCache.toLong())
            val budget = (maxHeapBytes / divisor).toInt()
            return budget.coerceIn(minCache, maxCache)
        }

        private fun ensureMemoryCallbacksRegistered(context: Context) {
            if (memoryCallbacksRegistered) return
            synchronized(this) {
                if (memoryCallbacksRegistered) return
                val appContext = context.applicationContext
                appContext.registerComponentCallbacks(object : ComponentCallbacks2 {
                    @Suppress("DEPRECATION")
                    override fun onTrimMemory(level: Int) {
                        // Video first-frame eviction is owned by LocalVideoFrameCache.
                        synchronized(localImageCache) {
                            when {
                                level >= ComponentCallbacks2.TRIM_MEMORY_COMPLETE ||
                                    level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL -> {
                                    localImageCache.evictAll()
                                    clearPinnedLocalImageLocked()
                                }
                                level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND ||
                                    level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE -> {
                                    localImageCache.trimToSize(localImageCache.maxSize() / 2)
                                }
                            }
                        }
                    }

                    override fun onLowMemory() {
                        clearVisualCaches()
                    }

                    override fun onConfigurationChanged(newConfig: Configuration) = Unit
                })
                memoryCallbacksRegistered = true
            }
        }

        private fun buildLocalImageCacheKey(context: Context, uri: Uri): String {
            return buildLocalVisualCacheKey(context, uri, prefix = "image")
        }

        private fun buildLocalVisualCacheKey(context: Context, uri: Uri, prefix: String): String {
            val scheme = uri.scheme?.lowercase()
            return if (scheme.isNullOrEmpty() || scheme == "file") {
                val path = uri.path.orEmpty()
                val file = File(path)
                if (path.isNotEmpty() && file.exists()) {
                    "$prefix:file:$path:${file.length()}:${file.lastModified()}"
                } else {
                    "$prefix:file:$path"
                }
            } else {
                "$prefix:${context.applicationContext.packageName}|${uri}"
            }
        }

        fun getCachedLocalImage(context: Context, uri: Uri): Bitmap? {
            if (!isLocalUri(uri)) return null
            val key = buildLocalImageCacheKey(context, uri)
            synchronized(localImageCache) {
                if (pinnedLocalImageKey == key) {
                    pinnedLocalImage?.let { return it }
                }
            }
            return synchronized(localImageCache) {
                localImageCache.get(key)
            }
        }

        fun getCachedVideoFirstFrame(context: Context, uri: Uri): Bitmap? {
            return LocalVideoFrameCache.peek(context, uri)
        }

        fun prefetchVideoFirstFrame(
            context: Context,
            uri: Uri,
            onComplete: ((Boolean) -> Unit)? = null
        ): Future<*>? {
            return LocalVideoFrameCache.load(context, uri) { bitmap ->
                onComplete?.invoke(bitmap != null)
            }
        }

        fun clearVideoFirstFrameCache() {
            LocalVideoFrameCache.evictAll()
        }

        fun clearVisualCaches() {
            synchronized(localImageCache) {
                localImageCache.evictAll()
                clearPinnedLocalImageLocked()
            }
            clearVideoFirstFrameCache()
        }

        private fun clearPinnedLocalImageLocked() {
            pinnedLocalImageKey = null
            pinnedLocalImage = null
        }

        fun prefetchItemVisual(
            context: Context,
            item: PreviewItem,
            runtimeProfile: PreviewRuntimeProfile,
            onComplete: ((Boolean) -> Unit)? = null
        ): Future<*>? {
            return when (item.mediaType) {
                MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> {
                    if (isLocalUri(item.uri)) {
                        prefetchLocalImage(context, item.uri, onComplete)
                    } else {
                        mainHandler.post { onComplete?.invoke(false) }
                        null
                    }
                }
                MediaPreviewType.VIDEO -> {
                    if (!isLocalUri(item.uri) || !runtimeProfile.enableLocalVideoFirstFrameExtraction) {
                        mainHandler.post { onComplete?.invoke(false) }
                        null
                    } else {
                        prefetchVideoFirstFrame(context, item.uri, onComplete)
                    }
                }
            }
        }

        private fun prefetchLocalImage(
            context: Context,
            uri: Uri,
            onComplete: ((Boolean) -> Unit)? = null
        ): Future<*>? {
            val key = buildLocalImageCacheKey(context, uri)
            if (getCachedLocalImage(context, uri) != null) {
                mainHandler.post { onComplete?.invoke(true) }
                return null
            }
            val appContext = context.applicationContext
            ensureMemoryCallbacksRegistered(appContext)
            val metrics = context.resources.displayMetrics
            val targetW = metrics?.widthPixels ?: 1080
            val targetH = metrics?.heightPixels ?: 1920
            return executor.submit {
                val bitmap = decodeLocalBitmap(appContext, uri, targetW, targetH)
                if (bitmap != null) {
                    synchronized(localImageCache) {
                        pinnedLocalImageKey = key
                        pinnedLocalImage = bitmap
                        localImageCache.put(key, bitmap)
                    }
                }
                mainHandler.post { onComplete?.invoke(bitmap != null) }
            }
        }

        fun loadRemote(
            url: String,
            target: ImageView,
            progressBar: ProgressBar?,
            onComplete: (() -> Unit)? = null
        ): Future<*>? {
            ensureMemoryCallbacksRegistered(target.context.applicationContext)
            val imageRef = WeakReference(target)
            val progressRef = progressBar?.let { WeakReference(it) }
            val metrics = target.context.resources.displayMetrics
            val targetW = metrics?.widthPixels ?: 1080
            val targetH = metrics?.heightPixels ?: 1920
            return executor.submit {
                try {
                    val bytes = downloadBytesWithLimit(url, MAX_REMOTE_IMAGE_BYTES)
                    val bitmap = bytes?.let { decodeSampledBitmap(it, targetW, targetH) }
                    val imageView = imageRef.get()
                    if (imageView != null) {
                        imageView.post {
                            progressRef?.get()?.visibility = View.GONE
                            if (bitmap != null) {
                                imageView.setImageBitmap(bitmap)
                            } else {
                                imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                            }
                            onComplete?.invoke()
                        }
                    }
                } catch (e: Exception) {
                    val imageView = imageRef.get()
                    imageView?.post {
                        progressRef?.get()?.visibility = View.GONE
                        imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                        onComplete?.invoke()
                    }
                }
            }
        }

        fun loadLocal(
            context: Context,
            uri: Uri,
            target: ImageView,
            progressBar: ProgressBar?,
            onComplete: (() -> Unit)? = null
        ): Future<*>? {
            val appContext = context.applicationContext
            ensureMemoryCallbacksRegistered(appContext)
            val imageRef = WeakReference(target)
            val progressRef = progressBar?.let { WeakReference(it) }
            val cached = getCachedLocalImage(appContext, uri)
            if (cached != null) {
                target.post {
                    progressRef?.get()?.visibility = View.GONE
                    target.setImageBitmap(cached)
                    onComplete?.invoke()
                }
                return null
            }
            return executor.submit {
                val imageView = imageRef.get()
                val metrics = imageView?.context?.resources?.displayMetrics
                val targetW = metrics?.widthPixels ?: 1080
                val targetH = metrics?.heightPixels ?: 1920
                val bitmap = decodeLocalBitmap(appContext, uri, targetW, targetH)
                imageView?.post {
                    progressRef?.get()?.visibility = View.GONE
                    if (bitmap != null) {
                        val key = buildLocalImageCacheKey(appContext, uri)
                        synchronized(localImageCache) {
                            localImageCache.put(key, bitmap)
                        }
                        imageView.setImageBitmap(bitmap)
                    } else {
                        imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                    }
                    onComplete?.invoke()
                }
            }
        }

        fun loadVideoFirstFrame(
            context: Context,
            uri: Uri,
            target: ImageView,
            onComplete: (() -> Unit)? = null
        ): Future<*>? {
            val imageRef = WeakReference(target)
            return LocalVideoFrameCache.load(context, uri) { bitmap ->
                val view = imageRef.get()
                if (view != null && bitmap != null) {
                    view.setImageBitmap(bitmap)
                }
                onComplete?.invoke()
            }
        }

        private fun decodeLocalBitmap(
            context: Context,
            uri: Uri,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap? {
            return try {
                when {
                    uri.scheme.isNullOrEmpty() || uri.scheme.equals("file", true) -> {
                        val path = uri.path ?: return null
                        decodeFileWithSample(path, targetWidth, targetHeight)
                    }
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.P -> {
                        val source = ImageDecoder.createSource(context.contentResolver, uri)
                        ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                            val sample = calculateSample(info.size.width, info.size.height, targetWidth, targetHeight)
                            val width = max(1, info.size.width / sample)
                            val height = max(1, info.size.height / sample)
                            decoder.setTargetSize(width, height)
                            decoder.isMutableRequired = false
                            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                        }
                    }
                    else -> {
                        context.contentResolver.openInputStream(uri)?.use { stream ->
                            val bytes = readBytesWithLimit(stream, MAX_LOCAL_CONTENT_IMAGE_BYTES)
                            bytes?.let { decodeSampledBitmap(it, targetWidth, targetHeight) }
                        }
                    }
                }
            } catch (_: Exception) {
                null
            }
        }

        private fun decodeFileWithSample(path: String, targetWidth: Int, targetHeight: Int): Bitmap? {
            val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(path, opts)
            if (opts.outWidth <= 0 || opts.outHeight <= 0) {
                return BitmapFactory.decodeFile(
                    path,
                    BitmapFactory.Options().apply {
                        inPreferredConfig = Bitmap.Config.RGB_565
                    }
                )
            }

            val sample = calculateSample(opts.outWidth, opts.outHeight, targetWidth, targetHeight)
            val decodeOpts = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.RGB_565
            }

            val decoded = BitmapFactory.decodeFile(path, decodeOpts) ?: return null
            val orientation = try {
                ExifInterface(path).getAttributeInt(
                    ExifInterface.TAG_ORIENTATION,
                    ExifInterface.ORIENTATION_NORMAL
                )
            } catch (_: Exception) {
                ExifInterface.ORIENTATION_UNDEFINED
            }

            val matrix = Matrix()
            when (orientation) {
                ExifInterface.ORIENTATION_ROTATE_90 -> matrix.postRotate(90f)
                ExifInterface.ORIENTATION_ROTATE_180 -> matrix.postRotate(180f)
                ExifInterface.ORIENTATION_ROTATE_270 -> matrix.postRotate(270f)
                ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> matrix.postScale(-1f, 1f)
                ExifInterface.ORIENTATION_FLIP_VERTICAL -> matrix.postScale(1f, -1f)
            }

            return if (!matrix.isIdentity) {
                try {
                    Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, matrix, true)
                        .also { if (it != decoded) decoded.recycle() }
                } catch (_: Exception) {
                    decoded
                }
            } else {
                decoded
            }
        }

        private fun downloadBytesWithLimit(url: String, maxBytes: Int): ByteArray? {
            var connection: HttpURLConnection? = null
            return try {
                connection = (URL(url).openConnection() as HttpURLConnection).apply {
                    connectTimeout = 5_000
                    readTimeout = 10_000
                    instanceFollowRedirects = true
                    doInput = true
                }
                connection.connect()
                if (connection.responseCode !in 200..299) {
                    null
                } else {
                    connection.inputStream.use { readBytesWithLimit(it, maxBytes) }
                }
            } finally {
                connection?.disconnect()
            }
        }

        private fun readBytesWithLimit(input: java.io.InputStream, limitBytes: Int): ByteArray? {
            val output = ByteArrayOutputStream(limitBytes.coerceAtMost(16 * 1024))
            val buffer = ByteArray(8 * 1024)
            var total = 0
            while (true) {
                val read = input.read(buffer)
                if (read <= 0) break
                total += read
                if (total > limitBytes) return null
                output.write(buffer, 0, read)
            }
            return output.toByteArray()
        }

        private fun decodeSampledBitmap(
            bytes: ByteArray,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap? {
            if (bytes.isEmpty()) return null
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                return BitmapFactory.decodeByteArray(
                    bytes,
                    0,
                    bytes.size,
                    BitmapFactory.Options().apply {
                        inPreferredConfig = Bitmap.Config.RGB_565
                    }
                )
            }
            val sample = calculateSample(bounds.outWidth, bounds.outHeight, targetWidth, targetHeight)
            val opts = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.RGB_565
            }
            return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
        }

        private fun calculateSample(
            width: Int,
            height: Int,
            targetWidth: Int,
            targetHeight: Int
        ): Int {
            var sample = 1
            var outWidth = width
            var outHeight = height
            while (outWidth / 2 >= targetWidth || outHeight / 2 >= targetHeight) {
                outWidth /= 2
                outHeight /= 2
                sample *= 2
            }
            return sample.coerceAtLeast(1)
        }
    }
}
