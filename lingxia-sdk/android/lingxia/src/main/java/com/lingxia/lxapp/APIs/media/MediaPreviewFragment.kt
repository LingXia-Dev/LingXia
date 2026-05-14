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
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeApi
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
    private var currentIndex: Int = 0
    private var currentPagerPosition: Int = 0
    private var advance: PreviewAdvance = PreviewAdvance.MANUAL
    private var showIndexIndicator: Boolean = false
    private var finished = false
    private val mainHandler = Handler(Looper.getMainLooper())
    private var imageAutoRunnable: Runnable? = null
    private var imageAutoRunnablePagerPosition: Int = RecyclerView.NO_POSITION
    private var previewRoot: View? = null
    private var transitionOverlay: ImageView? = null
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        runtimeProfile = PreviewRuntimeProfile.from(requireContext())
        previewItems = readPreviewItems()
        callbackId = arguments?.getLong(ARG_CALLBACK_ID, 0L) ?: 0L
        advance = PreviewAdvance.fromRaw(arguments?.getString(ARG_ADVANCE))
        currentIndex = clampIndex(arguments?.getInt(ARG_START_INDEX, 0) ?: 0)
        currentPagerPosition = initialPagerPosition(currentIndex)
        showIndexIndicator = arguments?.getBoolean(ARG_SHOW_INDEX_INDICATOR, false) ?: false
        Log.i(
            LOG_TAG,
            "onCreate args: callbackId=$callbackId advance=$advance " +
                "startIndex=$currentIndex pagerPos=$currentPagerPosition " +
                "showIndexIndicator=$showIndexIndicator items=${previewItems.size}"
        )
        previewItems.forEachIndexed { i, item ->
            Log.i(
                LOG_TAG,
                "  item[$i] type=${item.mediaType} rotate=${item.rotate} " +
                    "objectFit=${item.objectFit} durationMs=${item.durationMs} uri=${item.uri}"
            )
        }
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

        val overlay = ImageView(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            setBackgroundColor(Color.BLACK)
            scaleType = ImageView.ScaleType.FIT_XY
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
        if (callbackId <= 0L) return
        val result = JSONObject()
            .put("reason", reason)
            .put("lastIndex", currentIndex)
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    private fun cleanupPreviewResources() {
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
        updateIndicator(currentIndex)
        updateCloseButtonVisibility()
        previewAdapter?.onPageSelected(position)
        applySharedPlayerForCurrentItem()
        scheduleCurrentItemBehaviorWhenVisualReady("page_selected")
        prefetchUpcomingVisual(position)
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
        // Hide the player view while the user (or programmatic settle) is
        // mid-swipe so the underlying ViewPager pages — which carry the
        // first-frame placeholders — remain visible during the gesture.
        // Once the page settles we re-evaluate visibility based on the
        // newly-current item.
        when (state) {
            ViewPager2.SCROLL_STATE_DRAGGING,
            ViewPager2.SCROLL_STATE_SETTLING -> {
                sharedPlayerHost?.visibility = View.INVISIBLE
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
                if (event == "firstframerendered" || event == "playing" || event == "timeupdate") {
                    mainHandler.post { onSharedPlayerFirstFrame() }
                }
            },
            typedEventSink = { event ->
                when (event) {
                    is LxMediaEvent.Ended -> mainHandler.post { onSharedPlayerTerminal("ended") }
                    is LxMediaEvent.Error -> mainHandler.post { onSharedPlayerTerminal("error") }
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
                host.visibility = View.INVISIBLE
                player.playlistGoToIndex(playlistIdx)
                player.play()
            }
            MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> {
                sharedPlayer?.pause()
                // INVISIBLE (not GONE) so the player's TextureView keeps its
                // SurfaceTexture alive — switching back to a video page later
                // can resume rendering immediately without a surface rebind.
                host.visibility = View.INVISIBLE
            }
        }
    }

    private fun onSharedPlayerFirstFrame() {
        val item = previewItems.getOrNull(currentIndex)
        // Reveal the host only if the current preview item is a video and
        // we're settled on it — events for a stale source must not flash
        // a wrong frame.
        if (item?.mediaType == MediaPreviewType.VIDEO &&
            sharedPlayerScrollState == ViewPager2.SCROLL_STATE_IDLE
        ) {
            sharedPlayerHost?.visibility = View.VISIBLE
        }
        hideTransitionOverlay()
        previewAdapter?.notifyVideoRenderedAt(currentPagerPosition)
    }

    private fun onSharedPlayerTerminal(terminal: String) {
        onVideoTerminal(currentPagerPosition, terminal)
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
                advanceFromCurrentItem()
            }
            imageAutoRunnable = runnable
            imageAutoRunnablePagerPosition = scheduledPagerPosition
            mainHandler.postDelayed(runnable, timeoutMs)
        }
    }

    private fun onVideoTerminal(position: Int, terminal: String) {
        if (finished || position != currentPagerPosition) return
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
                showTerminalAdvanceOverlay()
                clearAutoRunnables()
                advanceFromCurrentItem()
            }
        }
    }

    private fun advanceFromCurrentItem() {
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
        pendingSwitchPrefetch = PreviewPagerAdapter.prefetchItemVisual(
            context = context,
            item = target,
            runtimeProfile = runtimeProfile
        ) { success ->
            if (finished || generation != pendingSwitchPrefetchGeneration) return@prefetchItemVisual
            pendingSwitchPrefetch = null
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
        initialContentRevealed = true
        previewRoot?.visibility = View.VISIBLE
    }

    private fun revealPreviewRoot() {
        if (initialContentRevealed) return
        initialContentRevealed = true
        previewRoot?.visibility = View.VISIBLE
    }

    private fun onItemVisualReady(position: Int) {
        if (!initialContentRevealed && position == currentPagerPosition) {
            revealPreviewRoot()
        }
        if (position == currentPagerPosition) {
            val item = previewItems.getOrNull(currentIndex)
            // For video items, the placeholder being ready does not mean the
            // player has decoded its first frame yet. Keep the transition
            // overlay up until the shared player's firstframerendered event
            // fires (handled in onSharedPlayerFirstFrame). For images, the
            // visual is fully on-screen now, so we can hide it immediately.
            if (item?.mediaType != MediaPreviewType.VIDEO) {
                hideTransitionOverlay()
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
        if (!showTransitionOverlayForTargetVisual(target)) {
            showTransitionOverlayFromCurrentVisual()
        }
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
        val bitmap = previewAdapter?.snapshotCurrentVisualBitmap() ?: return false
        clearTransitionOverlayBitmap()
        transitionOverlayBitmap = bitmap
        transitionOverlayOwnsBitmap = true
        overlay.scaleType = ImageView.ScaleType.FIT_XY
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
        overlay.scaleType = previewFrameScaleType(target.objectFit)
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

    private fun switchToPagerPositionWithVisualOverlay(targetPagerPosition: Int, target: PreviewItem) {
        if (!showTransitionOverlayForTargetVisual(target)) {
            showTransitionOverlayFromCurrentVisual()
        }
        viewPager?.setCurrentItem(targetPagerPosition, false)
    }

    private fun advanceToPagerPosition(targetPagerPosition: Int) {
        val targetIndex = realIndexFor(targetPagerPosition)
        val target = previewItems.getOrNull(targetIndex)
        if (target == null) {
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

    companion object {
        private const val ARG_PAYLOADS = "arg_payloads"
        private const val ARG_START_INDEX = "arg_start_index"
        private const val ARG_ADVANCE = "arg_advance"
        private const val ARG_SHOW_INDEX_INDICATOR = "arg_show_index_indicator"
        private const val ARG_CALLBACK_ID = "arg_callback_id"
        private const val TAG = "MediaPreviewOverlay"
        private const val LOG_TAG = "LingXia.MediaPreview"

        fun show(
            activity: AppCompatActivity,
            payloads: Array<PreviewMediaPayload>,
            startIndex: Int,
            advance: String,
            showIndexIndicator: Boolean,
            callbackId: Long
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
                            callbackId = callbackId
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
                callbackId = callbackId
            )
        }

        private fun showNow(
            activity: AppCompatActivity,
            payloads: Array<PreviewMediaPayload>,
            startIndex: Int,
            advance: String,
            showIndexIndicator: Boolean,
            callbackId: Long
        ) {
            val fm = activity.supportFragmentManager
            val fragment = MediaPreviewFragment().apply {
                arguments = Bundle().apply {
                    putSerializable(ARG_PAYLOADS, ArrayList(payloads.toList()))
                    putInt(ARG_START_INDEX, startIndex)
                    putString(ARG_ADVANCE, advance)
                    putBoolean(ARG_SHOW_INDEX_INDICATOR, showIndexIndicator)
                    putLong(ARG_CALLBACK_ID, callbackId)
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

private fun previewFrameScaleType(objectFit: LxMediaObjectFit?): ImageView.ScaleType {
    return when (objectFit ?: LxMediaObjectFit.CONTAIN) {
        LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
        LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
        LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
    }
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

            val frameView = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                scaleType = frameScaleType(item.objectFit)
            }
            videoFrameView = frameView
            container.addView(
                frameView,
                FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            )

            if (mediaUri == Uri.EMPTY) {
                frameView.setImageResource(android.R.drawable.ic_dialog_alert)
                frameView.scaleType = ImageView.ScaleType.CENTER
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

        private fun frameScaleType(objectFit: LxMediaObjectFit?): ImageView.ScaleType {
            return when (objectFit ?: LxMediaObjectFit.CONTAIN) {
                LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
                LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
                LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
            }
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
