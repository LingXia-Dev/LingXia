package com.lingxia.lxapp.APIs.media

import android.content.Context
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.SeekBar
import android.widget.TextView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.R
import java.io.File

private const val TAG = "LxMediaPlayer"

// Media source types
sealed class LxMediaSource {
    data class Url(val url: String) : LxMediaSource()
    data class FilePath(val path: String) : LxMediaSource()
    data class Pipe(val path: String) : LxMediaSource()

    fun toUri(): Uri? = when (this) {
        is Url -> Uri.parse(url)
        is FilePath -> Uri.fromFile(File(path))
        is Pipe -> Uri.fromFile(File(path))
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
    data class QualityRequest(val available: List<LxMediaQuality>, val current: String?) : LxMediaEvent()
    data class SpeedRequest(val available: List<Double>, val current: Double?) : LxMediaEvent()
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
            is QualityRequest -> "qualityrequest"
            is SpeedRequest -> "speedrequest"
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
            is QualityRequest -> buildMap {
                put("availableQualities", available.map { mapOf("label" to it.label, "url" to (it.url ?: "")) })
                current?.let { put("currentQuality", it) }
            }
            is SpeedRequest -> buildMap {
                put("availableRates", available)
                current?.let { put("currentRate", it) }
            }
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
    private val typedEventSink: ((LxMediaEvent) -> Unit)? = null
) {

    val view: FrameLayout = FrameLayout(context).apply {
        setBackgroundColor(Color.BLACK)
        clipToOutline = true
    }

    private var player: ExoPlayer? = null
    private var playerView: PlayerView? = null
    private var posterImageView: ImageView? = null
    private var loadingIndicator: ProgressBar? = null
  private var controlsOverlay: LxMediaControlsOverlay? = null

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
    private var firstFrameDisplayed = false
    private var posterUrl: String? = null
    private var preferredOrientation: Int? = null
    private var videoWidth = 0.0
    private var videoHeight = 0.0
    private var videoRotationDegrees = 0
    private var closeRequestListener: (() -> Unit)? = null
    private var lastTimeForPoster: Double = -1.0  // Track time progression for poster hiding
    private var pendingPosterHide = false  // Flag to delay poster hiding until time progresses

    // Fullscreen state
    private var fullscreenDialog: android.app.Dialog? = null
    private var fullscreenContainer: FrameLayout? = null
    private var fullscreenContent: FrameLayout? = null
    private var fullscreenLayoutListener: View.OnLayoutChangeListener? = null
    private var originalParent: ViewGroup? = null
    private var originalIndex: Int = 0
    private var originalLayoutParams: ViewGroup.LayoutParams? = null
    private var originalClipToOutline: Boolean? = null
    private var originalOutlineProvider: android.view.ViewOutlineProvider? = null

    // Player listener - must be declared before init block
    private val playerListener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            when (playbackState) {
                Player.STATE_READY -> {
                    loadingIndicator?.visibility = View.GONE
                    if (!firstFrameDisplayed) {
                        // Don't hide poster immediately - wait for time to progress
                        // This prevents black screen flash when video is buffering
                        pendingPosterHide = true
                        lastTimeForPoster = -1.0
                        emitLoadedMetadata()
                    }
                }
                Player.STATE_BUFFERING -> {
                    loadingIndicator?.visibility = View.VISIBLE
                    emitEvent(LxMediaEvent.Waiting)
                }
                Player.STATE_ENDED -> {
                    loadingIndicator?.visibility = View.GONE
                    emitEvent(LxMediaEvent.Ended)
                    if (loopEnabled) {
                        player?.seekTo(0)
                        player?.play()
                    } else {
                        // Show poster when video ends (non-loop mode)
                        // Reset state so poster can show again on next play
                        firstFrameDisplayed = false
                        pendingPosterHide = false
                        if (posterUrl != null) {
                            posterImageView?.visibility = View.VISIBLE
                            posterImageView?.bringToFront()  // Ensure poster is above playerView
                        }
                        // Bring controls to front AFTER poster so they're visible on top
                        controlsOverlay?.view?.bringToFront()
                        controlsOverlay?.showCenterPlayButton(true)
                    }
                }
                Player.STATE_IDLE -> {
                    loadingIndicator?.visibility = View.GONE
                }
            }
            controlsOverlay?.updatePlayPauseButton()
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (isPlaying) {
                emitEvent(LxMediaEvent.Play)
                startTimeUpdates()
            } else {
                emitEvent(LxMediaEvent.Pause)
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
                Log.d(TAG, "View attached to window, re-setting player to PlayerView")
                playerView?.player = player
            }
            override fun onViewDetachedFromWindow(v: View) {
                Log.d(TAG, "View detached from window")
            }
        })
    }

    private fun setupPlayer() {
        try {
            // Use application context to avoid memory leaks and context issues
            val appContext = context.applicationContext
            Log.d(TAG, "Creating ExoPlayer with context: $appContext")
            val exoPlayer = ExoPlayer.Builder(appContext).build()
            Log.d(TAG, "ExoPlayer created: $exoPlayer")

            val audioAttributes = AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
                .build()
            exoPlayer.setAudioAttributes(audioAttributes, true)
            exoPlayer.setHandleAudioBecomingNoisy(true)

            exoPlayer.addListener(playerListener)
            player = exoPlayer
            playerView?.player = exoPlayer
            Log.d(TAG, "ExoPlayer setup complete, playerView.player assigned")
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
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER
            )
            indeterminateDrawable?.setTint(android.graphics.Color.rgb(0, 122, 255))
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

        config.poster?.let { posterUrl = it; loadPoster(it) }
        config.autoplay?.let { if (it) player?.playWhenReady = true }
        config.loop?.let { loopEnabled = it }
        config.muted?.let { setMuted(it) }
        config.volume?.let { setVolume(it) }
        config.controls?.let { controlsEnabled = it; controlsOverlay?.setVisible(it) }
        config.cornerRadius?.let { setCornerRadius(it) }
        config.qualities?.let { availableQualities = it }
        availablePlaybackRates = config.speeds ?: emptyList()
        config.objectFit?.let { setObjectFit(it) }
        controlsOverlay?.updateSettingsButton()
    }

    fun handle(command: LxMediaCommand) {
        when (command) {
            is LxMediaCommand.Play -> play()
            is LxMediaCommand.Pause -> pause()
            is LxMediaCommand.Stop -> stop()
            is LxMediaCommand.Seek -> seek(command.time)
            is LxMediaCommand.SetVolume -> setVolume(command.volume)
            is LxMediaCommand.SetMuted -> setMuted(command.muted)
            is LxMediaCommand.SetPlaybackRate -> setPlaybackRate(command.rate)
            is LxMediaCommand.EnterFullscreen -> enterFullscreen()
            is LxMediaCommand.ExitFullscreen -> exitFullscreen()
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
        player?.stop()
        player?.release()
        player = null
        (view.parent as? ViewGroup)?.removeView(view)
    }

    fun setFrame(x: Float, y: Float, width: Float, height: Float) {
        view.layoutParams = FrameLayout.LayoutParams(width.toInt(), height.toInt()).apply {
            leftMargin = x.toInt()
            topMargin = y.toInt()
        }
        view.requestLayout()
    }

    fun play() {
        if (player?.playbackState == Player.STATE_ENDED) {
            player?.seekTo(0)
        }
        player?.play()
    }

    fun pause() {
        player?.pause()
    }

    fun stop() {
        player?.stop()
        player?.seekTo(0)
        emitEvent(LxMediaEvent.Stop)
    }

    fun seek(time: Double) {
        val positionMs = (time * 1000).toLong()
        player?.seekTo(positionMs)
        emitEvent(LxMediaEvent.Seeked(time))
        updateProgressUIAfterSeek(time)
    }

    fun setVolume(volume: Double) {
        currentVolume = volume.coerceIn(0.0, 1.0)
        if (!isMuted) {
            player?.volume = currentVolume.toFloat()
        }
        emitEvent(LxMediaEvent.VolumeChange(currentVolume))
        controlsOverlay?.updateVolumeState(isMuted, currentVolume)
    }

    fun setMuted(muted: Boolean) {
        isMuted = muted
        player?.volume = if (muted) 0f else currentVolume.toFloat()
        controlsOverlay?.updateVolumeState(isMuted, currentVolume)
    }

    fun setPlaybackRate(rate: Double) {
        Log.d(TAG, "setPlaybackRate: $rate")
        currentPlaybackRate = rate.toFloat()
        player?.setPlaybackSpeed(currentPlaybackRate)
        emitEvent(LxMediaEvent.RateChange(rate))
        // Also emit SpeedRequest event to update any listeners
        emitEvent(LxMediaEvent.SpeedRequest(availablePlaybackRates, rate))
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

        // Get Activity context - required for Dialog
        val activityContext = getActivityContext() ?: run {
            Log.w(TAG, "enterFullscreen: Cannot get Activity context")
            return
        }

        isFullscreen = true

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
        Log.d(TAG, "enterFullscreen: videoWidth=$videoWidth, videoHeight=$videoHeight")

        // Determine if video is landscape
        val isLandscapeVideo = isLandscapeVideo()
        val direction = if (isLandscapeVideo) "horizontal" else "vertical"
        Log.d(TAG, "enterFullscreen: isLandscapeVideo=$isLandscapeVideo, direction=$direction")

        // Create container to ensure full coverage
        val fullscreenContainer = FrameLayout(activityContext).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        val contentWrapper = FrameLayout(activityContext).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        fullscreenContainer.addView(contentWrapper)

        fullscreenDialog = android.app.Dialog(activityContext, android.R.style.Theme_Black_NoTitleBar_Fullscreen).apply {
            setContentView(fullscreenContainer)
            setCancelable(true)
            setCanceledOnTouchOutside(false)

            // Set fullscreen flags
            window?.apply {
                setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                setBackgroundDrawable(ColorDrawable(Color.BLACK))
                statusBarColor = Color.BLACK
                navigationBarColor = Color.BLACK
                attributes = attributes?.apply {
                    layoutInDisplayCutoutMode = android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                }
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
                Log.d(TAG, "Fullscreen dialog shown, ensuring player is attached")
                playerView?.player = player
            }

            setOnDismissListener {
                Log.d(TAG, "Fullscreen dialog dismissed")
            }

            show()
        }

        // Update view layout for fullscreen
        view.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        contentWrapper.addView(
            view,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
        )
        fullscreenContainer.also { this.fullscreenContainer = it }
        fullscreenContent = contentWrapper

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
        Log.d(TAG, "exitFullscreen: starting")
        isFullscreen = false

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
            view.layoutParams = originalLayoutParams ?: FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            parent.addView(view, originalIndex.coerceIn(0, parent.childCount))
            Log.d(TAG, "exitFullscreen: view restored to parent, index=$originalIndex")
        }

        // Restore rounding state
        originalClipToOutline?.let { view.clipToOutline = it }
        view.outlineProvider = originalOutlineProvider
        originalClipToOutline = null
        originalOutlineProvider = null

        controlsOverlay?.onFullscreenChanged(false)
        emitEvent(LxMediaEvent.FullscreenChange(false, "vertical"))
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

    private fun applyFullscreenTransform() {
        val container = fullscreenContent ?: return
        val host = fullscreenContainer
        val dm = container.context.resources.displayMetrics
        val screenW = (host?.width?.takeIf { it > 0 } ?: container.width.takeIf { it > 0 } ?: dm.widthPixels).toFloat()
        val screenH = (host?.height?.takeIf { it > 0 } ?: container.height.takeIf { it > 0 } ?: dm.heightPixels).toFloat()

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
        currentSource = uri
        firstFrameDisplayed = false
        pendingPosterHide = false  // Reset poster hide state
        lastTimeForPoster = -1.0
        posterImageView?.visibility = if (posterUrl != null) View.VISIBLE else View.GONE
        loadingIndicator?.visibility = View.VISIBLE
        player?.apply {
            setMediaItem(MediaItem.fromUri(uri))
            prepare()
        }
    }

    private fun loadPoster(url: String) {
        posterImageView?.visibility = View.VISIBLE
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
                            Log.d(TAG, "loadPoster: network image loaded")
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

        val (displayWidth, displayHeight) = getDisplayVideoSize()
        preferredOrientation = when {
            displayWidth > displayHeight * 1.1 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            displayHeight > displayWidth * 1.1 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
            else -> ActivityInfo.SCREEN_ORIENTATION_SENSOR
        }

        if (isFullscreen) {
            applyFullscreenTransform()
        }
    }

    private fun emitEvent(event: LxMediaEvent) {
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
                        // This prevents black screen flash (like HarmonyOS implementation)
                        if (pendingPosterHide && duration > 0) {
                            val timeAdvanced = lastTimeForPoster >= 0 && currentTime > lastTimeForPoster
                            if (currentTime > 0.2 && timeAdvanced) {
                                Log.d(TAG, "Hiding poster: currentTime=$currentTime, lastTime=$lastTimeForPoster")
                                pendingPosterHide = false
                                firstFrameDisplayed = true
                                posterImageView?.visibility = View.GONE
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

    internal fun isPlaying(): Boolean = player?.isPlaying == true
    internal fun getCurrentPosition(): Long = player?.currentPosition ?: 0
    internal fun getDuration(): Long = player?.duration ?: 0
    internal fun getAvailableQualities(): List<LxMediaQuality> = availableQualities
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

    internal fun emitQualityRequest(selectedLabel: String) {
        currentQuality = selectedLabel
        Log.d(TAG, "Quality request: $selectedLabel, available: ${availableQualities.map { it.label }}")

        // Find the selected quality and switch to its URL if available
        val selectedQuality = availableQualities.find { it.label == selectedLabel }
        selectedQuality?.url?.let { url ->
            if (url.isNotEmpty()) {
                Log.d(TAG, "Switching to quality URL: $url")
                // Remember current position and playing state
                val currentPosition = player?.currentPosition ?: 0L
                val wasPlaying = player?.isPlaying == true

                // Load the new source
                loadSource(Uri.parse(url))

                // Seek to previous position after a short delay to allow buffering
                mainHandler.postDelayed({
                    player?.seekTo(currentPosition)
                    if (wasPlaying) {
                        player?.play()
                    }
                    Log.d(TAG, "Resumed at position: ${currentPosition}ms, playing: $wasPlaying")
                }, 500)
            }
        }

        emitEvent(LxMediaEvent.QualityRequest(availableQualities, currentQuality))
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
}
