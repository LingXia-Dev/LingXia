package com.lingxia.lxapp.APIs.media

import android.content.Context
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
import android.view.TextureView
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.SeekBar
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NavigationBar
import com.lingxia.lxapp.R
import com.lingxia.lxapp.SameLevel.ComponentRouter
import com.lingxia.lxapp.TabBar
import java.io.File
import kotlin.math.max

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
data class LxMediaQuality(
    val label: String,
    val url: String? = null
)

// Object fit modes
enum class LxMediaObjectFit {
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
data class LxMediaPlayerConfig(
    var source: LxMediaSource? = null,
    var src: String? = null,
    var poster: String? = null,
    var autoplay: Boolean? = null,
    var loop: Boolean? = null,
    var muted: Boolean? = null,
    var volume: Double? = null,
    var controls: Boolean? = null,
    var progressBar: Boolean? = null,
    var cornerRadius: Double? = null,
    var qualities: List<LxMediaQuality>? = null,
    var speeds: List<Double>? = null,
    var showControlsOnInit: Boolean? = null,
    var objectFit: LxMediaObjectFit? = null
)

// Commands that can be sent to the player
sealed class LxMediaCommand {
    object Play : LxMediaCommand()
    object Pause : LxMediaCommand()
    object Stop : LxMediaCommand()
    data class Seek(val time: Double) : LxMediaCommand()
    data class SetDuration(val duration: Double) : LxMediaCommand()
    data class SetVolume(val volume: Double) : LxMediaCommand()
    data class SetMuted(val muted: Boolean) : LxMediaCommand()
    data class SetPlaybackRate(val rate: Double) : LxMediaCommand()
    object EnterFullscreen : LxMediaCommand()
    object ExitFullscreen : LxMediaCommand()
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
            is RateChange -> "playbackratechange"
            is VolumeChange -> "volumechange"
            is FullscreenChange -> "fullscreenchange"
            is LoadedMetadata -> "loadedmetadata"
            is QualityChange -> "qualitychange"
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
            is Error -> mapOf("code" to code, "message" to message)
            is Raw -> data
        }

    val rawPayload: Map<String, Any>
        get() = mapOf("event" to rawName, "detail" to rawData)
}

/**
 * LxMediaPlayer - A native video player with built-in controls.
 * Designed to be reused by SameLevel components and MediaPreview.
 */
