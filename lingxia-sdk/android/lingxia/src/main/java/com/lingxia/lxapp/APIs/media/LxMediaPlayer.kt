package com.lingxia.lxapp.APIs.media

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.util.Log
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.SurfaceView
import android.view.TextureView
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.ProgressBar
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import com.lingxia.lxapp.APIs.media.player.BackendKind
import com.lingxia.lxapp.APIs.media.player.FeedEngine
import com.lingxia.lxapp.APIs.media.player.JsEventMapper
import com.lingxia.lxapp.APIs.media.player.PlayerCore
import com.lingxia.lxapp.APIs.media.player.PlayerEvent as CorePlayerEvent
import com.lingxia.lxapp.APIs.media.player.PlayerSource as CorePlayerSource
import com.lingxia.lxapp.APIs.media.player.StopReason
import com.lingxia.lxapp.APIs.media.player.SurfaceHost
import com.lingxia.lxapp.APIs.media.player.UrlEngine
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.chrome.NavigationBar
import com.lingxia.lxapp.R
import com.lingxia.lxapp.NativeComponents.ComponentRouter
import com.lingxia.lxapp.chrome.TabBar
import java.io.File
import java.io.ByteArrayOutputStream
import java.util.concurrent.Executors
import java.util.concurrent.Future
import kotlin.math.max
import kotlin.math.min

private const val TAG = "LxMediaPlayer"

// Media source types
sealed class LxMediaSource {
    data class Url(val url: String) : LxMediaSource()
    data class FilePath(val path: String) : LxMediaSource()

    fun toUri(): Uri? = when (this) {
        is Url -> Uri.parse(url)
        is FilePath -> Uri.fromFile(File(path))
    }
}

// Quality option for video
internal data class LxMediaQuality(
    val label: String,
    val url: String? = null
)

// Object fit modes
internal enum class LxMediaObjectFit {
    COVER, CONTAIN, FILL, FIT;

    fun toResizeMode(): Int = when (this) {
        COVER -> AspectRatioFrameLayout.RESIZE_MODE_ZOOM
        CONTAIN -> AspectRatioFrameLayout.RESIZE_MODE_FIT
        FILL -> AspectRatioFrameLayout.RESIZE_MODE_FILL
        FIT -> AspectRatioFrameLayout.RESIZE_MODE_FIT
    }

    companion object {
        fun fromString(value: String?): LxMediaObjectFit = when (value?.lowercase()) {
            "cover" -> COVER
            "contain" -> CONTAIN
            "fill" -> FILL
            "fit" -> FIT
            else -> COVER
        }
    }
}

// Player configuration
internal data class LxMediaPlayerConfig(
    var source: LxMediaSource? = null,
    var src: String? = null,
    var playlist: List<String>? = null,
    var poster: String? = null,
    var autoplay: Boolean? = null,
    var loop: Boolean? = null,
    var muted: Boolean? = null,
    var volume: Double? = null,
    var controls: Boolean? = null,
    var progressBar: Boolean? = null,
    var live: Boolean? = null,
    var cornerRadius: Double? = null,
    var qualities: List<LxMediaQuality>? = null,
    var speeds: List<Double>? = null,
    var showControlsOnInit: Boolean? = null,
    var objectFit: LxMediaObjectFit? = null,
    // Rotate video content inside the component (0/90/180/270). This does not rotate the overlay controls.
    var rotateDegrees: Int? = null,
    // Internal bridge protocol: fields to clear/reset to defaults.
    var clearProps: Set<String> = emptySet(),
)

// Commands that can be sent to the player
sealed class LxMediaCommand {
    object Play : LxMediaCommand()
    object Pause : LxMediaCommand()
    object Stop : LxMediaCommand()
    object NotifyEnded : LxMediaCommand()
    data class Seek(val time: Double) : LxMediaCommand()
    data class SetDuration(val duration: Double) : LxMediaCommand()
    data class SetVolume(val volume: Double) : LxMediaCommand()
    data class SetMuted(val muted: Boolean) : LxMediaCommand()
    data class SetPlaybackRate(val rate: Double) : LxMediaCommand()
    object EnterFullscreen : LxMediaCommand()
    object ExitFullscreen : LxMediaCommand()
    object PlaylistNext : LxMediaCommand()
    object PlaylistPrevious : LxMediaCommand()
    data class PlaylistGoToIndex(val index: Int) : LxMediaCommand()
}

// Events emitted by the player
sealed class LxMediaEvent {
    object Play : LxMediaEvent()
    object Pause : LxMediaEvent()
    object Stop : LxMediaEvent()
    object Ended : LxMediaEvent()
    object Waiting : LxMediaEvent()
    data class Seeked(val time: Double) : LxMediaEvent()
    data class TimeUpdate(val currentTime: Double, val duration: Double) : LxMediaEvent()
    data class RateChange(val rate: Double) : LxMediaEvent()
    data class VolumeChange(val volume: Double) : LxMediaEvent()
    data class FullscreenChange(val fullScreen: Boolean, val direction: String) : LxMediaEvent()
    data class LoadedMetadata(val width: Double, val height: Double, val duration: Double) : LxMediaEvent()
    data class QualityChange(val quality: String, val url: String?) : LxMediaEvent()
    data class PlaylistChange(val index: Int, val url: String, val reason: String) : LxMediaEvent()
    data class PlaylistEnd(val index: Int, val url: String) : LxMediaEvent()
    data class Error(val code: String, val message: String) : LxMediaEvent()
    data class Raw(val name: String, val data: Map<String, Any>) : LxMediaEvent()

    val rawName: String
        get() = when (this) {
            is Play -> "play"
            is Pause -> "pause"
            is Stop -> "stop"
            is Ended -> "ended"
            is Waiting -> "waiting"
            is Seeked -> "seeked"
            is TimeUpdate -> "timeupdate"
            is RateChange -> "ratechange"
            is VolumeChange -> "volumechange"
            is FullscreenChange -> "fullscreenchange"
            is LoadedMetadata -> "loadedmetadata"
            is QualityChange -> "qualitychange"
            is PlaylistChange -> "playlistchange"
            is PlaylistEnd -> "playlistend"
            is Error -> "error"
            is Raw -> name
        }

    val rawData: Map<String, Any>
        get() = when (this) {
            is Play, is Pause, is Stop, is Ended, is Waiting -> emptyMap()
            is Seeked -> mapOf("time" to time)
            is TimeUpdate -> mapOf("currentTime" to currentTime, "duration" to duration)
            is RateChange -> mapOf("rate" to rate)
            is VolumeChange -> mapOf("volume" to volume)
            is FullscreenChange -> mapOf("fullScreen" to fullScreen, "direction" to direction)
            is LoadedMetadata -> mapOf("width" to width, "height" to height, "duration" to duration)
            is QualityChange -> mapOf("quality" to quality, "url" to (url ?: ""))
            is PlaylistChange -> mapOf("index" to index, "url" to url, "reason" to reason)
            is PlaylistEnd -> mapOf("index" to index, "url" to url)
            is Error -> mapOf("code" to code, "message" to message)
            is Raw -> data
        }

    val rawPayload: Map<String, Any>
        get() = mapOf("event" to rawName, "detail" to rawData)
}

/**
 * LxMediaPlayer - A native video player with built-in controls.
 * Designed to be reused by native components and MediaPreview.
 */
