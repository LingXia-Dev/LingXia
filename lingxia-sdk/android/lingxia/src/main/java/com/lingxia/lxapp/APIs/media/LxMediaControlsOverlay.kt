package com.lingxia.lxapp.APIs.media

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.graphics.drawable.ShapeDrawable
import android.graphics.drawable.shapes.OvalShape
import android.os.Handler
import android.os.Looper
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.animation.OvershootInterpolator
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import com.lingxia.lxapp.R
import kotlin.math.abs

private const val TAG = "LxMediaControls"

/**
 * Custom video controls overlay for LxMediaPlayer.
 */
internal class LxMediaControlsOverlay(
    private val context: Context,
    private val player: LxMediaPlayer
) {
    val view: FrameLayout = FrameLayout(context).apply {
        layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
    }

    init {
        // If the overlay view gets detached (e.g. fullscreen transition), ongoing animations may not
        // complete, leaving the dim overlay/popup stuck on top of the video (audio plays, video looks black).
        view.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(v: View) = Unit
            override fun onViewDetachedFromWindow(v: View) {
                dismissSettingsPopup(immediate = true)
            }
        })
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var hideControlsRunnable: Runnable? = null
    private var controlsVisible = false
    private var isEnabled = true
    private var showCloseButton = false
    private var showFullscreenButton = true
    private var showSettingsButton = false
    private var showProgressBar = true
    private var isSeeking = false
    private var lockedSeekSeconds: Double? = null

    companion object {
        val ACCENT_BLUE = Color.rgb(0, 122, 255)
        val ACCENT_BLUE_ALPHA = Color.argb(51, 0, 122, 255)
        val TRACK_BG = Color.argb(77, 255, 255, 255)
        val POPUP_BG = Color.argb(247, 46, 46, 46)
        val POPUP_BORDER = Color.argb(38, 255, 255, 255)
        val TEXT_SECONDARY = Color.argb(153, 255, 255, 255)
        const val POPUP_OVERLAY_TAG = 9999
        const val POPUP_MENU_TAG = 9998
    }

    private lateinit var topGradient: View
    private lateinit var bottomGradient: View
    private lateinit var topBar: FrameLayout
    private lateinit var bottomBar: FrameLayout
    private lateinit var closeButton: ImageButton
    private lateinit var titleLabel: TextView
    private lateinit var progressRow: LinearLayout
    private lateinit var progressSeekBar: SeekBar
    private lateinit var timeLabel: TextView
    private lateinit var playPauseButton: ImageButton
    private lateinit var volumeButton: ImageButton
    private lateinit var volumeSeekBar: SeekBar
    private lateinit var settingsButton: ImageButton
    private lateinit var fullscreenButton: ImageButton
    private var currentDurationSeconds: Double = 0.0
    private var timeLabelWide = false

    init {
        setupUI()
        setControlsVisible(false)
        updateSettingsButton()
    }

    private fun setupUI() {
        // Use a GestureDetector to distinguish single tap from other gestures
        // This prevents accidental triggers during scrolling or quick touches
        val gestureDetector = android.view.GestureDetector(context, object : android.view.GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapConfirmed(e: android.view.MotionEvent): Boolean {
                toggleControls()
                return true
            }
        })

        val tapLayer = View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            isClickable = true
            isFocusable = true
            setOnTouchListener { _, event ->
                gestureDetector.onTouchEvent(event)
                true
            }
        }
        view.addView(tapLayer)

        topGradient = View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(100)
            ).apply { gravity = Gravity.TOP }
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(Color.parseColor("#B3000000"), Color.TRANSPARENT)
            )
            alpha = 0f
        }
        view.addView(topGradient)

        bottomGradient = View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(140)
            ).apply { gravity = Gravity.BOTTOM }
            background = GradientDrawable(
                GradientDrawable.Orientation.BOTTOM_TOP,
                intArrayOf(Color.parseColor("#B3000000"), Color.TRANSPARENT)
            )
            alpha = 0f
        }
        view.addView(bottomGradient)

        topBar = createTopBar()
        view.addView(topBar)

        bottomBar = createBottomBar()
        view.addView(bottomBar)
    }

    private fun createTopBar(): FrameLayout {
        val bar = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(50),
                Gravity.TOP
            )
            alpha = 0f
        }

        closeButton = ImageButton(context).apply {
            layoutParams = FrameLayout.LayoutParams(dp(44), dp(40)).apply {
                leftMargin = dp(8)
                topMargin = dp(5)
            }
            setBackgroundColor(Color.TRANSPARENT)
            setImageResource(R.drawable.icon_close)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(8), dp(8), dp(8), dp(8))
            visibility = View.GONE
            setOnClickListener { onCloseClick() }
        }
        bar.addView(closeButton)

        titleLabel = TextView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                dp(40)
            ).apply {
                leftMargin = dp(60)
                topMargin = dp(5)
            }
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            gravity = Gravity.CENTER_VERTICAL
            text = ""
        }
        bar.addView(titleLabel)

        return bar
    }

    private fun createBottomBar(): FrameLayout {
        val bar = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(100),
                Gravity.BOTTOM
            )
            alpha = 0f
        }

        val padding = dp(12)  // Reduced from 16
        val buttonWidth = dp(40)  // Reduced from 44
        val progressTop = dp(6)
        val progressRowHeight = dp(44)

        progressRow = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                progressRowHeight,
                Gravity.TOP
            ).apply {
                leftMargin = padding
                rightMargin = padding
                topMargin = progressTop
            }
        }
        updateProgressRowVisibility()

        progressSeekBar = SeekBar(context).apply {
            layoutParams = LinearLayout.LayoutParams(0, dp(32), 1f).apply {
                rightMargin = -dp(8)  // Negative margin to extend into time label space
            }
            max = 1000
            progress = 0
            elevation = dp(4).toFloat()
            // Add right padding to prevent thumb clipping at max position (half of thumb size)
            setPadding(paddingLeft, paddingTop, dp(7), paddingBottom)
            thumb = createThumbDrawable()
            progressDrawable = createProgressDrawable()
        }
        progressSeekBar.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            private var pausedPlaybackForScrub = false
            private var seekProgress = 0

            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                if (fromUser) {
                    seekProgress = progress
                    val durationSeconds = effectiveDurationSeconds() ?: return
                    val positionSeconds = progress.toDouble() / 1000.0 * durationSeconds
                    val remaining = durationSeconds - positionSeconds
                    timeLabel.text = "-" + formatTime((remaining * 1000).toLong().coerceAtLeast(0))
                    // Update UI currentTime during drag to keep progress bar in sync
                    if (isSeeking) {
                        updateProgressUI(positionSeconds, durationSeconds)
                    }
                }
            }

            override fun onStartTrackingTouch(seekBar: SeekBar?) {
                isSeeking = true
                lockedSeekSeconds = null
                // Pause during drag for ALL modes (including stream) to prevent slider jumping
                pausedPlaybackForScrub = player.isPlaying()
                seekProgress = seekBar?.progress ?: 0
                if (pausedPlaybackForScrub) player.pause()
                cancelAutoHide()
            }

            override fun onStopTrackingTouch(seekBar: SeekBar?) {
                val durationSeconds = effectiveDurationSeconds() ?: currentDurationSeconds.takeIf { it > 0 }
                if (durationSeconds != null && durationSeconds > 0) {
                    val positionSeconds = (seekProgress.toDouble() / 1000.0 * durationSeconds)
                    lockedSeekSeconds = positionSeconds
                    isSeeking = true
                    updateProgressUI(positionSeconds, durationSeconds)
                    player.seek(positionSeconds)
                    
                    // Defer play until after seek completes to avoid race condition
                    // where seek and play both trigger stream session creation
                    if (pausedPlaybackForScrub) {
                        mainHandler.postDelayed({ player.play() }, 100)  // Small delay to let seek initiate first
                    }
                } else {
                    isSeeking = false
                }
                scheduleAutoHide()
            }
        })

        timeLabel = TextView(context).apply {
            layoutParams = LinearLayout.LayoutParams(dp(48), dp(32))  // Reduced width
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
            typeface = Typeface.MONOSPACE
            gravity = Gravity.END or Gravity.CENTER_VERTICAL
            setSingleLine(true)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            text = "-0:00"
            setShadowLayer(dp(2).toFloat(), 0f, dp(1).toFloat(), Color.argb(128, 0, 0, 0))
            // Add padding to prevent text from being too close to edge
            setPadding(0, 0, dp(2), 0)
        }

        progressRow.addView(progressSeekBar)
        progressRow.addView(timeLabel)
        bar.addView(progressRow)

        val controlY = dp(50)
        val spacing = dp(12)
        val volumeSliderWidth = dp(80)

        val leftControls = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                buttonWidth
            ).apply {
                gravity = Gravity.START or Gravity.TOP
                leftMargin = padding
                topMargin = controlY
            }
        }

        playPauseButton = ImageButton(context).apply {
            layoutParams = LinearLayout.LayoutParams(buttonWidth, buttonWidth)
            setBackgroundColor(Color.TRANSPARENT)
            setImageResource(R.drawable.icon_play)
            setColorFilter(Color.WHITE)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(8), dp(8), dp(8), dp(8))
            setOnClickListener { onPlayPauseClick() }
        }
        leftControls.addView(playPauseButton)

        volumeButton = ImageButton(context).apply {
            layoutParams = LinearLayout.LayoutParams(buttonWidth, buttonWidth).apply {
                marginStart = spacing
            }
            setBackgroundColor(Color.TRANSPARENT)
            setImageResource(R.drawable.icon_volume_on)
            setColorFilter(Color.WHITE)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(8), dp(8), dp(8), dp(8))
            setOnClickListener { onVolumeClick() }
        }
        leftControls.addView(volumeButton)

        volumeSeekBar = SeekBar(context).apply {
            layoutParams = LinearLayout.LayoutParams(volumeSliderWidth, dp(28)).apply {
                marginStart = dp(4)
                gravity = Gravity.CENTER_VERTICAL
            }
            max = 100
            progress = 100
            // Add horizontal padding to prevent thumb clipping at min/max
            val thumbPadding = dp(6)
            setPadding(thumbPadding, 0, thumbPadding, 0)
            thumb = createVolumeThumbDrawable()
            progressDrawable = createVolumeProgressDrawable()
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                    if (fromUser) onVolumeSliderChanged(progress)
                }
                override fun onStartTrackingTouch(seekBar: SeekBar?) { cancelAutoHide() }
                override fun onStopTrackingTouch(seekBar: SeekBar?) { scheduleAutoHide() }
            })
        }
        leftControls.addView(volumeSeekBar)

        bar.addView(leftControls)

        val rightControls = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                buttonWidth
            ).apply {
                gravity = Gravity.END or Gravity.TOP
                rightMargin = padding
                topMargin = controlY
            }
        }

        settingsButton = ImageButton(context).apply {
            layoutParams = LinearLayout.LayoutParams(buttonWidth, buttonWidth)
            setBackgroundColor(Color.TRANSPARENT)
            setImageResource(R.drawable.icon_settings)
            setColorFilter(Color.WHITE)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            visibility = View.GONE
            setOnClickListener { onSettingsClick() }
        }
        rightControls.addView(settingsButton)

        fullscreenButton = ImageButton(context).apply {
            layoutParams = LinearLayout.LayoutParams(buttonWidth, buttonWidth).apply {
                marginStart = spacing
            }
            setBackgroundColor(Color.TRANSPARENT)
            setImageResource(R.drawable.icon_fullscreen_enter)
            imageTintList = android.content.res.ColorStateList.valueOf(Color.WHITE)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(4), dp(4), dp(4), dp(4))
            setOnClickListener { onFullscreenClick() }
        }
        rightControls.addView(fullscreenButton)

        bar.addView(rightControls)
        return bar
    }

    fun setVisible(visible: Boolean) {
        isEnabled = visible
        if (!visible) setControlsVisible(false)
    }

    fun updatePlayPauseButton() {
        val isPlaying = player.isPlaying()
        playPauseButton.setImageResource(if (isPlaying) R.drawable.icon_pause else R.drawable.icon_play)
        updateFullscreenButton()
        updateSettingsButton()
    }

    fun showCenterPlayButton(show: Boolean) {
        if (show) {
            setControlsVisible(true)
        }
    }

    fun updateProgress(currentTime: Double, duration: Double) {
        // Update cached duration (clear it when duration is 0 for live streams)
        currentDurationSeconds = if (duration > 0) duration else 0.0
        updateProgressRowVisibility()
        updateTimeLabelWidth(duration)
        lockedSeekSeconds?.let { target ->
            if (abs(currentTime - target) > 0.5) return
            lockedSeekSeconds = null
            isSeeking = false
        }
        if (isSeeking) return
        if (duration > 0) {
            val progress = (currentTime / duration * 1000).toInt()
            progressSeekBar.progress = progress
        }
        val remaining = duration - currentTime
        timeLabel.text = "-" + formatTime((remaining * 1000).toLong().coerceAtLeast(0))
    }

    private fun toggleControls() {
        if (!isEnabled) return
        dismissSettingsPopup()
        setControlsVisible(!controlsVisible)
    }

    private fun setControlsVisible(visible: Boolean) {
        controlsVisible = visible
        val targetAlpha = if (visible) 1f else 0f
        val duration = 300L

        topGradient.animate().alpha(targetAlpha).setDuration(duration).start()
        bottomGradient.animate().alpha(targetAlpha).setDuration(duration).start()
        if (showCloseButton) topBar.animate().alpha(targetAlpha).setDuration(duration).start()
        bottomBar.animate().alpha(targetAlpha).setDuration(duration).start()

        // Disable interaction when controls are hidden to prevent accidental clicks
        // on invisible buttons (e.g., fullscreen, progress bar)
        setControlsInteractionEnabled(visible)

        if (visible) {
            scheduleAutoHide()
        } else {
            cancelAutoHide()
        }
    }

    private fun setControlsInteractionEnabled(enabled: Boolean) {
        // Top bar controls
        closeButton.isEnabled = enabled
        closeButton.isClickable = enabled

        // Bottom bar controls
        progressSeekBar.isEnabled = enabled
        playPauseButton.isEnabled = enabled
        playPauseButton.isClickable = enabled
        volumeButton.isEnabled = enabled
        volumeButton.isClickable = enabled
        volumeSeekBar.isEnabled = enabled
        settingsButton.isEnabled = enabled
        settingsButton.isClickable = enabled
        fullscreenButton.isEnabled = enabled
        fullscreenButton.isClickable = enabled
    }

    private fun scheduleAutoHide() {
        cancelAutoHide()
        hideControlsRunnable = Runnable {
            dismissSettingsPopup()
            setControlsVisible(false)
        }
        mainHandler.postDelayed(hideControlsRunnable!!, if (player.isFullscreen()) 5000L else 3000L)
    }

    private fun cancelAutoHide() {
        hideControlsRunnable?.let { mainHandler.removeCallbacks(it) }
        hideControlsRunnable = null
    }

    private fun onPlayPauseClick() {
        if (player.isPlaying()) player.pause() else player.play()
        scheduleAutoHide()
    }

    private fun onFullscreenClick() {
        dismissSettingsPopup()
        if (player.isFullscreen()) player.exitFullscreen() else player.enterFullscreen()
        scheduleAutoHide()
    }

    private fun onCloseClick() {
        dismissSettingsPopup()
        player.requestClose()
        scheduleAutoHide()
    }

    private fun onVolumeClick() {
        val muted = player.isMuted()
        player.setMuted(!muted)
        updateVolumeButton(!muted)
        scheduleAutoHide()
    }

    private fun onVolumeSliderChanged(value: Int) {
        val volume = value / 100.0
        player.setMuted(false)
        player.setVolume(volume)
        updateVolumeButton(false)
    }

    private fun onSettingsClick() {
        if (player.getAvailableQualities().isEmpty() && player.getAvailableSpeeds().isEmpty()) return
        if (view.findViewWithTag<View>(POPUP_MENU_TAG) != null) {
            dismissSettingsPopup()
            return
        }
        showSettingsPopup()
        cancelAutoHide()
    }

    internal fun onFullscreenChanged(isFullscreen: Boolean) {
        dismissSettingsPopup()
        if (showCloseButton) topBar.alpha = 1f
        setControlsVisible(true)
        updateFullscreenButton()
    }

    private fun updateFullscreenButton() {
        fullscreenButton.setImageResource(if (player.isFullscreen()) R.drawable.icon_fullscreen_exit else R.drawable.icon_fullscreen_enter)
        fullscreenButton.imageTintList = android.content.res.ColorStateList.valueOf(Color.WHITE)
        fullscreenButton.visibility = if (showFullscreenButton) View.VISIBLE else View.GONE
    }

    fun setShowProgressBar(show: Boolean) {
        if (showProgressBar == show) return
        showProgressBar = show
        updateProgressRowVisibility()
    }

    private fun updateProgressRowVisibility() {
        if (!::progressRow.isInitialized) return
        // Only show progress bar if explicitly enabled OR in stream mode with valid seekable duration
        // Don't show for live streams (duration == null or infinite)
        val streamHasDuration = player.isStreamDecoderMode() && effectiveDurationSeconds() != null
        val isLiveStream = player.isStreamDecoderMode() && effectiveDurationSeconds() == null
        progressRow.visibility = if ((showProgressBar && !isLiveStream) || streamHasDuration) View.VISIBLE else View.GONE
    }

    fun updateSettingsButton() {
        showSettingsButton = player.getAvailableQualities().isNotEmpty() || player.getAvailableSpeeds().isNotEmpty()
        settingsButton.visibility = if (showSettingsButton) View.VISIBLE else View.GONE
    }

    internal fun updateVolumeButton(isMuted: Boolean) {
        volumeButton.setImageResource(if (isMuted) R.drawable.icon_volume_off else R.drawable.icon_volume_on)
    }

    internal fun updateVolumeState(isMuted: Boolean, volume: Double) {
        updateVolumeButton(isMuted)
        volumeSeekBar.progress = (volume * 100).toInt().coerceIn(0, 100)
    }

    internal fun setShowCloseButton(show: Boolean) {
        showCloseButton = show
        closeButton.visibility = if (show) View.VISIBLE else View.GONE
    }

    internal fun setShowFullscreenButton(show: Boolean) {
        showFullscreenButton = show
        updateFullscreenButton()
    }

    private fun updateProgressUI(currentSeconds: Double, durationSeconds: Double) {
        currentDurationSeconds = durationSeconds
        updateTimeLabelWidth(durationSeconds)
        if (durationSeconds > 0) {
            val progress = (currentSeconds / durationSeconds * 1000).toInt()
            progressSeekBar.progress = progress
        }
        val remaining = durationSeconds - currentSeconds
        timeLabel.text = "-" + formatTime((remaining * 1000).toLong().coerceAtLeast(0))
    }

    private fun updateTimeLabelWidth(durationSeconds: Double) {
        if (!::timeLabel.isInitialized) return
        val wide = durationSeconds >= 3600.0
        if (wide == timeLabelWide) return
        timeLabelWide = wide
        val lp = (timeLabel.layoutParams as? LinearLayout.LayoutParams) ?: return
        lp.width = dp(if (wide) 72 else 48)
        timeLabel.layoutParams = lp
        timeLabel.requestLayout()
    }

    private fun effectiveDurationSeconds(): Double? {
        val durationMs = player.getDuration()
        return when {
            currentDurationSeconds > 0 -> currentDurationSeconds
            durationMs > 0 -> durationMs.toDouble() / 1000.0
            else -> null
        }
    }

    private fun formatTime(ms: Long): String {
        val totalSeconds = (ms / 1000).coerceAtLeast(0)
        val minutes = totalSeconds / 60
        val seconds = totalSeconds % 60
        if (minutes >= 60) {
            val hours = minutes / 60
            val remMinutes = minutes % 60
            return "%d:%02d:%02d".format(hours, remMinutes, seconds)
        }
        return "%d:%02d".format(minutes, seconds)
    }

    private fun formatRate(rate: Double): String =
        if (rate == rate.toLong().toDouble()) "${rate.toLong()}x" else "${rate}x"

    private fun dp(value: Int): Int = TypedValue.applyDimension(
        TypedValue.COMPLEX_UNIT_DIP,
        value.toFloat(),
        context.resources.displayMetrics
    ).toInt()

    private fun createPopupContainer(): FrameLayout = FrameLayout(context).apply {
        tag = POPUP_MENU_TAG
        background = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = dp(13).toFloat()
            setColor(POPUP_BG)
            setStroke(1, POPUP_BORDER)
        }
        elevation = dp(12).toFloat()
    }

    private fun createPopupOverlay(): View = View(context).apply {
        tag = POPUP_OVERLAY_TAG
        layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        setBackgroundColor(Color.argb((0.3 * 255).toInt(), 0, 0, 0))
        setOnClickListener { dismissSettingsPopup() }
        alpha = 0f
    }

    private fun showPopup(menuWidth: Int, content: View) {
        val popup = createPopupContainer().apply { addView(content) }

        popup.measure(
            View.MeasureSpec.makeMeasureSpec(menuWidth, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED)
        )
        val popupHeight = popup.measuredHeight

        val (settingsX, settingsY) = locationInOverlay(settingsButton)

        var popupX = settingsX + settingsButton.width / 2 - menuWidth / 2
        var popupY = settingsY - popupHeight - dp(12)

        val minX = dp(12)
        val maxX = maxOf(minX, view.width - menuWidth - dp(12))
        val minY = dp(12)
        val maxY = maxOf(minY, view.height - popupHeight - dp(12))
        popupX = popupX.coerceIn(minX, maxX)
        popupY = popupY.coerceIn(minY, maxY)

        popup.layoutParams = FrameLayout.LayoutParams(menuWidth, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
            leftMargin = popupX
            topMargin = popupY
            gravity = Gravity.TOP or Gravity.START
        }

        val overlay = createPopupOverlay()
        view.addView(overlay)
        view.addView(popup)

        popup.alpha = 0f
        popup.scaleX = 0.8f
        popup.scaleY = 0.8f
        popup.pivotX = menuWidth / 2f
        popup.pivotY = popupHeight.toFloat()

        popup.animate()
            .alpha(1f)
            .scaleX(1f)
            .scaleY(1f)
            .setDuration(300)
            .setInterpolator(OvershootInterpolator(0.8f))
            .start()

        overlay.animate().alpha(1f).setDuration(200).start()
    }

    private fun showSettingsPopup() {
        val qualities = player.getAvailableQualities()
        val speeds = player.getAvailableSpeeds()
        val currentQuality = player.getCurrentQuality() ?: qualities.firstOrNull()?.label
        val currentSpeed = player.getCurrentSpeed()

        val content = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            setPadding(dp(8), dp(8), dp(8), dp(8))
        }

        // Quality option
        if (qualities.isNotEmpty()) {
            val qualityButton = createMainMenuButton(
                title = context.getString(R.string.lx_video_quality),
                subtitle = currentQuality ?: "Auto"
            ) { showQualitySubmenu() }
            content.addView(qualityButton)
        }

        // Speed option
        if (speeds.isNotEmpty()) {
            val speedButton = createMainMenuButton(
                title = context.getString(R.string.lx_video_speed),
                subtitle = formatRate(currentSpeed)
            ) { showSpeedSubmenu() }
            content.addView(speedButton)
        }

        showPopup(dp(180), content)
    }

    private fun showQualitySubmenu() {
        dismissSettingsPopup()
        val qualities = player.getAvailableQualities()
        val currentQuality = player.getCurrentQuality() ?: qualities.firstOrNull()?.label
        showSubmenu(qualities.map { it.label }, currentQuality) { selected ->
            // Emit quality change event
            player.emitQualityChange(selected)
        }
    }

    private fun showSpeedSubmenu() {
        dismissSettingsPopup()
        val speeds = player.getAvailableSpeeds()
        val currentSpeed = player.getCurrentSpeed()
        showSubmenu(speeds.map { formatRate(it) }, formatRate(currentSpeed)) { selected ->
            val rate = selected.removeSuffix("x").toDoubleOrNull() ?: 1.0
            player.setPlaybackRate(rate)
        }
    }

    private fun showSubmenu(items: List<String>, current: String?, onSelect: (String) -> Unit) {
        val content = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            setPadding(dp(8), dp(8), dp(8), dp(8))
        }

        for (item in items) {
            val isSelected = item == current
            val button = createSubmenuButton(item, isSelected) {
                dismissSettingsPopup()
                onSelect(item)
                scheduleAutoHide()
            }
            content.addView(button)
        }
        showPopup(dp(200), content)
    }

    private fun createMainMenuButton(title: String, subtitle: String, onClick: () -> Unit): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(44)
            )
            setBackgroundColor(Color.TRANSPARENT)
            setPadding(dp(12), 0, dp(12), 0)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick() }

            // Text container
            val textContainer = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }

            val titleView = TextView(context).apply {
                text = title
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            }
            textContainer.addView(titleView)

            val subtitleView = TextView(context).apply {
                text = subtitle
                setTextColor(TEXT_SECONDARY)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            }
            textContainer.addView(subtitleView)

            addView(textContainer)

            // Chevron
            val chevron = TextView(context).apply {
                text = "›"
                setTextColor(Color.argb((0.4 * 255).toInt(), 255, 255, 255))
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
            }
            addView(chevron)
        }
    }

    private fun locationInOverlay(anchor: View): Pair<Int, Int> {
        // Prefer descendant-rect conversion: robust against fullscreen dialog re-parenting,
        // translations, and nested view offsets.
        try {
            val rect = Rect()
            anchor.getDrawingRect(rect)
            view.offsetDescendantRectToMyCoords(anchor, rect)
            return rect.left to rect.top
        } catch (_: Throwable) {
            // Fall back to screen coordinates.
        }

        val anchorLoc = IntArray(2)
        val viewLoc = IntArray(2)
        anchor.getLocationOnScreen(anchorLoc)
        view.getLocationOnScreen(viewLoc)
        return (anchorLoc[0] - viewLoc[0]) to (anchorLoc[1] - viewLoc[1])
    }

    private fun createSubmenuButton(title: String, isSelected: Boolean, onClick: () -> Unit): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(36)
            ).apply { bottomMargin = dp(2) }

            if (isSelected) {
                background = GradientDrawable().apply {
                    cornerRadius = dp(8).toFloat()
                    setColor(ACCENT_BLUE_ALPHA)
                }
            }
            setPadding(dp(12), 0, dp(12), 0)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick() }

            val titleView = TextView(context).apply {
                text = title
                setTextColor(Color.WHITE)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                typeface = if (isSelected) Typeface.create("sans-serif-medium", Typeface.NORMAL) else Typeface.DEFAULT
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
            addView(titleView)

            if (isSelected) {
                val checkmark = TextView(context).apply {
                    text = "✓"
                    setTextColor(ACCENT_BLUE)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                    typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
                }
                addView(checkmark)
            }
        }
    }

    private fun dismissSettingsPopup(immediate: Boolean = false) {
        val overlay = view.findViewWithTag<View>(POPUP_OVERLAY_TAG)
        val popup = view.findViewWithTag<View>(POPUP_MENU_TAG)

        if (overlay == null && popup == null) return

        if (immediate || !view.isAttachedToWindow) {
            if (popup != null) view.removeView(popup)
            if (overlay != null) view.removeView(overlay)
            return
        }

        popup?.animate()
            ?.alpha(0f)
            ?.scaleX(0.9f)
            ?.scaleY(0.9f)
            ?.setDuration(200)
            ?.withEndAction { view.removeView(popup) }
            ?.start()

        overlay?.animate()
            ?.alpha(0f)
            ?.setDuration(200)
            ?.withEndAction { view.removeView(overlay) }
            ?.start()
    }

    private fun createThumbDrawable(): Drawable {
        val size = dp(12)
        return object : ShapeDrawable(OvalShape()) {
            init {
                intrinsicWidth = size
                intrinsicHeight = size
                paint.color = Color.WHITE
                paint.isAntiAlias = true
                paint.setShadowLayer(dp(2).toFloat(), 0f, dp(1).toFloat(), Color.argb(77, 0, 0, 0))
            }

            override fun draw(canvas: Canvas) {
                canvas.save()
                super.draw(canvas)
                canvas.restore()
            }
        }
    }

    private fun createProgressDrawable(): Drawable {
        val trackHeight = dp(3)
        val cornerRadius = dp(2).toFloat()

        val background = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            this.cornerRadius = cornerRadius
            setColor(TRACK_BG)
            setSize(-1, trackHeight)
        }
        val progress = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            this.cornerRadius = cornerRadius
            setColor(ACCENT_BLUE)
            setSize(-1, trackHeight)
        }

        val clipProgress = android.graphics.drawable.ClipDrawable(
            progress,
            Gravity.START,
            android.graphics.drawable.ClipDrawable.HORIZONTAL
        )

        return LayerDrawable(arrayOf(background, clipProgress)).apply {
            setId(0, android.R.id.background)
            setId(1, android.R.id.progress)
            // Center the thin track in the SeekBar touch area
            val inset = dp(12)
            setLayerInset(0, 0, inset, 0, inset)
            setLayerInset(1, 0, inset, 0, inset)
        }
    }

    private fun createVolumeThumbDrawable(): Drawable {
        val thumbSize = dp(12)
        return object : GradientDrawable() {
            init {
                shape = OVAL
                setColor(Color.WHITE)
                setSize(thumbSize, thumbSize)
                // Add subtle shadow effect
                setStroke(dp(1), Color.argb(30, 0, 0, 0))
            }
        }
    }

    private fun createVolumeProgressDrawable(): Drawable {
        val trackHeight = dp(3)  // Slim track
        val cornerRadius = dp(2).toFloat()

        val background = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            this.cornerRadius = cornerRadius
            setColor(Color.argb(80, 255, 255, 255))  // Semi-transparent white
            setSize(-1, trackHeight)
        }
        val progress = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            this.cornerRadius = cornerRadius
            setColor(Color.WHITE)
            setSize(-1, trackHeight)
        }

        val clipProgress = android.graphics.drawable.ClipDrawable(
            progress,
            Gravity.START,
            android.graphics.drawable.ClipDrawable.HORIZONTAL
        )

        return LayerDrawable(arrayOf(background, clipProgress)).apply {
            setId(0, android.R.id.background)
            setId(1, android.R.id.progress)
            // Center the thin track vertically
            val inset = dp(10)
            setLayerInset(0, 0, inset, 0, inset)
            setLayerInset(1, 0, inset, 0, inset)
        }
    }

}