class LxMediaPlayer(
    private val context: Context,
    private val eventSink: (Map<String, Any>) -> Unit,
    private val typedEventSink: ((LxMediaEvent) -> Unit)? = null,
    private val componentId: String? = null
) {
    val view: FrameLayout = FrameLayout(context).apply {
        setBackgroundColor(Color.BLACK)
        clipToOutline = true
    }

    private var player: ExoPlayer? = null
    private var playerView: PlayerView? = null
    private var streamTextureView: TextureView? = null
    private var posterImageView: ImageView? = null
    private var loadingIndicator: ProgressBar? = null
    private var controlsOverlay: LxMediaControlsOverlay? = null

    private var streamDecoderMode = false
    private var overrideDurationSeconds: Double? = null
    private var streamPlaybackBaseOffsetSeconds = 0.0
    private var streamPausedPositionSeconds: Double? = null
    private var streamProgressRunnable: Runnable? = null
    private var streamIsPlaying = false
    private var streamIsBuffering = false
    // Stream intent latch for "src is empty but user pressed play" before a stream source/decoder exists.
    private var streamPlayRequested = false
    private var streamHasOutput = false
    private var streamHasEnded = false
    // CRITICAL: Permanent flag - once ANY frame is rendered, poster should NEVER show again.
    // Unlike streamHasOutput which gets reset on seek/reset, this flag persists until source changes.
    private var hasEverRenderedFrame = false
    private var lastPlaybackPosition = 0.0  // Track last known position for near-end detection
    private var lastKnownDuration = 0.0  // Track last known duration as fallback
    private var pendingSeekAfterEnded = false  // Flag to reset ended state on next acquire after seek
    private var pendingStreamSeekedSeconds: Double? = null
    private var posterVisibilityBeforeStream: Int? = null
    private var loadingVisibilityBeforeStream: Int? = null
    private var shutterColorBeforeStream: Int? = null

    private val mainHandler = Handler(Looper.getMainLooper())
    private var timeUpdateRunnable: Runnable? = null

    // Config state
    private var controlsEnabled = true
    private var loopEnabled = false
    private var currentVolume = 1.0
    private var isMuted = false
    private var currentPlaybackRate = 1.0f
    private var objectFit = LxMediaObjectFit.COVER
    private var cornerRadius = 0.0

    // Quality and Speed
    private var availableQualities: List<LxMediaQuality> = emptyList()
    private var currentQuality: String? = null
    private var availablePlaybackRates: List<Double> = emptyList()

    // State
    private var currentSource: Uri? = null
    private var isFullscreen = false
    private var suppressWaitingUntilMs: Long = 0
    private var isPausedByUser = false  // Track if user explicitly paused (vs buffering)
    private var firstFrameDisplayed = false
    private var posterUrl: String? = null
    private var videoWidth = 0.0
    private var videoHeight = 0.0
    private var videoRotationDegrees = 0
    private var closeRequestListener: (() -> Unit)? = null
    private var lastTimeForPoster: Double = -1.0  // Track time progression for poster hiding
    private var pendingPosterHide = false  // Flag to delay poster hiding until time progresses

    // State restoration for quality switching: (seekToMs, shouldPlay)
    private var pendingRestoreAfterLoad: Pair<Long, Boolean>? = null

    // Fullscreen state
    private var fullscreenDialog: android.app.Dialog? = null
    private var fullscreenContainer: FrameLayout? = null
    private var fullscreenContent: FrameLayout? = null
    private var fullscreenLayoutListener: View.OnLayoutChangeListener? = null
    private var inlineFullscreenParent: ViewGroup? = null
    private var inlineFullscreenLayoutListener: View.OnLayoutChangeListener? = null
    private var originalSystemUiVisibility: Int? = null
    private var originalWindowFlags: Int? = null
    private var originalDecorFitsSystemWindows: Boolean? = null
    private var originalStatusBarColor: Int? = null
    private var originalNavigationBarColor: Int? = null
    private var originalNavBarContrastEnforced: Boolean? = null
    private var originalCutoutMode: Int? = null
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
    private var nextPlayEmitsPlayRequest = false

    // Player listener - must be declared before init block
    private val playerListener = object : Player.Listener {
        override fun onRenderedFirstFrame() {
            if (streamDecoderMode) return
            if (firstFrameDisplayed) return
            firstFrameDisplayed = true
            hasEverRenderedFrame = true  // Permanent - poster will never show again
            pendingPosterHide = false
            updatePosterVisibility()
            loadingIndicator?.visibility = View.GONE
            scheduleRectSync(doublePass = false)
        }

        override fun onPlaybackStateChanged(playbackState: Int) {
            if (streamDecoderMode) {
                return
            }
            when (playbackState) {
                Player.STATE_READY -> {
                    loadingIndicator?.visibility = View.GONE
                    clearWaitingSuppression()

                    // Handle state restoration (e.g. after quality switch)
                    pendingRestoreAfterLoad?.let { (seekToMs, shouldPlay) ->
                        pendingRestoreAfterLoad = null
                        suppressWaitingFor(1500) // Prevent Waiting event during seek
                        player?.seekTo(seekToMs)
                        if (shouldPlay) {
                            player?.play()
                        }
                    }

                    if (!firstFrameDisplayed) {
                        // Don't hide poster immediately - wait for time to progress
                        // This prevents black screen flash when video is buffering
                        pendingPosterHide = true
                        lastTimeForPoster = -1.0
                        emitLoadedMetadata()
                        scheduleRectSync(doublePass = true)
                    }
                }
                Player.STATE_BUFFERING -> {
                    val now = android.os.SystemClock.uptimeMillis()
                    val suppressWaiting = now < suppressWaitingUntilMs
                    // Don't show loading during seek - better UX (frame stays visible)
                    // Also don't show loading if user explicitly paused
                    if (!suppressWaiting && !isPausedByUser) {
                        loadingIndicator?.visibility = View.VISIBLE
                        emitEvent(LxMediaEvent.Waiting)
                    }
                }
                Player.STATE_ENDED -> {
                    loadingIndicator?.visibility = View.GONE
                    clearWaitingSuppression()
                    emitEvent(LxMediaEvent.Ended)
                    if (loopEnabled) {
                        player?.seekTo(0)
                        player?.play()
                    } else {
                        // Keep the last rendered frame visible on ended (don't show poster).
                        firstFrameDisplayed = true
                        pendingPosterHide = false
                        updatePosterVisibility()
                        // Bring controls to front so the center play button is visible.
                        controlsOverlay?.view?.bringToFront()
                        controlsOverlay?.showCenterPlayButton(true)
                        controlsOverlay?.updatePlayPauseButton()  // Update play/pause button icon
                    }
                }
                Player.STATE_IDLE -> {
                    loadingIndicator?.visibility = View.GONE
                    clearWaitingSuppression()
                }
            }
            controlsOverlay?.updatePlayPauseButton()
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (streamDecoderMode) {
                return
            }
            if (isPlaying) {
                emitEvent(LxMediaEvent.Play)
                startTimeUpdates()
                scheduleRectSync(doublePass = false)
            } else {
                // Don't emit pause here - isPlaying=false can be due to buffering
                // Pause event is emitted explicitly in pause() method
                stopTimeUpdates()
            }
            controlsOverlay?.updatePlayPauseButton()
        }

        override fun onPlayerError(error: PlaybackException) {
            Log.e(TAG, "Player error: ${error.message}", error)
            loadingIndicator?.visibility = View.GONE
            emitEvent(LxMediaEvent.Error(
                code = error.errorCode.toString(),
                message = error.message ?: "Unknown error"
            ))
        }

        override fun onVideoSizeChanged(videoSize: VideoSize) {
            if (streamDecoderMode) {
                return
            }
            val w = videoSize.width.toDouble()
            val h = videoSize.height.toDouble()
            if (w > 0 && h > 0) {
                updatePreferredOrientation(w, h, videoSize.unappliedRotationDegrees)
            }
        }
    }

    init {
        setupUI()
        setupPlayer()

        // Ensure video output is ready when view is attached
        view.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(v: View) {
                if (streamDecoderMode) {
                    return
                }
                playerView?.player = player
            }
            override fun onViewDetachedFromWindow(v: View) = Unit
        })
    }

    private fun setupPlayer() {
        try {
            // Use application context to avoid memory leaks and context issues
            val appContext = context.applicationContext

            // Optimized load control for better seek/buffering experience
            val loadControl = DefaultLoadControl.Builder()
                .setBufferDurationsMs(
                    25000,  // minBufferMs - buffer at least 25s
                    50000,  // maxBufferMs - buffer up to 50s
                    1500,   // bufferForPlaybackMs - start playback with 1.5s buffer
                    3000    // bufferForPlaybackAfterRebufferMs - after rebuffer, need 3s
                )
                .build()

            val exoPlayer = ExoPlayer.Builder(appContext)
                .setLoadControl(loadControl)
                .setSeekParameters(SeekParameters.CLOSEST_SYNC)  // Fast seek to nearest keyframe
                .build()

            val audioAttributes = AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
                .build()
            exoPlayer.setAudioAttributes(audioAttributes, true)
            exoPlayer.setHandleAudioBecomingNoisy(true)

            exoPlayer.addListener(playerListener)
            player = exoPlayer
            playerView?.player = exoPlayer
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create ExoPlayer", e)
        }
    }

    private fun setupUI() {
        // PlayerView
        // Inflate PlayerView configured to use TextureView (see res/layout/lx_media_player_view.xml)
        playerView = LayoutInflater.from(context)
            .inflate(R.layout.lx_media_player_view, view, false) as PlayerView
        playerView?.apply {
            useController = false // We use custom controls
            setShowBuffering(PlayerView.SHOW_BUFFERING_WHEN_PLAYING)
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
            scaleType = ImageView.ScaleType.CENTER_CROP
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

        controlsOverlay = LxMediaControlsOverlay(context, this).also {
            view.addView(it.view)
        }
    }

    fun update(config: LxMediaPlayerConfig) {
        config.source?.let { source ->
            loadSource(source.toUri())
        } ?: config.src?.let { src ->
            loadSource(parseUri(src))
        }

        config.poster?.let {
            posterUrl = it
            loadPoster(it, show = shouldShowPoster())
        }
        updatePosterVisibility()
        config.loop?.let { loopEnabled = it }
        config.muted?.let { setMuted(it) }
        config.volume?.let { setVolume(it) }
        config.controls?.let { controlsEnabled = it; controlsOverlay?.setVisible(it) }
        config.progressBar?.let { controlsOverlay?.setShowProgressBar(it) }
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
            player?.setPlaybackSpeed(currentPlaybackRate)
        }
        config.autoplay?.let { if (it) player?.playWhenReady = true }
        config.objectFit?.let { setObjectFit(it) }
        controlsOverlay?.updateSettingsButton()
    }

    private fun shouldShowPoster(): Boolean {
        val hasPoster = !posterUrl.isNullOrBlank()
        if (!hasPoster) return false
        
        // PROFESSIONAL PLAYER STANDARD: Poster ONLY shows on cold start.
        // Once ANY frame has been rendered, poster should NEVER show again.
        if (hasEverRenderedFrame) return false
        
        // For native player mode
        if (!streamDecoderMode) {
            if (componentId != null && currentSource == null) return true
            return !firstFrameDisplayed
        }
        
        // For stream decoder mode: only show before first output
        return !streamHasOutput
    }

    private fun updatePosterVisibility() {
        val poster = posterImageView ?: return
        val shouldShow = shouldShowPoster()
        val oldVisibility = poster.visibility
        poster.visibility = if (shouldShow) View.VISIBLE else View.GONE
        Log.d(TAG, "updatePosterVisibility: shouldShow=$shouldShow, old=${if (oldVisibility == View.VISIBLE) "VISIBLE" else "GONE"}, new=${if (poster.visibility == View.VISIBLE) "VISIBLE" else "GONE"}, streamHasOutput=$streamHasOutput, streamHasEnded=$streamHasEnded")
        if (poster.visibility == View.VISIBLE) {
            poster.bringToFront()
            controlsOverlay?.view?.bringToFront()
            loadingIndicator?.bringToFront()
        }
    }

    fun handle(command: LxMediaCommand) {
        when (command) {
            is LxMediaCommand.Play -> play()
            is LxMediaCommand.Pause -> pause()
            is LxMediaCommand.Stop -> stop()
            is LxMediaCommand.Seek -> seek(command.time)
            is LxMediaCommand.SetDuration -> {
                val duration = command.duration
                if (duration.isFinite() && duration > 0) {
                    overrideDurationSeconds = duration
                    streamPlaybackBaseOffsetSeconds = 0.0
                    controlsOverlay?.updateProgress(0.0, duration)
                    startStreamProgressUpdatesIfNeeded()
                } else {
                    overrideDurationSeconds = null
                    streamPlaybackBaseOffsetSeconds = 0.0
                    controlsOverlay?.updateProgress(0.0, 0.0)
                    stopStreamProgressUpdates()
                }
            }
            is LxMediaCommand.SetVolume -> setVolume(command.volume)
            is LxMediaCommand.SetMuted -> setMuted(command.muted)
            is LxMediaCommand.SetPlaybackRate -> setPlaybackRate(command.rate)
            is LxMediaCommand.EnterFullscreen -> enterFullscreen()
            is LxMediaCommand.ExitFullscreen -> exitFullscreen()
        }
    }

    fun acquireStreamTextureView(): TextureView? {
        val caller = Thread.currentThread().stackTrace.getOrNull(3)?.let { "${it.className}.${it.methodName}:${it.lineNumber}" } ?: "unknown"
        Log.d(TAG, "acquireStreamTextureView called from $caller, streamDecoderMode=$streamDecoderMode, streamHasEnded=$streamHasEnded")
        // Check if this is acquisition after seek from ended state
        val isSeekAfterEnded = pendingSeekAfterEnded
        if (isSeekAfterEnded) {
            pendingSeekAfterEnded = false
            Log.d(TAG, "acquireStreamTextureView: clearing ended state due to seek")
        }
        
        // Preserve ended state UNLESS this is a seek operation
        val preserveEndedState = streamHasEnded && !isSeekAfterEnded
        
        if (!streamDecoderMode) {
            streamDecoderMode = true
            streamHasOutput = false
            streamHasEnded = false
            streamIsBuffering = streamPlayRequested
            streamPlaybackBaseOffsetSeconds = 0.0
            streamPausedPositionSeconds = null
            posterVisibilityBeforeStream = posterImageView?.visibility
            loadingVisibilityBeforeStream = loadingIndicator?.visibility

            shutterColorBeforeStream = Color.BLACK
            playerView?.setShutterBackgroundColor(Color.TRANSPARENT)
        }
        
        // Restore ended state if preserved (not a seek operation)
        if (preserveEndedState) {
            streamHasEnded = true
        }
        
        // Reset playback state
        streamIsPlaying = false
        if (!preserveEndedState) {
            streamIsBuffering = streamPlayRequested
        }
        
        // Use centralized poster visibility logic
        Log.d(TAG, "acquireStreamTextureView: updating poster, preserveEndedState=$preserveEndedState, streamHasEnded=$streamHasEnded")
        updatePosterVisibility()
        
        // Show loading only if not ended
        if (!preserveEndedState) {
            loadingIndicator?.visibility = if (streamIsBuffering && !isPausedByUser) View.VISIBLE else View.GONE
            if (loadingIndicator?.visibility == View.VISIBLE) {
                loadingIndicator?.bringToFront()
                controlsOverlay?.view?.bringToFront()
            }
        }
        streamTextureView?.visibility = View.VISIBLE
        playerView?.visibility = View.GONE
        player?.pause()
        player?.clearVideoSurface()
        playerView?.player = null
        val view = streamTextureView
        if (view == null) {
            Log.w(TAG, "Stream decoder: TextureView not available")
        }
        startStreamProgressUpdatesIfNeeded()
        return view
    }

    fun releaseStreamTextureView() {
        val caller = Thread.currentThread().stackTrace.getOrNull(3)?.let { "${it.className}.${it.methodName}:${it.lineNumber}" } ?: "unknown"
        Log.d(TAG, "releaseStreamTextureView called from $caller, streamDecoderMode=$streamDecoderMode, streamHasEnded=$streamHasEnded")
        if (streamDecoderMode) {
            // Fallback ended detection: if playback stopped within last 5% OR 3s of duration, treat as ended
            // This handles cases where VideoContext doesn't send ended event and progress timer stops early
            var nearEnd = false
            Log.d(TAG, "[NEAR-END] checking: streamHasEnded=$streamHasEnded, componentId=$componentId")
            if (!streamHasEnded && componentId != null) {
                var duration = overrideDurationSeconds
                // Fallback: if duration is null (e.g., after seek), use last known duration
                if (duration == null || duration <= 0) {
                    duration = if (lastKnownDuration > 0) lastKnownDuration else null
                } else {
                    // Update last known duration when we have a valid one
                    lastKnownDuration = duration
                }
                Log.d(TAG, "[NEAR-END] duration=$duration, lastPlaybackPosition=$lastPlaybackPosition")  
                if (duration != null && duration > 0) {
                    // Use lastPlaybackPosition instead of querying ComponentRouter
                    // because stream decoder may have stopped and returns 0
                    val raw = lastPlaybackPosition
                    val diff = duration - raw
                    // Use percentage-based threshold: within last 5% OR last 3 seconds, whichever is larger
                    val threshold = maxOf(duration * 0.05, 3.0)
                    Log.d(TAG, "[NEAR-END] raw=$raw, diff=$diff, threshold=$threshold")
                    if (diff < threshold && diff >= 0) {
                        nearEnd = true
                        Log.d(TAG, "[NEAR-END] *** DETECTED *** raw=$raw, duration=$duration, diff=$diff, threshold=$threshold")
                    } else {
                        Log.d(TAG, "[NEAR-END] NOT near end: diff=$diff >= threshold=$threshold")
                    }
                } else {
                    Log.d(TAG, "[NEAR-END] invalid duration")
                }
            } else {
                Log.d(TAG, "[NEAR-END] skipped: already ended or no componentId")
            }
            // Preserve streamHasEnded if video has ended - this prevents poster from showing
            // when stream decoder is stopped and recreated (e.g. during seek after ended)
            val preserveEnded = streamHasEnded || nearEnd
            streamDecoderMode = false
            streamIsPlaying = false
            streamHasOutput = false
            streamIsBuffering = false
            streamHasEnded = preserveEnded  // Preserve ended state
            streamPlayRequested = false
            streamPlaybackBaseOffsetSeconds = 0.0
            streamPausedPositionSeconds = null
            overrideDurationSeconds = null
            stopStreamProgressUpdates()
            controlsOverlay?.updateProgress(0.0, 0.0)  // Update visibility when duration cleared
            // Don't restore poster/loading if video ended - keep them hidden
            if (!preserveEnded) {
                posterVisibilityBeforeStream?.let { posterImageView?.visibility = it }
                loadingVisibilityBeforeStream?.let { loadingIndicator?.visibility = it }
            }
            posterVisibilityBeforeStream = null
            loadingVisibilityBeforeStream = null

            val color = shutterColorBeforeStream ?: Color.BLACK
            playerView?.setShutterBackgroundColor(color)
            shutterColorBeforeStream = null
            Log.d(TAG, "releaseStreamTextureView: preserveEnded=$preserveEnded, streamHasEnded now=$streamHasEnded")
        }
        // If video ended, keep texture view visible to show last frame
        // Otherwise hide it and show playerView
        if (!streamHasEnded) {
            streamTextureView?.visibility = View.GONE
            playerView?.visibility = View.VISIBLE
            playerView?.player = player
        } else {
            Log.d(TAG, "releaseStreamTextureView: keeping texture view visible for last frame")
        }
    }

    fun attach(to: ViewGroup) {
        if (view.parent != to) {
            (view.parent as? ViewGroup)?.removeView(view)
            to.addView(view)
        }
    }

    fun detach() {
        stopTimeUpdates()
        stopStreamProgressUpdates()
        rectSyncRunnable?.let { mainHandler.removeCallbacks(it) }
        rectSyncRunnable = null
        player?.stop()
        player?.release()
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

    fun requestPlay() {
        nextPlayEmitsPlayRequest = true
        play()
    }

    fun play() {
        val emitPlayRequest = nextPlayEmitsPlayRequest
        nextPlayEmitsPlayRequest = false

        // Stream-mode placeholder (no src) + no decoder yet: emit play intent instead of letting
        // ExoPlayer enter ENDED immediately (empty playlist), which hides the poster and prevents
        // JS from lazily calling setStreamSource().
        if (!streamDecoderMode && componentId != null && currentSource == null) {
            if (!emitPlayRequest) {
                return
            }
            isPausedByUser = false
            streamPlayRequested = true
            updatePosterVisibility()
            loadingIndicator?.visibility = View.VISIBLE
            loadingIndicator?.bringToFront()
            controlsOverlay?.view?.bringToFront()
            controlsOverlay?.updatePlayPauseButton()
            emitEvent(LxMediaEvent.Raw("playrequest", emptyMap()))
            emitEvent(LxMediaEvent.Raw("waiting", mapOf("reason" to "buffering")))
            scheduleRectSync(doublePass = true)
            return
        }
        if (streamDecoderMode && componentId != null) {
            isPausedByUser = false
            if (emitPlayRequest) {
                emitEvent(LxMediaEvent.Raw("playrequest", emptyMap()))
            }
            val resumeFrom = streamPausedPositionSeconds
            val duration = overrideDurationSeconds
            if (streamHasEnded) {
                streamHasEnded = false
                streamPlaybackBaseOffsetSeconds = 0.0
                streamPausedPositionSeconds = null
                controlsOverlay?.updateProgress(0.0, duration ?: 0.0)
                com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                    componentId,
                    "resetStream",
                    """{"hard":false,"emitWaiting":false}"""
                )
            }
            if (resumeFrom != null && duration != null && duration > 0) {
                // Pause/resume restarts the provider stream with PTS reset to 0. Preserve the
                // absolute position by shifting the base offset and resetting the decoder PTS.
                val clamped = resumeFrom.coerceIn(0.0, duration)
                streamPlaybackBaseOffsetSeconds = clamped
                controlsOverlay?.updateProgress(clamped, duration)
                com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                    componentId,
                    "resetStream",
                    """{"hard":false,"emitWaiting":false}"""
                )
                streamPausedPositionSeconds = null
            } else {
                streamPausedPositionSeconds = null
            }

            streamIsPlaying = true
            streamIsBuffering = true
            streamPlayRequested = true
            updatePosterVisibility()
            loadingIndicator?.visibility = View.VISIBLE
            loadingIndicator?.bringToFront()
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                componentId,
                "play",
                "{}"
            )
            // In stream mode, PlayerView doesn't drive layout updates; request a rect sync so the
            // native view stays aligned with DOM changes (e.g. switching view state).
            scheduleRectSync(doublePass = true)
            controlsOverlay?.updatePlayPauseButton()
            return
        }
        val p = player ?: return
        isPausedByUser = false  // User wants to play
        when (p.playbackState) {
            Player.STATE_ENDED -> {
                p.seekTo(0)
            }
            Player.STATE_IDLE -> {
                // After stop(), player is in IDLE state and needs prepare() again
                p.prepare()
            }
        }
        p.setPlaybackSpeed(currentPlaybackRate)
        p.play()
    }

    fun pause() {
        if (!streamDecoderMode && componentId != null && currentSource == null) {
            isPausedByUser = true
            streamPlayRequested = false
            loadingIndicator?.visibility = View.GONE
            updatePosterVisibility()
            controlsOverlay?.updatePlayPauseButton()
            emitEvent(LxMediaEvent.Raw("pause", mapOf("reason" to "user")))
            return
        }
        if (streamDecoderMode && componentId != null) {
            isPausedByUser = true
            streamIsPlaying = false
            streamPlayRequested = false
            clearWaitingSuppression()
            overrideDurationSeconds?.let { duration ->
                if (duration > 0) {
                    val relative = ComponentRouter.streamPlaybackPositionSeconds(componentId) ?: 0.0
                    val current =
                        (streamPlaybackBaseOffsetSeconds + relative).coerceIn(0.0, duration)
                    streamPausedPositionSeconds = current
                }
            }
            val currentTime = streamPausedPositionSeconds
            val paramsJson =
                if (currentTime != null && currentTime.isFinite() && currentTime >= 0.0) {
                    """{"currentTime":$currentTime}"""
                } else {
                    "{}"
                }
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(componentId, "pause", paramsJson)
            loadingIndicator?.visibility = View.GONE
            controlsOverlay?.updatePlayPauseButton()
            return
        }
        isPausedByUser = true  // User explicitly paused
        clearWaitingSuppression()
        player?.pause()
        // Keep the last rendered frame visible; never show poster on pause.
        updatePosterVisibility()
        // Always emit pause event when user explicitly pauses
        // (onIsPlayingChanged no longer sends pause to avoid confusion with buffering)
        emitEvent(LxMediaEvent.Pause)
        stopTimeUpdates()
        loadingIndicator?.visibility = View.GONE
    }

    fun stop() {
        isPausedByUser = true  // Stopped by user
        if (!streamDecoderMode && componentId != null && currentSource == null) {
            streamPlayRequested = false
            clearWaitingSuppression()
            firstFrameDisplayed = false
            hasEverRenderedFrame = false  // TRUE COLD START - only stop() resets this
            pendingPosterHide = false
            updatePosterVisibility()
            loadingIndicator?.visibility = View.GONE
            controlsOverlay?.showCenterPlayButton(true)
            controlsOverlay?.updatePlayPauseButton()
            emitEvent(LxMediaEvent.Raw("stop", mapOf("reason" to "user")))
            return
        }
        if (streamDecoderMode && componentId != null) {
            streamIsPlaying = false
            streamHasOutput = false
            hasEverRenderedFrame = false  // TRUE COLD START - only stop() resets this
            streamIsBuffering = false
            streamPlayRequested = false
            streamPlaybackBaseOffsetSeconds = 0.0
            streamPausedPositionSeconds = null
            clearWaitingSuppression()
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(componentId, "pause", "{}")
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                componentId,
                "resetStream",
                """{"hard":true}"""
            )
            updatePosterVisibility()
            controlsOverlay?.view?.bringToFront()
            controlsOverlay?.showCenterPlayButton(true)
            controlsOverlay?.updatePlayPauseButton()
            overrideDurationSeconds?.let { controlsOverlay?.updateProgress(0.0, it) }
            emitEvent(LxMediaEvent.Stop)
            return
        }
        // Get duration before stopping
        val duration = (player?.duration ?: 0L).toDouble() / 1000.0
        clearWaitingSuppression()

        player?.stop()
        player?.seekTo(0)

        // Reset to initial state - show poster and center play button (like video ended)
        firstFrameDisplayed = false
        pendingPosterHide = false
        updatePosterVisibility()
        controlsOverlay?.view?.bringToFront()
        controlsOverlay?.showCenterPlayButton(true)
        controlsOverlay?.updateProgress(0.0, duration)

        emitEvent(LxMediaEvent.Stop)
    }

    fun seek(time: Double) {
        if (streamDecoderMode && componentId != null) {
            val duration = overrideDurationSeconds ?: return
            val clamped = time.coerceIn(0.0, duration)
            streamPlaybackBaseOffsetSeconds = clamped
            streamPausedPositionSeconds = null
            pendingStreamSeekedSeconds = clamped
            controlsOverlay?.updateProgress(clamped, duration)
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                componentId,
                "resetStream",
                """{"hard":false}"""
            )
            emitEvent(LxMediaEvent.Raw("seeking", mapOf("time" to clamped)))
            startStreamProgressUpdatesIfNeeded()
            return
        }
        val positionMs = (time * 1000).toLong()
        suppressWaitingFor(1500)
        player?.seekTo(positionMs)
        emitEvent(LxMediaEvent.Seeked(time))
        updateProgressUIAfterSeek(time)
    }

    fun setVolume(volume: Double) {
        currentVolume = volume.coerceIn(0.0, 1.0)

        if (streamDecoderMode && componentId != null) {
            // In stream mode, dispatch to StreamDecoderSession
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                componentId,
                "setVolume",
                """{"volume":$currentVolume}"""
            )
        } else {
            // Normal mode: control ExoPlayer
            if (!isMuted) {
                player?.volume = currentVolume.toFloat()
            }
        }

        emitEvent(LxMediaEvent.VolumeChange(currentVolume))
        controlsOverlay?.updateVolumeState(isMuted, currentVolume)
    }

    fun setMuted(muted: Boolean) {
        isMuted = muted

        if (streamDecoderMode && componentId != null) {
            // In stream mode, dispatch to StreamDecoderSession
            com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                componentId,
                "setMuted",
                """{"muted":$muted}"""
            )
        } else {
            // Normal mode: control ExoPlayer
            player?.volume = if (muted) 0f else currentVolume.toFloat()
        }

        controlsOverlay?.updateVolumeState(isMuted, currentVolume)
    }

    fun setPlaybackRate(rate: Double) {
        currentPlaybackRate = rate.toFloat()
        player?.setPlaybackSpeed(currentPlaybackRate)
        emitEvent(LxMediaEvent.RateChange(rate))
    }

    fun setShowCloseButton(show: Boolean) {
        controlsOverlay?.setShowCloseButton(show)
    }

    fun setShowFullscreenButton(show: Boolean) {
        controlsOverlay?.setShowFullscreenButton(show)
    }

    fun setCloseRequestListener(listener: (() -> Unit)?) {
        closeRequestListener = listener
    }

    fun enterFullscreen() {
        if (isFullscreen) return

        if (streamDecoderMode) {
            enterInlineFullscreen()
            return
        }

        // Get Activity context - required for Dialog
        val activityContext = getActivityContext() ?: run {
            Log.w(TAG, "enterFullscreen: Cannot get Activity context")
            return
        }

        isFullscreen = true

        val hostActivity = (activityContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            Log.w(TAG, "enterFullscreen: host activity not found; using overlay fallback")
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
                attributes = attributes?.apply {
                    layoutInDisplayCutoutMode = android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
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
                if (streamDecoderMode) {
                } else {
                    playerView?.player = player
                }
                if (streamDecoderMode && componentId != null) {
                    com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                        componentId,
                        "rebindSurface",
                        "{}"
                    )
                    com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                        componentId,
                        "play",
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
        emitEvent(LxMediaEvent.FullscreenChange(true, direction))
    }

    fun exitFullscreen() {
        if (!isFullscreen) return
        if (streamDecoderMode && fullscreenDialog == null) {
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
                Log.w(TAG, "exitFullscreen: Error dismissing dialog", e)
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
        emitEvent(LxMediaEvent.FullscreenChange(false, "vertical"))
    }

    private fun enterInlineFullscreen() {
        if (isFullscreen) return
        val activityContext = getActivityContext() ?: run {
            Log.w(TAG, "enterFullscreen: Cannot get Activity context")
            return
        }

        isFullscreen = true
        val hostActivity = (activityContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            Log.w(TAG, "enterFullscreen: host activity not found; using overlay fallback")
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
        emitEvent(LxMediaEvent.FullscreenChange(true, direction))
    }

    private fun exitInlineFullscreen() {
        if (!isFullscreen) return
        isFullscreen = false

        val hostContext = getActivityContext()
        val hostActivity = (hostContext as? com.lingxia.lxapp.LxAppActivity)
            ?: (LxApp.getCurrentActivity() as? com.lingxia.lxapp.LxAppActivity)
        if (hostActivity == null) {
            Log.w(TAG, "exitFullscreen: host activity not found; using overlay fallback")
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
        emitEvent(LxMediaEvent.FullscreenChange(false, "vertical"))
    }

    private fun applyInlineFullscreenUi(activity: android.app.Activity) {
        val window = activity.window ?: return
        if (originalSystemUiVisibility == null) {
            originalSystemUiVisibility = window.decorView.systemUiVisibility
        }
        if (originalWindowFlags == null) {
            originalWindowFlags = window.attributes.flags
        }
        if (originalDecorFitsSystemWindows == null) {
            originalDecorFitsSystemWindows = ViewCompat.getFitsSystemWindows(window.decorView)
        }
        if (originalStatusBarColor == null) {
            originalStatusBarColor = window.statusBarColor
        }
        if (originalNavigationBarColor == null) {
            originalNavigationBarColor = window.navigationBarColor
        }
        if (originalNavBarContrastEnforced == null && android.os.Build.VERSION.SDK_INT >= 29) {
            originalNavBarContrastEnforced = window.isNavigationBarContrastEnforced
        }
        if (originalCutoutMode == null) {
            originalCutoutMode = window.attributes.layoutInDisplayCutoutMode
        }

        WindowCompat.setDecorFitsSystemWindows(window, false)
        val decorView = window.decorView
        decorView.systemUiVisibility = (
            View.SYSTEM_UI_FLAG_FULLSCREEN or
                View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
                View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE
            )

        WindowInsetsControllerCompat(window, decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        window.clearFlags(android.view.WindowManager.LayoutParams.FLAG_FORCE_NOT_FULLSCREEN)
        window.addFlags(
            android.view.WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS or
            android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN or
            android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
            android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
        )
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            window.isNavigationBarContrastEnforced = false
        }
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.hide(
                android.view.WindowInsets.Type.statusBars() or
                    android.view.WindowInsets.Type.navigationBars()
            )
        }
        decorView.post {
            WindowInsetsControllerCompat(window, decorView).hide(WindowInsetsCompat.Type.systemBars())
        }
    }

    private fun restoreInlineFullscreenUi(activity: android.app.Activity) {
        val window = activity.window ?: return
        originalSystemUiVisibility?.let { window.decorView.systemUiVisibility = it }
        originalSystemUiVisibility = null

        originalDecorFitsSystemWindows?.let { WindowCompat.setDecorFitsSystemWindows(window, it) }
        originalDecorFitsSystemWindows = null

        originalWindowFlags?.let { flags ->
            window.attributes = window.attributes.apply { this.flags = flags }
        }
        originalWindowFlags = null
        originalStatusBarColor?.let { window.statusBarColor = it }
        originalStatusBarColor = null
        originalNavigationBarColor?.let { window.navigationBarColor = it }
        originalNavigationBarColor = null
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            originalNavBarContrastEnforced?.let { window.isNavigationBarContrastEnforced = it }
        }
        originalNavBarContrastEnforced = null

        originalCutoutMode?.let { mode ->
            window.attributes = window.attributes.apply { layoutInDisplayCutoutMode = mode }
        }
        originalCutoutMode = null

        WindowInsetsControllerCompat(window, window.decorView).show(WindowInsetsCompat.Type.systemBars())
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
        val rotate = videoIsLandscape != deviceLandscape
        val angle = when {
            videoIsLandscape && !deviceLandscape -> 90f
            !videoIsLandscape && deviceLandscape -> -90f
            else -> 0f
        }

        val targetWidth = if (rotate) screenH else screenW
        val targetHeight = if (rotate) screenW else screenH

        view.layoutParams = FrameLayout.LayoutParams(
            targetWidth.toInt(),
            targetHeight.toInt(),
            Gravity.CENTER
        )
        view.translationX = 0f
        view.translationY = 0f
        view.pivotX = targetWidth / 2f
        view.pivotY = targetHeight / 2f
        view.rotation = if (rotate) angle else 0f
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
        if (root.tag == "SameLevelOverlay") return root
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

        // SameLevel components are created with application context; fall back to current activity
        LxApp.getCurrentActivity()?.let { return it }
        return view.rootView?.context as? android.app.Activity
    }

    private fun loadSource(uri: Uri?) {
        uri ?: return
        if (uri == currentSource) return
        streamPlayRequested = false
        currentSource = uri
        firstFrameDisplayed = false
        pendingPosterHide = false  // Reset poster hide state
        lastTimeForPoster = -1.0
        updatePosterVisibility()
        loadingIndicator?.visibility = View.VISIBLE
        player?.apply {
            setMediaItem(MediaItem.fromUri(uri))
            prepare()
        }
    }

    private fun loadPoster(url: String, show: Boolean) {
        if (show) {
            updatePosterVisibility()
        }
        try {
            val uri = parseUri(url) ?: return
            if (uri.scheme == "http" || uri.scheme == "https") {
                // Load network image in background thread
                Thread {
                    try {
                        val connection = java.net.URL(url).openConnection() as java.net.HttpURLConnection
                        connection.doInput = true
                        connection.connect()
                        val input = connection.inputStream
                        val bitmap = android.graphics.BitmapFactory.decodeStream(input)
                        mainHandler.post {
                            posterImageView?.setImageBitmap(bitmap)
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to load network poster: $url", e)
                    }
                }.start()
            } else {
                posterImageView?.setImageURI(uri)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to load poster: $url", e)
        }
    }

    private fun setObjectFit(fit: LxMediaObjectFit) {
        objectFit = fit
        playerView?.resizeMode = fit.toResizeMode()
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
            val uri = Uri.parse(src)
            if (uri.scheme.isNullOrEmpty() && src.startsWith("/")) {
                Uri.fromFile(File(src))
            } else {
                uri
            }
        } catch (e: Exception) {
            null
        }
    }

    private fun emitLoadedMetadata() {
        val p = player ?: return
        val format = p.videoFormat
        val width = format?.width?.toDouble() ?: 0.0
        val height = format?.height?.toDouble() ?: 0.0
        val rotation = format?.rotationDegrees ?: 0
        updatePreferredOrientation(width, height, rotation)
        val duration = p.duration.toDouble() / 1000.0
        emitEvent(LxMediaEvent.LoadedMetadata(width, height, duration))
    }

    private fun updatePreferredOrientation(width: Double, height: Double, rotationDegrees: Int = videoRotationDegrees) {
        if (width <= 0 || height <= 0) return
        videoWidth = width
        videoHeight = height
        videoRotationDegrees = normalizeRotation(rotationDegrees)

        if (isFullscreen) {
            if (inlineFullscreenParent != null) {
                applyInlineFullscreenTransform()
            } else {
                applyFullscreenTransform()
            }
        }
    }

    private fun emitEvent(event: LxMediaEvent) {
        // Prevent waiting events from showing poster/loading after video has ended
        if (event is LxMediaEvent.Waiting && streamHasEnded) {
            Log.d(TAG, "emitEvent: ignoring Waiting event, streamHasEnded=true")
            return
        }
        eventSink(event.rawPayload)
        typedEventSink?.invoke(event)
    }

    private fun startTimeUpdates() {
        stopTimeUpdates()
        lastTimeForPoster = -1.0  // Reset on timer start
        timeUpdateRunnable = object : Runnable {
            override fun run() {
                player?.let { p ->
                    if (p.isPlaying) {
                        val currentTime = p.currentPosition.toDouble() / 1000.0
                        val duration = p.duration.toDouble() / 1000.0

                        // Hide poster when video is actually rendering frames (time progresses)
                            // This prevents black screen flash.
                            if (pendingPosterHide && duration > 0) {
                                val timeAdvanced = lastTimeForPoster >= 0 && currentTime > lastTimeForPoster
                                if (currentTime > 0.2 && timeAdvanced) {
                                    pendingPosterHide = false
                                    firstFrameDisplayed = true
                                    hasEverRenderedFrame = true  // Permanent
                                    updatePosterVisibility()
                                }
                                lastTimeForPoster = currentTime
                        }

                        emitEvent(LxMediaEvent.TimeUpdate(currentTime, duration))
                        controlsOverlay?.updateProgress(currentTime, duration)
                    }
                }
                mainHandler.postDelayed(this, 250)
            }
        }
        mainHandler.post(timeUpdateRunnable!!)
    }

    private fun stopTimeUpdates() {
        timeUpdateRunnable?.let { mainHandler.removeCallbacks(it) }
        timeUpdateRunnable = null
    }

    internal fun isStreamDecoderMode(): Boolean = streamDecoderMode

    private fun startStreamProgressUpdatesIfNeeded() {
        if (!streamDecoderMode) {
            stopStreamProgressUpdates()
            return
        }
        val id = componentId ?: run {
            stopStreamProgressUpdates()
            return
        }
        val duration = overrideDurationSeconds ?: run {
            stopStreamProgressUpdates()
            return
        }
        if (duration <= 0) {
            stopStreamProgressUpdates()
            return
        }
        if (!streamIsPlaying || streamIsBuffering || !streamHasOutput) {
            stopStreamProgressUpdates()
            return
        }
        if (streamProgressRunnable != null) return

        streamProgressRunnable = object : Runnable {
            override fun run() {
                if (!streamDecoderMode) {
                    stopStreamProgressUpdates()
                    return
                }
                if (!streamIsPlaying || streamIsBuffering || !streamHasOutput) {
                    stopStreamProgressUpdates()
                    return
                }
                val d = overrideDurationSeconds
                if (d == null || d <= 0) {
                    stopStreamProgressUpdates()
                    return
                }
                val relative = ComponentRouter.streamPlaybackPositionSeconds(id) ?: 0.0
                val raw = streamPlaybackBaseOffsetSeconds + relative
                lastPlaybackPosition = raw  // Update last known position
                val current = raw.coerceIn(0.0, d)
                val endEpsilonSeconds = 0.03
                if (!loopEnabled && !streamHasEnded && raw >= d - endEpsilonSeconds) {
                    streamHasEnded = true
                    streamIsPlaying = false
                    streamIsBuffering = false
                    streamPlayRequested = false
                    isPausedByUser = true
                    // Ensure poster stays hidden on ended; keep the last rendered frame visible.
                    streamHasOutput = true
                    hasEverRenderedFrame = true  // Permanent - poster will never show again
                    streamPausedPositionSeconds = d
                    loadingIndicator?.visibility = View.GONE
                    stopStreamProgressUpdates()
                    controlsOverlay?.updateProgress(d, d)
                    controlsOverlay?.showCenterPlayButton(true)
                    controlsOverlay?.updatePlayPauseButton()
                    updatePosterVisibility()
                    com.lingxia.lxapp.SameLevel.ComponentRouter.dispatchVideoCommand(
                        id,
                        "pause",
                        """{"currentTime":$d,"reason":"ended","emitEvent":false}"""
                    )
                    controlsOverlay?.updatePlayPauseButton()  // Update play/pause button icon
                    emitEvent(LxMediaEvent.Ended)
                    return
                }
                controlsOverlay?.updateProgress(current, d)
                emitEvent(LxMediaEvent.TimeUpdate(current, d))
                mainHandler.postDelayed(this, 250)
            }
        }
        mainHandler.post(streamProgressRunnable!!)
    }

    private fun stopStreamProgressUpdates() {
        streamProgressRunnable?.let { mainHandler.removeCallbacks(it) }
        streamProgressRunnable = null
    }

    internal fun isStreamEnded(): Boolean = streamHasEnded

    internal fun setStreamEnded(ended: Boolean) {
        Log.d(TAG, "setStreamEnded: $ended (was: $streamHasEnded)")
        streamHasEnded = ended
    }

    internal fun handleSeekAfterEnded() {
        if (streamHasEnded) {
            Log.d(TAG, "handleSeekAfterEnded: setting pendingSeekAfterEnded=true")
            pendingSeekAfterEnded = true
        }
    }

    internal fun isPlaying(): Boolean =
        if (streamDecoderMode) (streamIsPlaying || streamPlayRequested) else (streamPlayRequested || player?.isPlaying == true)
    internal fun handleStreamDecoderEvent(event: String) {
        if (!streamDecoderMode) return

        when (event) {
            "waiting" -> {
                // After video has ended, don't show loading or update poster - keep last frame visible
                if (streamHasEnded) {
                    Log.d(TAG, "waiting event ignored: streamHasEnded=true")
                    return
                }
                if (!isPausedByUser) {
                    // Don't show loading indicator if we've already rendered frames and video is playing.
                    // This prevents loading indicator showing after ended or during minor buffering.
                    val wasPlaying = streamIsPlaying
                    if (streamHasOutput && wasPlaying) {
                        return
                    }
                    streamIsBuffering = true
                    loadingIndicator?.visibility = View.VISIBLE
                    loadingIndicator?.bringToFront()
                    controlsOverlay?.updatePlayPauseButton()
                }
                updatePosterVisibility()
                stopStreamProgressUpdates()
            }
            "play" -> {
                streamIsPlaying = true
                streamPlayRequested = false
                streamHasOutput = true
                hasEverRenderedFrame = true  // Permanent - poster will never show again
                streamIsBuffering = false
                streamHasEnded = false
                updatePosterVisibility()
                loadingIndicator?.visibility = View.GONE
                clearWaitingSuppression()
                controlsOverlay?.updatePlayPauseButton()
                startStreamProgressUpdatesIfNeeded()
                pendingStreamSeekedSeconds?.let { sought ->
                    pendingStreamSeekedSeconds = null
                    emitEvent(LxMediaEvent.Seeked(sought))
                }
            }
            "pause", "stop" -> {
                streamIsPlaying = false
                streamPlayRequested = false
                streamIsBuffering = false
                if (event == "stop") {
                    streamHasOutput = false
                    streamHasEnded = false
                }
                updatePosterVisibility()
                loadingIndicator?.visibility = View.GONE
                clearWaitingSuppression()
                controlsOverlay?.updatePlayPauseButton()
                stopStreamProgressUpdates()
                if (event == "stop") {
                    pendingStreamSeekedSeconds = null
                }
            }
        }
    }
    internal fun getCurrentPosition(): Long = player?.currentPosition ?: 0
    internal fun getDuration(): Long = player?.duration ?: 0
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
            pendingRestoreAfterLoad = (player?.currentPosition ?: 0L) to (player?.isPlaying == true)
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

    private fun updateProgressUIAfterSeek(positionSeconds: Double) {
        val durationMs = player?.duration ?: return
        if (durationMs <= 0) return
        controlsOverlay?.updateProgress(positionSeconds, durationMs.toDouble() / 1000.0)
    }

    private fun suppressWaitingFor(durationMs: Long) {
        val now = android.os.SystemClock.uptimeMillis()
        suppressWaitingUntilMs = max(suppressWaitingUntilMs, now + durationMs)
    }

    private fun clearWaitingSuppression() {
        suppressWaitingUntilMs = 0
    }
}