internal class LxMediaPlayer(
    private val context: Context,
    private val eventSink: (Map<String, Any>) -> Unit,
    private val typedEventSink: ((LxMediaEvent) -> Unit)? = null,
    private val componentId: String? = null
) {
    private companion object {
        private const val MAX_POSTER_DOWNLOAD_BYTES = 8 * 1024 * 1024
        private val posterExecutor = Executors.newFixedThreadPool(2) { runnable ->
            Thread(runnable, "LingXiaPosterLoader").apply { isDaemon = true }
        }
    }

    val view: FrameLayout = FrameLayout(context).apply {
        setBackgroundColor(Color.BLACK)
        clipToOutline = true
    }

    private val ownerKey: String =
        if (componentId != null) "p-unknown/$componentId" else "preview/${System.identityHashCode(this).toString(16)}"

    private var surfaceHost: SurfaceHost? = null
    private var playerCore: PlayerCore? = null
    private var activeUrlEngine: UrlEngine? = null
    private var activeFeedEngine: FeedEngine? = null

    private var player: ExoPlayer? = null
    private var playerView: PlayerView? = null
    private var streamTextureView: TextureView? = null
    private var posterImageView: ImageView? = null
    private var loadingIndicator: ProgressBar? = null
    private var controlsOverlay: LxMediaControlsOverlay? = null
    private var defaultBackendInitialized = false
    // Once current source has rendered a frame, do not show poster again for that same source.
    private var hasEverRenderedFrame = false
    private var loadingIndicatorEnabled = true

    private val mainHandler = Handler(Looper.getMainLooper())

    // Config state
    private var controlsEnabled = true
    private var isLiveContent = false
    private var loopEnabled = false
    private var currentVolume = 1.0
    private var isMuted = false
    private var currentPlaybackRate = 1.0f
    private var objectFit = LxMediaObjectFit.COVER
    private var cornerRadius = 0.0
    private var displayRotationDegrees = 0
    private var explicitDisplayRotationDegrees: Int? = null

    // Quality and Speed
    private var availableQualities: List<LxMediaQuality> = emptyList()
    private var currentQuality: String? = null
    private var availablePlaybackRates: List<Double> = emptyList()

    // State
    private var currentSource: Uri? = null
    private var isFullscreen = false
    private var isPausedByUser = false  // Track if user explicitly paused (vs buffering)
    private var isBufferingForUi = false
    private var firstFrameDisplayed = false
    private var hasEnded = false
    private var posterUrl: String? = null
    private var posterLoadFuture: Future<*>? = null
    private var posterLoadToken: Long = 0L
    private var videoWidth = 0.0
    private var videoHeight = 0.0
    private var videoRotationDegrees = 0
    private var closeRequestListener: (() -> Unit)? = null

    private var uiSeeking = false
    private var lastUiTimeUpdateMs: Long? = null

    // Fullscreen state
    private var fullscreenDialog: android.app.Dialog? = null
    private var fullscreenContainer: FrameLayout? = null
    private var fullscreenContent: FrameLayout? = null
    private var fullscreenLayoutListener: View.OnLayoutChangeListener? = null
    private var inlineFullscreenParent: ViewGroup? = null
    private var inlineFullscreenLayoutListener: View.OnLayoutChangeListener? = null
    private var inlineFullscreenWindowUiSnapshot: ImmersiveWindowUi.Snapshot? = null
    private var inlineFullscreenConsumesInsets: Boolean = false
    private var fallbackHiddenViews: MutableList<Pair<View, Int>>? = null
    private var fallbackOverlayLayoutParams: ViewGroup.LayoutParams? = null
    private var fallbackOverlayTranslationX: Float = 0f
    private var fallbackOverlayTranslationY: Float = 0f
    private var fallbackOverlayView: View? = null
    private var fallbackWebViewContainer: ViewGroup? = null
    private var fallbackWebViewContainerLayoutParams: FrameLayout.LayoutParams? = null
    private var fallbackCurrentWebViewContainer: ViewGroup? = null
    private var fallbackCurrentWebViewTranslationY: Float = 0f
    private var originalParent: ViewGroup? = null
    private var originalIndex: Int = 0
    private var originalLayoutParams: ViewGroup.LayoutParams? = null
    private var originalClipToOutline: Boolean? = null
    private var originalOutlineProvider: android.view.ViewOutlineProvider? = null

    // Track last frame for restoring after fullscreen
    private var lastFrameX = 0f
    private var lastFrameY = 0f
    private var lastFrameWidth = 0f
    private var lastFrameHeight = 0f
    private var rectSyncScheduledAtMs: Long = 0
    private var rectSyncRunnable: Runnable? = null

    // Playlist mode: controller drives advance / overlay / prefetch.
    // Source of truth for playlist state lives on the controller; LxMediaPlayer
    // forwards config / events / commands and exposes the surface needed for it
    // to drive playback (parseUri / loadSource / play / emitEvent).
    private val transitionOverlay = VideoTransitionOverlay(view)
    private val playlistHostImpl = object : LxMediaPlaylistController.PlaylistHost {
        override val context: Context get() = this@LxMediaPlayer.context
        override val isLoopEnabled: Boolean get() = loopEnabled
        override fun parseUri(src: String): Uri? = this@LxMediaPlayer.parseUri(src)
        override fun loadSourceForPlaylist(uri: Uri) {
            // Match the pre-refactor inline behavior: clear `hasEnded` BEFORE
            // loading the next source. PlayerCore suppresses `Waiting` events
            // when (hasEnded && !playIntent), so leaving it true through the
            // brief loadSource→play window risks dropping a buffering event.
            hasEnded = false
            loadSource(uri)
        }
        override fun playFromPlaylist() { play() }
        override fun emit(event: LxMediaEvent) { emitEvent(event) }
        override fun applyItemDisplay(item: LxMediaPlaylistItem) {
            // Per-item overrides only — null fields preserve element-level
            // values that were applied via update(config). lx-video's
            // string-array playlist therefore produces a no-op here, while
            // preview supplies real per-item objectFit / rotateDegrees.
            item.objectFit?.let { setObjectFit(it) }
            item.rotateDegrees?.let { setDisplayRotationDegrees(it) }
        }
    }
    private val playlistController = LxMediaPlaylistController(playlistHostImpl)

    init {
        setupUI()
        setupCore()

        // Keep inline rotation in sync with layout changes (e.g. frame updates from WebView overlay).
        view.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            applyInlineDisplayRotationTransform()
        }

        // Ensure video output is ready when view is attached
        view.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(v: View) {
                activeUrlEngine?.let { engine ->
                    playerView?.player = engine.exoPlayer
                }
            }
            override fun onViewDetachedFromWindow(v: View) = Unit
        })
    }

    private fun setupCore() {
        val pv = playerView ?: return
        val tv = streamTextureView ?: return
        surfaceHost = SurfaceHost(
            ownerKey = ownerKey,
            urlPlayerView = pv,
            feedTextureView = tv,
        )
        playerCore = PlayerCore(
            createUrlEngine = {
                UrlEngine(
                    context = context.applicationContext,
                    playerView = pv
                ).also { engine ->
                    activeUrlEngine = engine
                    player = engine.exoPlayer
                    engine.setLoopEnabled(loopEnabled && !playlistController.isActive)
                }
            },
            createFeedEngine = {
                val id = componentId ?: error("FeedEngine requires componentId")
                FeedEngine(id).also { engine ->
                    activeFeedEngine = engine
                }
            },
            emit = ::handleCoreEvent,
        )
    }

    private fun setupUI() {
        // PlayerView
        // Inflate PlayerView configured to use TextureView (see res/layout/lx_media_player_view.xml)
        playerView = LayoutInflater.from(context)
            .inflate(R.layout.lx_media_player_view, view, false) as PlayerView
        playerView?.apply {
            useController = false // We use custom controls
            resizeMode = objectFit.toResizeMode()
        }
        view.addView(playerView)

        // Dedicated TextureView for stream decoding.
        // Using PlayerView's internal surface view is fragile (shutter/background overlays can cover it
        // during rebind/mount transitions). A dedicated TextureView avoids "audio only / black video".
        streamTextureView = TextureView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            visibility = View.GONE
        }
        view.addView(streamTextureView)

        // Poster ImageView
        posterImageView = ImageView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            scaleType = posterScaleTypeForObjectFit(objectFit)
            setBackgroundColor(Color.BLACK)
            visibility = View.GONE
        }
        view.addView(posterImageView)

        loadingIndicator = ProgressBar(context).apply {
            val size = (28 * context.resources.displayMetrics.density).toInt()
            layoutParams = FrameLayout.LayoutParams(size, size, Gravity.CENTER)
            indeterminateTintList = android.content.res.ColorStateList.valueOf(Color.WHITE)
            visibility = View.GONE
        }
        view.addView(loadingIndicator)

        // Transition overlay sits above the player surface + loading spinner,
        // below the controls overlay. Hidden by default; shown only during
        // item transitions to bridge the visual gap (see LxMediaPlaylistController).
        transitionOverlay.attach()
        playlistController.bindOverlay(transitionOverlay)

        controlsOverlay = LxMediaControlsOverlay(context, this).also {
            view.addView(it.view)
        }
    }

    fun update(config: LxMediaPlayerConfig) {
        // Playlist takes priority over single source. The controller is the
        // sole owner of playlist state; if the URL list is unchanged it's a
        // no-op (no restart-mid-stream). Single-source path also resets the
        // controller via deactivate().
        val incomingPlaylist = config.playlist?.takeIf { it.isNotEmpty() }
        if (incomingPlaylist != null) {
            // JS-facing playlist is `string[]` — map to items with null
            // per-item overrides so element-level objectFit / rotateDegrees
            // (set further down in this method) keep applying uniformly.
            // Route through applyPlaylist so a prior native autoAdvance=false
            // call doesn't leak into JS's default semantics.
            applyPlaylist(incomingPlaylist.map { LxMediaPlaylistItem(it) })
            activeUrlEngine?.setLoopEnabled(false)
        } else if (config.source != null || config.src != null) {
            playlistController.deactivate()
            val uri = when (val source = config.source) {
                is LxMediaSource.Url -> parseUri(source.url)
                is LxMediaSource.FilePath -> parseUri(source.path)
                null -> config.src?.let(::parseUri)
            }
            if (uri != null) {
                loadSource(uri)
            }
        } else if (!defaultBackendInitialized && componentId != null) {
            ensureFeedBackendIfNeeded()
        }

        config.poster?.let {
            posterUrl = it
            loadPoster(it, show = shouldShowPoster())
        }
        updatePosterVisibility()
        config.loop?.let {
            loopEnabled = it
            activeUrlEngine?.setLoopEnabled(it && !playlistController.isActive)
        }
        config.muted?.let { setMuted(it) }
        config.volume?.let { setVolume(it) }
        config.controls?.let { controlsEnabled = it; controlsOverlay?.setVisible(it) }
        config.live?.let { isLiveContent = it }
        if (isLiveContent || config.progressBar != null) {
            controlsOverlay?.setShowProgressBar(!isLiveContent && (config.progressBar ?: true))
        }
        config.cornerRadius?.let { setCornerRadius(it) }
        config.qualities?.let { qualities ->
            availableQualities = qualities
            val existing = currentQuality
            currentQuality = when {
                existing != null && qualities.any { it.label == existing } -> existing
                else -> qualities.firstOrNull()?.label
            }
        }
        config.speeds?.let { rates ->
            availablePlaybackRates = rates
            val current = currentPlaybackRate.toDouble()
            currentPlaybackRate = when {
                rates.any { it == current } -> current.toFloat()
                else -> rates.firstOrNull()?.toFloat() ?: 1.0f
            }
            playerCore?.setRate(currentPlaybackRate)
        }
        config.autoplay?.let { if (it) play() }
        if ("objectFit" in config.clearProps) {
            setObjectFit(LxMediaObjectFit.COVER)
        } else {
            config.objectFit?.let { setObjectFit(it) }
        }
        if ("rotate" in config.clearProps) {
            setDisplayRotationDegrees(null)
        } else if (config.rotateDegrees != null) {
            setDisplayRotationDegrees(config.rotateDegrees)
        }
        controlsOverlay?.updateSettingsButton()
        applyInlineDisplayRotationTransform()
    }

    private fun shouldShowPoster(): Boolean {
        val hasPoster = !posterUrl.isNullOrBlank()
        if (!hasPoster) return false

        if (shouldShowEndedPoster()) return true

        // PROFESSIONAL PLAYER STANDARD: Poster ONLY shows on cold start.
        // Once ANY frame has been rendered, poster should NEVER show again.
        if (hasEverRenderedFrame) return false

        // For native component feed mode (no URL source), show poster only before the first frame.
        if (componentId != null && currentSource == null) return true

        return !firstFrameDisplayed
    }

    private fun shouldShowEndedPoster(): Boolean {
        return hasEnded && isFullscreen && !posterUrl.isNullOrBlank()
    }

    private fun applySurfaceVisibility(endedFullscreen: Boolean) {
        if (endedFullscreen) {
            playerView?.visibility = View.INVISIBLE
            streamTextureView?.visibility = View.INVISIBLE
            return
        }

        when (playerCore?.getBackend()) {
            BackendKind.FEED -> {
                streamTextureView?.visibility = View.VISIBLE
                playerView?.visibility = View.GONE
            }
            BackendKind.URL -> {
                playerView?.visibility = View.VISIBLE
                streamTextureView?.visibility = View.GONE
            }
            null -> Unit
        }
    }

    private fun updatePosterVisibility() {
        val poster = posterImageView ?: return
        val endedFullscreen = shouldShowEndedPoster()
        val shouldShow = shouldShowPoster()
        poster.visibility = if (shouldShow) View.VISIBLE else View.GONE
        if (poster.visibility == View.VISIBLE) {
            poster.bringToFront()
            controlsOverlay?.view?.bringToFront()
            loadingIndicator?.bringToFront()
        }

        applySurfaceVisibility(endedFullscreen)
    }

    private fun showLoadingIndicator() {
        if (!loadingIndicatorEnabled) {
            loadingIndicator?.visibility = View.GONE
            return
        }
        loadingIndicator?.visibility = View.VISIBLE
        loadingIndicator?.bringToFront()
    }

    private fun hideLoadingIndicator() {
        loadingIndicator?.visibility = View.GONE
    }

    fun handle(command: LxMediaCommand) {
        when (command) {
            is LxMediaCommand.Play -> play()
            is LxMediaCommand.Pause -> pause()
            is LxMediaCommand.Stop -> stop()
            is LxMediaCommand.NotifyEnded -> {
                // Stream providers can signal authoritative end-of-stream (VOD segment).
                // Route it through the FEED engine so Core emits `ended` and stops polling/loading.
                ensureFeedBackendIfNeeded()
                activeFeedEngine?.handleStreamDecoderEvent("ended", emptyMap())
            }
            is LxMediaCommand.Seek -> seek(command.time)
            is LxMediaCommand.SetDuration -> {
                val duration = command.duration
                if (duration.isFinite() && duration > 0) {
                    controlsOverlay?.updateProgress(0.0, duration)
                    playerCore?.setDurationMs((duration * 1000.0).toLong())
                } else {
                    controlsOverlay?.updateProgress(0.0, 0.0)
                    playerCore?.setDurationMs(null)
                }
            }
            is LxMediaCommand.SetVolume -> setVolume(command.volume)
            is LxMediaCommand.SetMuted -> setMuted(command.muted)
            is LxMediaCommand.SetPlaybackRate -> setPlaybackRate(command.rate)
            is LxMediaCommand.EnterFullscreen -> enterFullscreen()
            is LxMediaCommand.ExitFullscreen -> exitFullscreen()
            is LxMediaCommand.PlaylistNext -> playlistController.next()
            is LxMediaCommand.PlaylistPrevious -> playlistController.previous()
            is LxMediaCommand.PlaylistGoToIndex -> playlistController.goToIndex(command.index)
        }
    }

    /**
     * Direct Kotlin entry point for in-process callers (e.g. MediaPreview)
     * that need to feed a playlist with per-item display overrides — a richer
     * shape than the JS-facing `LxMediaPlayerConfig.playlist: string[]`.
     *
     * Equivalent to [update] with `config.playlist=...` for the cases JS
     * needs, but skips the rest of the config dance and accepts the typed
     * item form. No-op if the item list is unchanged.
     *
     * @param autoAdvance When `true` (default) the controller advances to
     *   the next item on Ended / Error — the lx-video element behavior. Set
     *   to `false` for scenarios that drive navigation externally (e.g.
     *   MediaPreview, where the ViewPager is the source of truth) so the
     *   controller doesn't queue up the next video's audio behind a UI
     *   page change the consumer hasn't made yet.
     * @param startingIndex Where playback should begin on the *initial*
     *   apply. Defaults to 0; callers opening on a non-zero item should
     *   pass it here so the controller doesn't first load item 0 only
     *   to immediately seek away. Ignored when [items] is structurally
     *   unchanged from the previous apply — use [playlistGoToIndex] to
     *   move within an already-applied playlist.
     */
    fun applyPlaylist(
        items: List<LxMediaPlaylistItem>,
        autoAdvance: Boolean = true,
        startingIndex: Int = 0,
    ) {
        playlistController.autoAdvance = autoAdvance
        if (items.isEmpty()) {
            playlistController.deactivate()
            return
        }
        playlistController.apply(items, startingIndex)
    }

    /** Switch playlist position; no-op when not in playlist mode or already there. */
    fun playlistGoToIndex(index: Int) {
        playlistController.goToIndex(index)
    }

    fun acquireStreamTextureView(): TextureView? {
        ensureFeedBackendIfNeeded()
        return surfaceHost?.getFeedTextureView() ?: streamTextureView
    }

    fun snapshotCurrentPlaybackFrame(): Bitmap? {
        val textureView = findFirstTextureView(playerView) ?: return null
        if (!textureView.isAvailable || textureView.width <= 0 || textureView.height <= 0) {
            return null
        }
        return try {
            textureView.getBitmap(textureView.width, textureView.height)
        } catch (_: OutOfMemoryError) {
            null
        } catch (_: Exception) {
            null
        }
    }

    fun releaseStreamTextureView() {
        // No-op: keep the last rendered frame visible; backend switching controls visibility.
    }

    fun attach(to: ViewGroup) {
        if (view.parent != to) {
            (view.parent as? ViewGroup)?.removeView(view)
            to.addView(view)
        }
    }

    fun detach() {
        rectSyncRunnable?.let { mainHandler.removeCallbacks(it) }
        rectSyncRunnable = null
        controlsOverlay?.cancelPendingDeferredActions()
        cancelPosterLoad()
        posterImageView?.setImageDrawable(null)
        playlistController.release()
        transitionOverlay.detach()
        playerCore?.release()
        playerCore = null
        activeUrlEngine = null
        activeFeedEngine = null
        player = null
        (view.parent as? ViewGroup)?.removeView(view)
    }

    fun setFrame(x: Float, y: Float, width: Float, height: Float) {
        if (isFullscreen) {
            return
        }

        val newWidth = width.toInt()
        val newHeight = height.toInt()

        val existingLp = view.layoutParams as? FrameLayout.LayoutParams
        if (existingLp == null || existingLp.width != newWidth || existingLp.height != newHeight) {
            view.layoutParams = FrameLayout.LayoutParams(newWidth, newHeight)
        }

        // Update immediately for smooth scrolling - use translation for position
        view.translationX = x
        view.translationY = y

        // Save for fullscreen restore (only when not in fullscreen)
        lastFrameX = x
        lastFrameY = y
        lastFrameWidth = width
        lastFrameHeight = height
    }

    private fun scheduleRectSync(doublePass: Boolean) {
        if (isFullscreen) return
        val id = componentId ?: return
        val now = android.os.SystemClock.uptimeMillis()
        // Avoid piling up requests if multiple events fire quickly.
        if (now - rectSyncScheduledAtMs < 200) return
        rectSyncScheduledAtMs = now

        rectSyncRunnable?.let { mainHandler.removeCallbacks(it) }
        rectSyncRunnable = Runnable {
            requestRectSync(id)
            if (doublePass) {
                // Second pass to catch async React layout updates triggered by event handlers.
                mainHandler.postDelayed({ requestRectSync(id) }, 250)
            }
        }
        // Give JS a moment to handle the event and for layout to settle.
        mainHandler.postDelayed(rectSyncRunnable!!, 80)
    }

    private fun requestRectSync(id: String) {
        ComponentRouter.requestRectSync(id)
    }

    fun play() {
        isPausedByUser = false  // User wants to play
        isBufferingForUi = true
        if (hasEnded) {
            hasEnded = false
            updatePosterVisibility()
        }
        if (!defaultBackendInitialized && componentId != null && currentSource == null) {
            ensureFeedBackendIfNeeded()
        }
        playerCore?.setRate(currentPlaybackRate)
        playerCore?.play()
    }

    fun pause() {
        isPausedByUser = true  // User explicitly paused
        isBufferingForUi = false
        controlsOverlay?.cancelPendingDeferredActions()
        playerCore?.pause()
    }

    fun stop() {
        isPausedByUser = true
        isBufferingForUi = false
        controlsOverlay?.cancelPendingDeferredActions()
        if (hasEnded) {
            hasEnded = false
            updatePosterVisibility()
        }
        playerCore?.stop(StopReason.USER)
    }

    fun seek(time: Double) {
        if (isLiveContent) {
            Log.i(TAG, "seek ignored for live content")
            return
        }
        val positionMs = (time * 1000).toLong()
        if (hasEnded) {
            hasEnded = false
            updatePosterVisibility()
        }
        playerCore?.seek(positionMs)
    }

    fun setVolume(volume: Double) {
        currentVolume = volume.coerceIn(0.0, 1.0)
        playerCore?.setVolume(currentVolume.toFloat())
    }

    fun setMuted(muted: Boolean) {
        isMuted = muted
        playerCore?.setMuted(muted)
    }

    fun setPlaybackRate(rate: Double) {
        currentPlaybackRate = rate.toFloat()
        playerCore?.setRate(currentPlaybackRate)
    }

    fun setShowCloseButton(show: Boolean) {
        controlsOverlay?.setShowCloseButton(show)
    }

    fun setShowFullscreenButton(show: Boolean) {
        controlsOverlay?.setShowFullscreenButton(show)
    }

    fun setSuppressAutoShowControls(suppress: Boolean) {
        controlsOverlay?.setSuppressAutoShow(suppress)
    }

    /// Subscribe to controls visibility transitions. Fires only on actual
    /// shown ↔ hidden flips. Used by the preview fragment to mirror its own
    /// close button visibility against the inline tap-to-reveal affordance.
    fun setOnControlsVisibilityChanged(listener: ((Boolean) -> Unit)?) {
        controlsOverlay?.visibilityListener = listener
    }

    fun isControlsVisible(): Boolean = controlsOverlay?.isControlsVisible == true

    fun setShowLoadingIndicator(show: Boolean) {
        loadingIndicatorEnabled = show
        if (!show) {
            hideLoadingIndicator()
        }
    }

    fun setCloseRequestListener(listener: (() -> Unit)?) {
        closeRequestListener = listener
    }

    fun enterFullscreen() {
        if (isFullscreen) return

        if (isStreamDecoderMode()) {
            enterInlineFullscreen()
            return
        }

        // Get Activity context - required for Dialog
        val activityContext = getActivityContext() ?: run {
            LxLog.w(TAG, "enterFullscreen: Cannot get Activity context")
            return
        }

        isFullscreen = true

        val hostActivity = (activityContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            LxLog.w(TAG, "enterFullscreen: host activity not found; using overlay fallback")
            hideOverlayViewsFallback(view.rootView)
        } else {
            hostActivity.enterMediaFullscreen()
        }

        // Save original parent and layout params
        originalParent = view.parent as? ViewGroup
        originalIndex = originalParent?.indexOfChild(view) ?: 0
        originalLayoutParams = view.layoutParams

        // Remove from original parent
        originalParent?.removeView(view)

        // Try to get video dimensions from player if not already set
        if (videoWidth <= 0 || videoHeight <= 0) {
            player?.videoFormat?.let { format ->
                if (format.width > 0 && format.height > 0) {
                    videoWidth = format.width.toDouble()
                    videoHeight = format.height.toDouble()
                }
            }
        }

        // Determine if video is landscape
        val isLandscapeVideo = isLandscapeVideo()
        val direction = if (isLandscapeVideo) "horizontal" else "vertical"

        // Create container to ensure full coverage
        val fullscreenContainer = FrameLayout(activityContext).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            fitsSystemWindows = false
            clipChildren = false
            clipToPadding = false
        }
        val contentWrapper = FrameLayout(activityContext).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            fitsSystemWindows = false
            clipChildren = false
            clipToPadding = false
        }
        ViewCompat.setOnApplyWindowInsetsListener(fullscreenContainer) { v, _ ->
            v.setPadding(0, 0, 0, 0)
            WindowInsetsCompat.CONSUMED
        }
        ViewCompat.setOnApplyWindowInsetsListener(contentWrapper) { v, _ ->
            v.setPadding(0, 0, 0, 0)
            WindowInsetsCompat.CONSUMED
        }
        fullscreenContainer.addView(contentWrapper)
        ViewCompat.requestApplyInsets(fullscreenContainer)

        fullscreenDialog = android.app.Dialog(activityContext, android.R.style.Theme_Black_NoTitleBar_Fullscreen).apply {
            setContentView(fullscreenContainer)
            setCancelable(true)
            setCanceledOnTouchOutside(false)

            // Set fullscreen flags
            window?.apply {
                setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                setBackgroundDrawable(ColorDrawable(Color.BLACK))
                addFlags(android.view.WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)
                statusBarColor = Color.TRANSPARENT
                navigationBarColor = Color.TRANSPARENT
                if (android.os.Build.VERSION.SDK_INT >= 29) {
                    isNavigationBarContrastEnforced = false
                }
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
                    attributes = attributes?.apply {
                        layoutInDisplayCutoutMode = android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                    }
                }
                @Suppress("DEPRECATION")
                addFlags(android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS)
                WindowCompat.setDecorFitsSystemWindows(this, false)
                WindowInsetsControllerCompat(this, decorView).apply {
                    hide(WindowInsetsCompat.Type.systemBars())
                    isAppearanceLightStatusBars = false
                    isAppearanceLightNavigationBars = false
                    systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                }
                @Suppress("DEPRECATION")
                addFlags(
                    android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN or
                    android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON or
                    android.view.WindowManager.LayoutParams.FLAG_SHOW_WHEN_LOCKED or
                    android.view.WindowManager.LayoutParams.FLAG_TURN_SCREEN_ON
                )
                @Suppress("DEPRECATION")
                decorView.systemUiVisibility = (
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                        View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                        View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                        View.SYSTEM_UI_FLAG_FULLSCREEN or
                        View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                        View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    )
            }

            // Handle back press - exit fullscreen instead of dismiss
            setOnKeyListener { _, keyCode, event ->
                if (keyCode == android.view.KeyEvent.KEYCODE_BACK && event.action == android.view.KeyEvent.ACTION_UP) {
                    exitFullscreen()
                    true
                } else {
                    false
                }
            }

            // Handle dialog lifecycle to ensure player surface is maintained
            setOnShowListener {
                activeUrlEngine?.let { engine ->
                    playerView?.player = engine.exoPlayer
                }
                if (isStreamDecoderMode() && componentId != null) {
                    com.lingxia.lxapp.NativeComponents.ComponentRouter.dispatchVideoCommand(
                        componentId,
                        "rebindSurface",
                        "{}"
                    )
                }
            }

            show()
        }

        // Update view layout for fullscreen
        view.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        view.translationX = 0f
        view.translationY = 0f
        contentWrapper.addView(
            view,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
        )
        fullscreenContainer.also { this.fullscreenContainer = it }
        fullscreenContent = contentWrapper
        fullscreenLayoutListener = View.OnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            applyFullscreenTransform()
        }.also { listener ->
            fullscreenContainer.addOnLayoutChangeListener(listener)
        }

        // Apply transform after layout is ready
        fullscreenContainer.post { applyFullscreenTransform() }

        // Remove rounded corners for fullscreen
        originalClipToOutline = view.clipToOutline
        originalOutlineProvider = view.outlineProvider
        view.clipToOutline = false
        view.outlineProvider = null

        controlsOverlay?.onFullscreenChanged(true)
        updatePosterVisibility()
        emitEvent(LxMediaEvent.FullscreenChange(true, direction))
    }

    fun exitFullscreen() {
        if (!isFullscreen) return
        if (isStreamDecoderMode() && fullscreenDialog == null) {
            exitInlineFullscreen()
            return
        }
        isFullscreen = false
        val hostContext = getActivityContext()
        val hostActivity = (hostContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            restoreOverlayViewsFallback()
        } else {
            hostActivity.exitMediaFullscreen()
        }

        // Dismiss dialog and restore view
        fullscreenDialog?.let { dialog ->
            // Remove view from dialog before dismissing
            (view.parent as? ViewGroup)?.removeView(view)
            try {
                dialog.dismiss()
            } catch (e: Exception) {
                LxLog.w(TAG, "exitFullscreen: Error dismissing dialog", e)
            }
        }
        fullscreenDialog = null
        fullscreenContainer?.let { container ->
            fullscreenLayoutListener?.let { container.removeOnLayoutChangeListener(it) }
        }
        fullscreenLayoutListener = null
        fullscreenContainer = null
        fullscreenContent = null

        // Reset rotation and transform for all child views (important!)
        resetChildViewTransforms()

        // Restore to original parent
        originalParent?.let { parent ->
            // Restore size
            val width = lastFrameWidth.toInt().takeIf { it > 0 } ?: ViewGroup.LayoutParams.MATCH_PARENT
            val height = lastFrameHeight.toInt().takeIf { it > 0 } ?: ViewGroup.LayoutParams.MATCH_PARENT

            view.layoutParams = originalLayoutParams ?: FrameLayout.LayoutParams(width, height)

            // Restore position
            view.translationX = lastFrameX
            view.translationY = lastFrameY

            parent.addView(view, originalIndex.coerceIn(0, parent.childCount))
        }

        // Restore rounding state
        originalClipToOutline?.let { view.clipToOutline = it }
        view.outlineProvider = originalOutlineProvider
        originalClipToOutline = null
        originalOutlineProvider = null

        controlsOverlay?.onFullscreenChanged(false)
        updatePosterVisibility()
        emitEvent(LxMediaEvent.FullscreenChange(false, "vertical"))
    }

    private fun enterInlineFullscreen() {
        if (isFullscreen) return
        val activityContext = getActivityContext() ?: run {
            LxLog.w(TAG, "enterFullscreen: Cannot get Activity context")
            return
        }

        isFullscreen = true
        val hostActivity = (activityContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            LxLog.w(TAG, "enterFullscreen: host activity not found; using overlay fallback")
            hideOverlayViewsFallback(view.rootView)
        } else {
            hostActivity.enterMediaFullscreen()
        }

        originalParent = view.parent as? ViewGroup
        originalIndex = originalParent?.indexOfChild(view) ?: 0
        originalLayoutParams = view.layoutParams

        if (videoWidth <= 0 || videoHeight <= 0) {
            player?.videoFormat?.let { format ->
                if (format.width > 0 && format.height > 0) {
                    videoWidth = format.width.toDouble()
                    videoHeight = format.height.toDouble()
                }
            }
        }

        val isLandscapeVideo = isLandscapeVideo()
        val direction = if (isLandscapeVideo) "horizontal" else "vertical"

        val parent = originalParent ?: return
        inlineFullscreenParent = parent
        applyInlineFullscreenUi(activityContext)
        applyOverlayFullscreenFallback(view.rootView)
        applyWebViewContainerFullscreenFallback(view.rootView)

        if (!inlineFullscreenConsumesInsets) {
            inlineFullscreenConsumesInsets = true
            ViewCompat.setOnApplyWindowInsetsListener(view) { v, _ ->
                v.setPadding(0, 0, 0, 0)
                WindowInsetsCompat.CONSUMED
            }
        }
        ViewCompat.requestApplyInsets(view)

        view.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT,
            Gravity.CENTER
        )
        view.translationX = 0f
        view.translationY = 0f
        view.bringToFront()

        inlineFullscreenLayoutListener = View.OnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            applyInlineFullscreenTransform()
        }.also { listener ->
            parent.addOnLayoutChangeListener(listener)
        }

        applyInlineFullscreenTransform()

        originalClipToOutline = view.clipToOutline
        originalOutlineProvider = view.outlineProvider
        view.clipToOutline = false
        view.outlineProvider = null

        controlsOverlay?.onFullscreenChanged(true)
        updatePosterVisibility()
        emitEvent(LxMediaEvent.FullscreenChange(true, direction))
    }

    private fun exitInlineFullscreen() {
        if (!isFullscreen) return
        isFullscreen = false

        val hostContext = getActivityContext()
        val hostActivity = (hostContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            LxLog.w(TAG, "exitFullscreen: host activity not found; using overlay fallback")
            restoreOverlayViewsFallback()
        } else {
            hostActivity.exitMediaFullscreen()
        }

        if (hostContext != null) {
            restoreInlineFullscreenUi(hostContext)
        }
        restoreOverlayFullscreenFallback()
        restoreWebViewContainerFullscreenFallback()
        if (inlineFullscreenConsumesInsets) {
            inlineFullscreenConsumesInsets = false
            ViewCompat.setOnApplyWindowInsetsListener(view, null)
            view.setPadding(0, 0, 0, 0)
        }

        inlineFullscreenParent?.let { parent ->
            inlineFullscreenLayoutListener?.let { parent.removeOnLayoutChangeListener(it) }
        }
        inlineFullscreenLayoutListener = null
        inlineFullscreenParent = null

        resetChildViewTransforms()

        val width = lastFrameWidth.toInt().takeIf { it > 0 } ?: ViewGroup.LayoutParams.MATCH_PARENT
        val height = lastFrameHeight.toInt().takeIf { it > 0 } ?: ViewGroup.LayoutParams.MATCH_PARENT
        view.layoutParams = originalLayoutParams ?: FrameLayout.LayoutParams(width, height)
        view.translationX = lastFrameX
        view.translationY = lastFrameY

        originalClipToOutline?.let { view.clipToOutline = it }
        view.outlineProvider = originalOutlineProvider
        originalClipToOutline = null
        originalOutlineProvider = null

        controlsOverlay?.onFullscreenChanged(false)
        updatePosterVisibility()
        emitEvent(LxMediaEvent.FullscreenChange(false, "vertical"))
    }

    private fun applyInlineFullscreenUi(activity: android.app.Activity) {
        val window = activity.window ?: return
        if (inlineFullscreenWindowUiSnapshot == null) {
            inlineFullscreenWindowUiSnapshot = ImmersiveWindowUi.capture(window)
        }
        ImmersiveWindowUi.apply(window, keepScreenOn = true)
    }

    private fun restoreInlineFullscreenUi(activity: android.app.Activity) {
        val window = activity.window ?: return
        inlineFullscreenWindowUiSnapshot?.let { snapshot ->
            ImmersiveWindowUi.restore(window, snapshot)
        }
        inlineFullscreenWindowUiSnapshot = null
    }

    private fun resetChildViewTransforms() {
        view.rotation = 0f
        view.scaleX = 1f
        view.scaleY = 1f

        fun resetView(v: View?) {
            v ?: return
            v.rotation = 0f
            v.scaleX = 1f
            v.scaleY = 1f
            v.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        resetView(playerView)
        // PlayerView owns its internal TextureView; don't touch its LayoutParams, only clear transforms.
        findFirstTextureView(playerView)?.let { tv ->
            tv.rotation = 0f
            tv.scaleX = 1f
            tv.scaleY = 1f
        }
        findFirstSurfaceView(playerView)?.let { sv ->
            sv.rotation = 0f
            sv.scaleX = 1f
            sv.scaleY = 1f
        }
        resetView(streamTextureView)
        resetView(posterImageView)
        loadingIndicator?.let { loader ->
            loader.rotation = 0f
            loader.scaleX = 1f
            loader.scaleY = 1f
            loader.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER
            )
        }
        controlsOverlay?.view?.let { resetView(it) }
    }

    private fun getRealDisplaySizePx(context: Context): Pair<Float, Float> {
        val windowManager = context.getSystemService(Context.WINDOW_SERVICE) as? WindowManager
            ?: return context.resources.displayMetrics.run {
                widthPixels.toFloat() to heightPixels.toFloat()
            }

        return if (android.os.Build.VERSION.SDK_INT >= 30) {
            val bounds = windowManager.currentWindowMetrics.bounds
            bounds.width().toFloat() to bounds.height().toFloat()
        } else {
            @Suppress("DEPRECATION")
            val display = windowManager.defaultDisplay
            val dm = DisplayMetrics()
            @Suppress("DEPRECATION")
            display.getRealMetrics(dm)
            dm.widthPixels.toFloat() to dm.heightPixels.toFloat()
        }
    }

    private fun applyFullscreenTransform() {
        val container = fullscreenContent ?: return
        val (screenW, screenH) = getRealDisplaySizePx(container.context)
        applyFullscreenTransformFor(screenW, screenH)
    }

    private fun applyInlineFullscreenTransform() {
        val parent = inlineFullscreenParent ?: return
        val (screenW, screenH) = getRealDisplaySizePx(parent.context)
        applyFullscreenTransformFor(screenW, screenH)
    }

    private fun applyFullscreenTransformFor(screenW: Float, screenH: Float) {

        if (videoWidth <= 0 || videoHeight <= 0) {
            player?.videoFormat?.let { format ->
                if (format.width > 0 && format.height > 0) {
                    videoWidth = format.width.toDouble()
                    videoHeight = format.height.toDouble()
                }
            }
        }

        val videoIsLandscape = isLandscapeVideo()
        val deviceLandscape = screenW >= screenH
        val autoAngle = when {
            videoIsLandscape && !deviceLandscape -> 90f
            !videoIsLandscape && deviceLandscape -> -90f
            else -> 0f
        }
        val angle = explicitDisplayRotationDegrees
            ?.toFloat()
            ?.let { normalizeViewRotation(it) }
            ?: autoAngle
        val swapAxes = isQuarterTurnAngle(angle)
        val hasRotation = normalizeRotation(Math.round(angle)) != 0

        val targetWidth = if (swapAxes) screenH else screenW
        val targetHeight = if (swapAxes) screenW else screenH

        view.layoutParams = FrameLayout.LayoutParams(
            targetWidth.toInt(),
            targetHeight.toInt(),
            Gravity.CENTER
        )
        view.translationX = 0f
        view.translationY = 0f
        view.pivotX = targetWidth / 2f
        view.pivotY = targetHeight / 2f
        view.rotation = if (hasRotation) angle else 0f
        view.scaleX = 1f
        view.scaleY = 1f

        fun setMatchParent(v: View?) {
            v ?: return
            v.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            v.rotation = 0f
            v.scaleX = 1f
            v.scaleY = 1f
        }

        setMatchParent(playerView)
        setMatchParent(streamTextureView)  // CRITICAL: Stream mode uses this, not playerView
        // PlayerView internal TextureView may still carry inline transforms.
        findFirstTextureView(playerView)?.let { tv ->
            tv.rotation = 0f
            tv.scaleX = 1f
            tv.scaleY = 1f
        }
        // Some device/decoder paths render URL video on SurfaceView.
        findFirstSurfaceView(playerView)?.let { sv ->
            sv.rotation = 0f
            sv.scaleX = 1f
            sv.scaleY = 1f
        }
        setMatchParent(posterImageView)
        controlsOverlay?.view?.let { setMatchParent(it) }
        loadingIndicator?.let { loader ->
            loader.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER
            )
            loader.rotation = 0f
            loader.scaleX = 1f
            loader.scaleY = 1f
        }

        // Force layout update
        view.requestLayout()
    }

    private fun hideOverlayViewsFallback(root: View?) {
        if (root == null) return
        val hidden = mutableListOf<Pair<View, Int>>()

        fun visit(node: View) {
            if (node is NavigationBar || node is TabBar) {
                hidden.add(node to node.visibility)
                node.visibility = View.GONE
            }
            if (node is ViewGroup) {
                for (i in 0 until node.childCount) {
                    visit(node.getChildAt(i))
                }
            }
        }

        visit(root)
        if (hidden.isNotEmpty()) {
            fallbackHiddenViews = hidden
        }
    }

    private fun restoreOverlayViewsFallback() {
        fallbackHiddenViews?.forEach { (view, visibility) ->
            view.visibility = visibility
        }
        fallbackHiddenViews = null
    }

    private fun applyOverlayFullscreenFallback(root: View?) {
        if (root == null) return
        val overlay = findOverlayHost(root) ?: return
        if (fallbackOverlayView == null) {
            fallbackOverlayView = overlay
            fallbackOverlayLayoutParams = when (val params = overlay.layoutParams) {
                is FrameLayout.LayoutParams -> FrameLayout.LayoutParams(params)
                is ViewGroup.LayoutParams -> ViewGroup.LayoutParams(params)
                else -> overlay.layoutParams
            }
            fallbackOverlayTranslationX = overlay.translationX
            fallbackOverlayTranslationY = overlay.translationY
        }

        overlay.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        ).apply {
            leftMargin = 0
            topMargin = 0
            rightMargin = 0
            bottomMargin = 0
        }
        overlay.translationX = 0f
        overlay.translationY = 0f
        overlay.requestLayout()
    }

    private fun restoreOverlayFullscreenFallback() {
        val overlay = fallbackOverlayView ?: return
        fallbackOverlayLayoutParams?.let { overlay.layoutParams = it }
        overlay.translationX = fallbackOverlayTranslationX
        overlay.translationY = fallbackOverlayTranslationY
        overlay.requestLayout()
        fallbackOverlayLayoutParams = null
        fallbackOverlayView = null
    }

    private fun findOverlayHost(root: View): View? {
        if (root.tag == "ComponentOverlay") return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                val found = findOverlayHost(root.getChildAt(i))
                if (found != null) return found
            }
        }
        return null
    }

    private fun applyWebViewContainerFullscreenFallback(root: View?) {
        if (root !is ViewGroup) return
        val currentContainer = root.findViewWithTag<View>("current_webview_container") as? ViewGroup
            ?: return
        val webViewContainer = currentContainer.parent as? ViewGroup ?: return
        val lp = webViewContainer.layoutParams as? FrameLayout.LayoutParams ?: return

        if (fallbackWebViewContainer == null) {
            fallbackWebViewContainer = webViewContainer
            fallbackWebViewContainerLayoutParams = FrameLayout.LayoutParams(lp)
        }
        if (fallbackCurrentWebViewContainer == null) {
            fallbackCurrentWebViewContainer = currentContainer
            fallbackCurrentWebViewTranslationY = currentContainer.translationY
        }

        webViewContainer.layoutParams = FrameLayout.LayoutParams(lp).apply {
            width = ViewGroup.LayoutParams.MATCH_PARENT
            height = ViewGroup.LayoutParams.MATCH_PARENT
            topMargin = 0
            bottomMargin = 0
            leftMargin = 0
            rightMargin = 0
        }
        webViewContainer.requestLayout()
        currentContainer.translationY = 0f
        currentContainer.requestLayout()
    }

    private fun restoreWebViewContainerFullscreenFallback() {
        fallbackWebViewContainer?.let { container ->
            fallbackWebViewContainerLayoutParams?.let { container.layoutParams = FrameLayout.LayoutParams(it) }
            container.requestLayout()
        }
        fallbackCurrentWebViewContainer?.let { current ->
            current.translationY = fallbackCurrentWebViewTranslationY
            current.requestLayout()
        }
        fallbackWebViewContainer = null
        fallbackWebViewContainerLayoutParams = null
        fallbackCurrentWebViewContainer = null
        fallbackCurrentWebViewTranslationY = 0f
    }

    private fun getActivityContext(): android.app.Activity? {
        var ctx: Context? = context
        while (ctx != null) {
            if (ctx is android.app.Activity) return ctx
            ctx = (ctx as? android.content.ContextWrapper)?.baseContext
        }

        // Native components are created with application context; fall back to current activity
        LxApp.getCurrentActivity()?.let { return it }
        return view.rootView?.context as? android.app.Activity
    }

    private fun ensureFeedBackendIfNeeded() {
        val id = componentId ?: return
        val core = playerCore ?: return
        val host = surfaceHost ?: return
        if (defaultBackendInitialized && core.getBackend() == BackendKind.FEED) return
        defaultBackendInitialized = true
        host.setActiveBackend(BackendKind.FEED)
        core.setSurfaceToken(host.nextFeedSurfaceToken())
        core.setSource(CorePlayerSource.Feed(sessionId = id))
    }

    private fun loadSource(uri: Uri) {
        if (uri == currentSource) return
        defaultBackendInitialized = true
        currentSource = uri
        firstFrameDisplayed = false
        hasEverRenderedFrame = false
        hasEnded = false
        updatePosterVisibility()
        showLoadingIndicator()

        surfaceHost?.setActiveBackend(BackendKind.URL)
        playerCore?.setSurfaceToken(null)
        playerCore?.setSource(CorePlayerSource.Url(url = uri.toString()))
    }

    private fun loadPoster(url: String, show: Boolean) {
        cancelPosterLoad()
        val requestToken = posterLoadToken
        if (show) {
            updatePosterVisibility()
        }
        try {
            val uri = parseUri(url) ?: return
            val (targetWidth, targetHeight) = resolvePosterTargetSize()
            if (uri.scheme == "http" || uri.scheme == "https") {
                posterLoadFuture = posterExecutor.submit {
                    try {
                        val bitmap = decodeNetworkPoster(url, targetWidth, targetHeight)
                        mainHandler.post {
                            if (requestToken != posterLoadToken) return@post
                            if (bitmap != null) {
                                posterImageView?.setImageBitmap(bitmap)
                            }
                        }
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to load network poster: $url", e)
                    }
                }
            } else {
                posterLoadFuture = posterExecutor.submit {
                    try {
                        val bitmap = decodeLocalPoster(uri, targetWidth, targetHeight)
                        mainHandler.post {
                            if (requestToken != posterLoadToken) return@post
                            if (bitmap != null) {
                                posterImageView?.setImageBitmap(bitmap)
                            } else {
                                posterImageView?.setImageURI(uri)
                            }
                        }
                    } catch (e: Exception) {
                        LxLog.w(TAG, "Failed to load local poster: $uri", e)
                        mainHandler.post {
                            if (requestToken == posterLoadToken) {
                                posterImageView?.setImageURI(uri)
                            }
                        }
                    }
                }
            }
        } catch (e: Exception) {
            LxLog.w(TAG, "Failed to load poster: $url", e)
        }
    }

    private fun cancelPosterLoad() {
        posterLoadFuture?.cancel(true)
        posterLoadFuture = null
        posterLoadToken += 1L
    }

    private fun resolvePosterTargetSize(): Pair<Int, Int> {
        val width = (posterImageView?.width ?: view.width).coerceAtLeast(360)
        val height = (posterImageView?.height ?: view.height).coerceAtLeast(360)
        return width to height
    }

    private fun decodeNetworkPoster(url: String, targetWidth: Int, targetHeight: Int): Bitmap? {
        var connection: java.net.HttpURLConnection? = null
        return try {
            connection = (java.net.URL(url).openConnection() as java.net.HttpURLConnection).apply {
                connectTimeout = 5_000
                readTimeout = 10_000
                doInput = true
            }
            connection.connect()
            if (connection.responseCode !in 200..299) {
                null
            } else {
                connection.inputStream.use { input ->
                    val bytes = readBytesWithLimit(input, MAX_POSTER_DOWNLOAD_BYTES)
                    if (bytes == null) {
                        LxLog.w(TAG, "Poster too large, skipped: $url")
                        return@use null
                    }
                    decodeSampledBitmap(bytes, targetWidth, targetHeight)
                }
            }
        } catch (e: Exception) {
            LxLog.w(TAG, "decodeNetworkPoster failed: $url", e)
            null
        } finally {
            connection?.disconnect()
        }
    }

    private fun decodeSampledBitmap(data: ByteArray, targetWidth: Int, targetHeight: Int): Bitmap? {
        if (data.isEmpty()) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
            return BitmapFactory.decodeByteArray(data, 0, data.size)
        }

        val sampleSize = calculateInSampleSize(
            bounds.outWidth,
            bounds.outHeight,
            targetWidth,
            targetHeight
        )
        val options = BitmapFactory.Options().apply {
            inSampleSize = sampleSize
            inPreferredConfig = Bitmap.Config.RGB_565
        }
        return BitmapFactory.decodeByteArray(data, 0, data.size, options)
    }

    private fun decodeLocalPoster(uri: Uri, targetWidth: Int, targetHeight: Int): Bitmap? {
        if (uri.scheme == "file") {
            val path = uri.path ?: return null
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(path, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                return BitmapFactory.decodeFile(
                    path,
                    BitmapFactory.Options().apply {
                        inPreferredConfig = Bitmap.Config.RGB_565
                    }
                )
            }
            val sampleSize = calculateInSampleSize(
                bounds.outWidth,
                bounds.outHeight,
                targetWidth,
                targetHeight
            )
            val options = BitmapFactory.Options().apply {
                inSampleSize = sampleSize
                inPreferredConfig = Bitmap.Config.RGB_565
            }
            return BitmapFactory.decodeFile(path, options)
        }

        return context.contentResolver.openInputStream(uri)?.use { input ->
            val bytes = readBytesWithLimit(input, MAX_POSTER_DOWNLOAD_BYTES) ?: return@use null
            decodeSampledBitmap(bytes, targetWidth, targetHeight)
        }
    }

    private fun readBytesWithLimit(input: java.io.InputStream, limitBytes: Int): ByteArray? {
        val output = ByteArrayOutputStream(min(limitBytes, 16 * 1024))
        val buffer = ByteArray(8 * 1024)
        var total = 0
        while (true) {
            val read = input.read(buffer)
            if (read <= 0) break
            total += read
            if (total > limitBytes) {
                return null
            }
            output.write(buffer, 0, read)
        }
        return output.toByteArray()
    }

    private fun calculateInSampleSize(
        width: Int,
        height: Int,
        targetWidth: Int,
        targetHeight: Int
    ): Int {
        var sampleSize = 1
        var currentWidth = width
        var currentHeight = height
        while (currentWidth / 2 >= targetWidth || currentHeight / 2 >= targetHeight) {
            currentWidth /= 2
            currentHeight /= 2
            sampleSize *= 2
        }
        return sampleSize.coerceAtLeast(1)
    }

    private fun setObjectFit(fit: LxMediaObjectFit) {
        objectFit = fit
        playerView?.resizeMode = fit.toResizeMode()
        posterImageView?.scaleType = posterScaleTypeForObjectFit(fit)
        transitionOverlay.setObjectFit(fit)
    }

    private fun posterScaleTypeForObjectFit(fit: LxMediaObjectFit): ImageView.ScaleType {
        return when (fit) {
            LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
            LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
        }
    }

    private fun setDisplayRotationDegrees(degrees: Int?) {
        val normalized = degrees?.let { normalizeRotation(it) }
        if (normalized != null && normalized != 0 && normalized != 90 && normalized != 180 && normalized != 270) {
            LxLog.w(TAG, "Ignoring invalid rotate value: input=$degrees normalized=$normalized")
            return
        }
        explicitDisplayRotationDegrees = normalized
        displayRotationDegrees = normalized ?: 0
        if (isFullscreen) {
            if (inlineFullscreenParent != null) {
                applyInlineFullscreenTransform()
            } else {
                applyFullscreenTransform()
            }
        } else {
            applyInlineDisplayRotationTransform()
        }
    }

    private fun findFirstTextureView(root: View?): TextureView? {
        root ?: return null
        if (root is TextureView) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                val found = findFirstTextureView(root.getChildAt(i))
                if (found != null) return found
            }
        }
        return null
    }

    private fun findFirstSurfaceView(root: View?): SurfaceView? {
        root ?: return null
        if (root is SurfaceView) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                val found = findFirstSurfaceView(root.getChildAt(i))
                if (found != null) return found
            }
        }
        return null
    }

    private fun applyInlineDisplayRotationTransform() {
        if (isFullscreen) return

        val degrees = displayRotationDegrees
        val containerW = view.width.toFloat()
        val containerH = view.height.toFloat()
        if (containerW <= 0f || containerH <= 0f) return

        val (scaleX, scaleY) = computeInlineRotationScales(degrees, containerW, containerH)
        if (scaleX.isNaN() || scaleY.isNaN()) {
            // Video size not in yet — defer the whole apply until LoadedMetadata
            // re-triggers us via updatePreferredOrientation. Without this we'd
            // pollute playerView's transform with stale fallback numbers; with
            // it the playerView remains at its laid-out identity until we have
            // the real fit, and the fragment doesn't reveal the host yet.
            return
        }

        fun apply(v: View?) {
            v ?: return
            v.pivotX = v.width / 2f
            v.pivotY = v.height / 2f
            v.rotation = degrees.toFloat()
            v.scaleX = scaleX
            v.scaleY = scaleY
        }

        fun reset(v: View?) {
            v ?: return
            v.rotation = 0f
            v.scaleX = 1f
            v.scaleY = 1f
        }

        // Rotate URL playback at PlayerView level for stability across decoders.
        // Some device pipelines override internal surface transforms every frame.
        apply(playerView)
        // Keep the transition overlay's bitmap aligned with the rotated player.
        transitionOverlay.applyInlineTransform(degrees, scaleX, scaleY)
        reset(findFirstTextureView(playerView))
        reset(findFirstSurfaceView(playerView))
        apply(streamTextureView)
        apply(posterImageView)
    }

    private fun computeInlineRotationScales(
        degrees: Int,
        containerW: Float,
        containerH: Float,
    ): Pair<Float, Float> {
        val rotate90 = degrees == 90 || degrees == 270
        if (!rotate90) {
            return 1f to 1f
        }

        if (objectFit == LxMediaObjectFit.FILL) {
            val ratioX = containerW / containerH
            val ratioY = containerH / containerW
            return ratioX to ratioY
        }

        val (sourceW, sourceH) = getDisplayVideoSize()
        if (sourceW <= 0.0 || sourceH <= 0.0) {
            // Metadata not in yet — caller must skip the apply rather than
            // settle for a container-ratio fallback (that produced a visible
            // 0.45x shrunken first frame until the metadata-triggered
            // re-apply corrected it). The fragment-level reveal is gated on
            // firstframerendered, which by ExoPlayer's event order fires
            // after LoadedMetadata, so the surface stays hidden during the
            // brief unmeasured window.
            return Float.NaN to Float.NaN
        }

        val baseScale = fitScale(sourceW, sourceH, containerW.toDouble(), containerH.toDouble())
        val rotatedScale = fitScale(sourceH, sourceW, containerW.toDouble(), containerH.toDouble())
        if (baseScale <= 0.0 || rotatedScale <= 0.0) {
            return 1f to 1f
        }

        val uniform = (rotatedScale / baseScale).toFloat()
        return uniform to uniform
    }

    private fun fitScale(
        sourceW: Double,
        sourceH: Double,
        containerW: Double,
        containerH: Double,
    ): Double {
        if (sourceW <= 0.0 || sourceH <= 0.0 || containerW <= 0.0 || containerH <= 0.0) {
            return 0.0
        }

        val scaleX = containerW / sourceW
        val scaleY = containerH / sourceH
        return when (objectFit) {
            LxMediaObjectFit.COVER -> max(scaleX, scaleY)
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> min(scaleX, scaleY)
            LxMediaObjectFit.FILL -> min(scaleX, scaleY)
        }
    }

    private fun setCornerRadius(radius: Double) {
        cornerRadius = radius
        view.outlineProvider = android.view.ViewOutlineProvider.BACKGROUND
        view.clipToOutline = true
        val drawable = android.graphics.drawable.GradientDrawable().apply {
            setColor(Color.BLACK)
            this.cornerRadius = radius.toFloat() * context.resources.displayMetrics.density
        }
        view.background = drawable
    }

    private fun parseUri(src: String): Uri? {
        return try {
            val resolved = resolveSandboxUri(src) ?: return null
            val uri = Uri.parse(resolved)
            if (uri.scheme.isNullOrEmpty() && resolved.startsWith("/")) {
                Uri.fromFile(File(resolved))
            } else {
                uri
            }
        } catch (e: Exception) {
            null
        }
    }

    private fun resolveSandboxUri(value: String): String? {
        val raw = value.trim()
        if (raw.isEmpty()) return null

        if (raw.startsWith("http://") || raw.startsWith("https://")) return raw

        val appId = (context as? LxAppActivity)?.getAppId()
            ?: LxApp.getCurrentActivity()?.getAppId()
            ?: return null

        return NativeApi.resolveLxUri(appId, raw)
    }

    private fun updatePreferredOrientation(width: Double, height: Double, rotationDegrees: Int = videoRotationDegrees) {
        if (width <= 0 || height <= 0) return
        val widthChanged = videoWidth != width
        val heightChanged = videoHeight != height
        val rotChanged = videoRotationDegrees != normalizeRotation(rotationDegrees)
        videoWidth = width
        videoHeight = height
        videoRotationDegrees = normalizeRotation(rotationDegrees)

        if (isFullscreen) {
            if (inlineFullscreenParent != null) {
                applyInlineFullscreenTransform()
            } else {
                applyFullscreenTransform()
            }
        } else if (widthChanged || heightChanged || rotChanged) {
            // Inline rotation scale depends on these (via computeInlineRotationScales),
            // so re-apply once metadata lands. Without this, the first apply uses the
            // fallback "video size unknown" path and the scale stays wrong until some
            // unrelated layout change happens to fire the layout listener.
            applyInlineDisplayRotationTransform()
        }
    }

    private fun handleCoreEvent(event: CorePlayerEvent) {
        when (event) {
            CorePlayerEvent.PlayRequest -> {
                // Intent-only. UI feedback (e.g. spinner) is driven by `waiting`.
                if (!isPausedByUser) {
                    isBufferingForUi = true
                }
                controlsOverlay?.updatePlayPauseButton()
            }
            CorePlayerEvent.Play -> {
                isBufferingForUi = false
                if (hasEnded) {
                    hasEnded = false
                    updatePosterVisibility()
                }
                controlsOverlay?.updatePlayPauseButton()
            }
            is CorePlayerEvent.Waiting -> {
                isBufferingForUi = !isPausedByUser
                if (!isPausedByUser) {
                    showLoadingIndicator()
                }
                controlsOverlay?.updatePlayPauseButton()
                updatePosterVisibility()
            }
            is CorePlayerEvent.Playing -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                uiSeeking = false
                firstFrameDisplayed = true
                hasEverRenderedFrame = true
                if (hasEnded) {
                    hasEnded = false
                }
                updatePosterVisibility()
                controlsOverlay?.updatePlayPauseButton()
            }
            CorePlayerEvent.FirstFrameRendered -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                firstFrameDisplayed = true
                hasEverRenderedFrame = true
                updatePosterVisibility()
            }
            is CorePlayerEvent.Pause -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                uiSeeking = false
                controlsOverlay?.updatePlayPauseButton()
            }
            is CorePlayerEvent.Seeking -> {
                uiSeeking = true
                if (!isPausedByUser) {
                    isBufferingForUi = true
                }
                if (!isPausedByUser) {
                    showLoadingIndicator()
                }
                controlsOverlay?.updatePlayPauseButton()
            }
            is CorePlayerEvent.Seeked -> {
                uiSeeking = false
                isBufferingForUi = false
                hideLoadingIndicator()
                val durationMs = playerCore?.getLastKnownDurationMs()
                val durationSeconds = (durationMs ?: 0L).toDouble() / 1000.0
                controlsOverlay?.updateProgress(event.currentTimeMs.toDouble() / 1000.0, durationSeconds)
            }
            is CorePlayerEvent.TimeUpdate -> {
                val currentSeconds = event.currentTimeMs.toDouble() / 1000.0
                val durationSeconds = ((event.durationMs ?: 0L).toDouble() / 1000.0)
                controlsOverlay?.updateProgress(currentSeconds, durationSeconds)
                val prev = lastUiTimeUpdateMs
                lastUiTimeUpdateMs = event.currentTimeMs
                if (!isPausedByUser && !uiSeeking && prev != null && event.currentTimeMs > prev + 50) {
                    hideLoadingIndicator()
                }
            }
            is CorePlayerEvent.LoadedMetadata -> {
                val width = event.width.toDouble()
                val height = event.height.toDouble()
                updatePreferredOrientation(width, height, event.rotation)
                applyInlineDisplayRotationTransform()
            }
            is CorePlayerEvent.Ended -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                uiSeeking = false
                hasEnded = true
                updatePosterVisibility()
                // Only surface the center play / pause UI when controls are
                // actually enabled. With controls=false (e.g. swiper pages),
                // `showCenterPlayButton(true)` would re-show the overlay
                // because it bypasses the `isEnabled` gate — leaving a play
                // button stuck on the page after the first playback cycle.
                if (controlsEnabled) {
                    controlsOverlay?.showCenterPlayButton(true)
                    controlsOverlay?.updatePlayPauseButton()
                }
            }
            is CorePlayerEvent.Error -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                uiSeeking = false
                if (controlsEnabled) {
                    controlsOverlay?.showCenterPlayButton(true)
                    controlsOverlay?.updatePlayPauseButton()
                }
            }
            is CorePlayerEvent.RateChange -> Unit
            is CorePlayerEvent.VolumeChange -> {
                controlsOverlay?.updateVolumeState(event.muted, event.volume.toDouble())
            }
            is CorePlayerEvent.Stop -> {
                isBufferingForUi = false
                hideLoadingIndicator()
                uiSeeking = false
                firstFrameDisplayed = false
                hasEnded = false
                updatePosterVisibility()
                if (controlsEnabled) {
                    controlsOverlay?.showCenterPlayButton(true)
                    controlsOverlay?.updatePlayPauseButton()
                }
            }
            is CorePlayerEvent.FullscreenChange -> Unit
        }

        eventSink(JsEventMapper.toPayload(event))
        mapCoreEventToTypedEvent(event)?.let { typedEventSink?.invoke(it) }

        // Playlist orchestration runs AFTER the per-item event reaches JS so
        // listeners observe `ended`/`error` for the item that finished before
        // any `playlistchange` for the next item. The controller also drives the
        // transition overlay (hide on FirstFrameRendered / Playing).
        playlistController.onCoreEvent(event)
    }

    private fun mapCoreEventToTypedEvent(event: CorePlayerEvent): LxMediaEvent? {
        return when (event) {
            CorePlayerEvent.PlayRequest,
            CorePlayerEvent.Play,
            is CorePlayerEvent.Playing -> LxMediaEvent.Play
            CorePlayerEvent.FirstFrameRendered -> null
            is CorePlayerEvent.Pause -> LxMediaEvent.Pause
            is CorePlayerEvent.Waiting -> LxMediaEvent.Waiting
            is CorePlayerEvent.Seeking -> null
            is CorePlayerEvent.Seeked -> LxMediaEvent.Seeked(event.currentTimeMs.toDouble() / 1000.0)
            is CorePlayerEvent.TimeUpdate -> LxMediaEvent.TimeUpdate(
                currentTime = event.currentTimeMs.toDouble() / 1000.0,
                duration = (event.durationMs ?: 0L).toDouble() / 1000.0
            )
            is CorePlayerEvent.LoadedMetadata -> LxMediaEvent.LoadedMetadata(
                width = event.width.toDouble(),
                height = event.height.toDouble(),
                duration = (event.durationMs ?: 0L).toDouble() / 1000.0
            )
            is CorePlayerEvent.Ended -> LxMediaEvent.Ended
            is CorePlayerEvent.Error -> LxMediaEvent.Error(
                code = event.code.value,
                message = event.message
            )
            is CorePlayerEvent.RateChange -> LxMediaEvent.RateChange(event.rate.toDouble())
            is CorePlayerEvent.VolumeChange -> LxMediaEvent.VolumeChange(event.volume.toDouble())
            is CorePlayerEvent.Stop -> LxMediaEvent.Stop
            is CorePlayerEvent.FullscreenChange -> null
        }
    }

    private fun emitEvent(event: LxMediaEvent) {
        eventSink(event.rawPayload)
        typedEventSink?.invoke(event)
    }

    internal fun isStreamDecoderMode(): Boolean = playerCore?.getBackend() == BackendKind.FEED

    internal fun isPlaying(): Boolean {
        return when (playerCore?.getBackend()) {
            BackendKind.FEED -> activeFeedEngine?.isPlaying() == true
            BackendKind.URL -> activeUrlEngine?.isPlaying() == true
            null -> false
        }
    }

    internal fun shouldShowPauseIconForUi(): Boolean {
        return isPlaying() || (!isPausedByUser && isBufferingForUi)
    }

    internal fun handleStreamDecoderEvent(event: String, detail: Map<String, Any?>) {
        ensureFeedBackendIfNeeded()
        activeFeedEngine?.handleStreamDecoderEvent(event, detail)
    }

    internal fun getCurrentPosition(): Long =
        playerCore?.getLastKnownTimeMs() ?: player?.currentPosition ?: 0L

    internal fun getDuration(): Long =
        playerCore?.getLastKnownDurationMs() ?: (player?.duration?.takeIf { it > 0 } ?: 0L)
    internal fun getAvailableQualities(): List<LxMediaQuality> = availableQualities
    internal fun getCurrentQuality(): String? = currentQuality
    internal fun getAvailableSpeeds(): List<Double> = availablePlaybackRates
    internal fun getCurrentSpeed(): Double = currentPlaybackRate.toDouble()
    internal fun isFullscreen(): Boolean = isFullscreen
    internal fun isMuted(): Boolean = isMuted
    internal fun requestClose() {
        if (isFullscreen) {
            exitFullscreen()
        }
        closeRequestListener?.invoke()
    }

    internal fun emitQualityChange(selectedLabel: String) {
        currentQuality = selectedLabel
        val switchedUrl = availableQualities
            .firstOrNull { it.label == selectedLabel }
            ?.url
            ?.takeIf { it.isNotBlank() }

        val switchedUri = switchedUrl?.let(Uri::parse)
        if (switchedUri != null && switchedUri != currentSource) {
            loadSource(switchedUri)
        }

        emitEvent(LxMediaEvent.QualityChange(selectedLabel, switchedUrl))
    }

    private fun isLandscapeVideo(): Boolean {
        if (videoWidth <= 0 || videoHeight <= 0) return true
        val (displayWidth, displayHeight) = getDisplayVideoSize()
        return displayWidth >= displayHeight
    }

    private fun getDisplayVideoSize(): Pair<Double, Double> {
        val rotation = normalizeRotation(videoRotationDegrees)
        val swap = rotation == 90 || rotation == 270
        return if (swap) Pair(videoHeight, videoWidth) else Pair(videoWidth, videoHeight)
    }

    private fun normalizeRotation(rotation: Int): Int {
        var normalized = rotation % 360
        if (normalized < 0) normalized += 360
        return normalized
    }

    private fun normalizeViewRotation(rotation: Float): Float {
        var normalized = rotation % 360f
        if (normalized <= -180f) normalized += 360f
        if (normalized > 180f) normalized -= 360f
        return normalized
    }

    private fun isQuarterTurnAngle(rotation: Float): Boolean {
        val normalized = normalizeRotation(Math.round(rotation))
        return normalized == 90 || normalized == 270
    }
}
