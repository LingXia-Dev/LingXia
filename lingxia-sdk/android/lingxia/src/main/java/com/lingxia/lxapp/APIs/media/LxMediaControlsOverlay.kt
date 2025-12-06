package com.lingxia.lxapp.APIs.media

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.animation.AnimatorSet
import android.animation.ObjectAnimator
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.graphics.drawable.ShapeDrawable
import android.graphics.drawable.shapes.OvalShape
import android.os.Handler
import android.os.Looper
import android.util.Log
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

    private val mainHandler = Handler(Looper.getMainLooper())
    private var hideControlsRunnable: Runnable? = null
    private var controlsVisible = false
    private var isEnabled = true
    private var showCloseButton = false
    private var showFullscreenButton = true
    private var showSettingsButton = false
    private var isSeeking = false
    private var lockedSeekSeconds: Double? = null

    companion object {
        val ACCENT_BLUE = Color.rgb(0, 122, 255)
        val ACCENT_BLUE_ALPHA = Color.argb(51, 0, 122, 255)
        val TRACK_BG = Color.argb(77, 255, 255, 255)
        val CENTER_BTN_BG = Color.argb(128, 0, 0, 0)
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
    private lateinit var centerPlayButton: ImageButton
    private lateinit var closeButton: ImageButton
    private lateinit var titleLabel: TextView
    private lateinit var progressSeekBar: SeekBar
    private lateinit var timeLabel: TextView
    private lateinit var playPauseButton: ImageButton
    private lateinit var volumeButton: ImageButton
    private lateinit var volumeSeekBar: SeekBar
    private lateinit var settingsButton: ImageButton
    private lateinit var fullscreenButton: ImageButton
    private var currentDurationSeconds: Double = 0.0

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

        centerPlayButton = createCenterPlayButton()
        view.addView(centerPlayButton)
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
            setImageResource(android.R.drawable.ic_menu_close_clear_cancel)
            setColorFilter(Color.WHITE)
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

        val progressRow = LinearLayout(context).apply {
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

        progressSeekBar = SeekBar(context).apply {
            layoutParams = LinearLayout.LayoutParams(0, dp(32), 1f).apply {
                rightMargin = -dp(8)  // Negative margin to extend into time label space
            }
            max = 1000
            progress = 0
            elevation = dp(4).toFloat()
            // Reduce right padding to extend progress bar closer to time label
            setPadding(paddingLeft, paddingTop, 0, paddingBottom)
            thumb = createThumbDrawable()
            progressDrawable = createProgressDrawable()
        }
        progressSeekBar.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            private var wasPlaying = false
            private var seekProgress = 0

            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                if (fromUser) {
                    seekProgress = progress
                    val durationSeconds = effectiveDurationSeconds() ?: return
                    val positionSeconds = progress.toDouble() / 1000.0 * durationSeconds
                    val remaining = durationSeconds - positionSeconds
                    timeLabel.text = "-" + formatTime((remaining * 1000).toLong().coerceAtLeast(0))
                }
            }

            override fun onStartTrackingTouch(seekBar: SeekBar?) {
                isSeeking = true
                lockedSeekSeconds = null
                wasPlaying = player.isPlaying()
                seekProgress = seekBar?.progress ?: 0
                if (wasPlaying) player.pause()
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
                } else {
                    isSeeking = false
                }
                if (wasPlaying) player.play()
                scheduleAutoHide()
            }
        })

        timeLabel = TextView(context).apply {
            layoutParams = LinearLayout.LayoutParams(dp(48), dp(32))  // Reduced width
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
            typeface = Typeface.MONOSPACE
            gravity = Gravity.END or Gravity.CENTER_VERTICAL
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
            setImageDrawable(createPlayIconDrawable(dp(24)))
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
            setImageResource(android.R.drawable.ic_lock_silent_mode_off)
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
            setPadding(0, 0, 0, 0)
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
            layoutParams = LinearLayout.LayoutParams(dp(36), dp(36))
            setBackgroundColor(Color.TRANSPARENT)
            setImageDrawable(createGearIcon())
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
            setImageDrawable(createFullscreenIcon(false))
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(4), dp(4), dp(4), dp(4))
            setOnClickListener { onFullscreenClick() }
        }
        rightControls.addView(fullscreenButton)

        bar.addView(rightControls)
        return bar
    }

    private fun createCenterPlayButton(): ImageButton {
        return ImageButton(context).apply {
            val size = dp(80)
            layoutParams = FrameLayout.LayoutParams(size, size, Gravity.CENTER)
            background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(CENTER_BTN_BG)
            }
            setImageDrawable(createPlayIconDrawable(dp(40)))
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(20), dp(20), dp(20), dp(20))
            setOnClickListener { onPlayPauseClick() }
            visibility = View.GONE
            alpha = 0f
        }
    }

    fun setVisible(visible: Boolean) {
        isEnabled = visible
        if (!visible) setControlsVisible(false)
    }

    fun updatePlayPauseButton() {
        val isPlaying = player.isPlaying()
        val iconSize = dp(24)
        playPauseButton.setImageDrawable(if (isPlaying) createPauseIconDrawable(iconSize) else createPlayIconDrawable(iconSize))
        val centerIconSize = dp(40)
        centerPlayButton.setImageDrawable(if (isPlaying) createPauseIconDrawable(centerIconSize) else createPlayIconDrawable(centerIconSize))
        centerPlayButton.visibility = if (controlsVisible) View.VISIBLE else View.GONE
        updateFullscreenButton()
        updateSettingsButton()
    }

    fun showCenterPlayButton(show: Boolean) {
        centerPlayButton.setImageDrawable(createPlayIconDrawable(dp(40)))
        centerPlayButton.visibility = if (show) View.VISIBLE else View.GONE
        centerPlayButton.alpha = 1f
        if (show) {
            setControlsVisible(true)
        }
    }

    fun updateProgress(currentTime: Double, duration: Double) {
        if (duration > 0) currentDurationSeconds = duration
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

        if (visible) {
            centerPlayButton.visibility = View.VISIBLE
            centerPlayButton.animate().alpha(1f).setDuration(duration).start()
            scheduleAutoHide()
        } else {
            centerPlayButton.animate().alpha(0f).setDuration(duration)
                .withEndAction { centerPlayButton.visibility = View.GONE }.start()
            cancelAutoHide()
        }
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
        fullscreenButton.setImageDrawable(createFullscreenIcon(player.isFullscreen()))
        fullscreenButton.visibility = if (showFullscreenButton) View.VISIBLE else View.GONE
    }

    fun updateSettingsButton() {
        showSettingsButton = player.getAvailableQualities().isNotEmpty() || player.getAvailableSpeeds().isNotEmpty()
        settingsButton.visibility = if (showSettingsButton) View.VISIBLE else View.GONE
    }

    internal fun updateVolumeButton(isMuted: Boolean) {
        volumeButton.setImageResource(if (isMuted) android.R.drawable.ic_lock_silent_mode else android.R.drawable.ic_lock_silent_mode_off)
        volumeButton.setColorFilter(Color.WHITE)
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
        if (durationSeconds > 0) {
            val progress = (currentSeconds / durationSeconds * 1000).toInt()
            progressSeekBar.progress = progress
        }
        val remaining = durationSeconds - currentSeconds
        timeLabel.text = "-" + formatTime((remaining * 1000).toLong().coerceAtLeast(0))
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
        return "%d:%02d".format(minutes, seconds)
    }

    private fun formatRate(rate: Double): String {
        return if (rate == rate.toLong().toDouble()) {
            "${rate.toLong()}x"
        } else {
            "${rate}x"
        }
    }

    private fun dp(value: Int): Int {
        return TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value.toFloat(),
            context.resources.displayMetrics
        ).toInt()
    }

    private fun showSettingsPopup() {
        val qualities = player.getAvailableQualities()
        val speeds = player.getAvailableSpeeds()
        val currentQuality = qualities.firstOrNull()?.label
        val currentSpeed = player.getCurrentSpeed()

        val menuWidth = dp(180)
        var yOffset = dp(8)

        // Create popup container
        val popup = FrameLayout(context).apply {
            tag = POPUP_MENU_TAG
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = dp(13).toFloat()
                setColor(POPUP_BG)
                setStroke(1, POPUP_BORDER)
            }
            elevation = dp(12).toFloat()
        }

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
                title = "Quality",
                subtitle = currentQuality ?: "Auto",
                iconRes = android.R.drawable.ic_menu_gallery
            ) { showQualitySubmenu() }
            content.addView(qualityButton)
        }

        // Speed option
        if (speeds.isNotEmpty()) {
            val speedButton = createMainMenuButton(
                title = "Speed",
                subtitle = formatRate(currentSpeed),
                iconRes = android.R.drawable.ic_menu_manage
            ) { showSpeedSubmenu() }
            content.addView(speedButton)
        }

        popup.addView(content)

        // Measure popup
        popup.measure(
            View.MeasureSpec.makeMeasureSpec(menuWidth, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED)
        )
        val popupHeight = popup.measuredHeight

        // Position near settings button
        val settingsLoc = IntArray(2)
        settingsButton.getLocationOnScreen(settingsLoc)
        val viewLoc = IntArray(2)
        view.getLocationOnScreen(viewLoc)

        val settingsX = settingsLoc[0] - viewLoc[0]
        val settingsY = settingsLoc[1] - viewLoc[1]

        var popupX = settingsX + settingsButton.width / 2 - menuWidth / 2
        var popupY = settingsY - popupHeight - dp(12)

        // Keep within bounds
        val minX = dp(12)
        val maxX = maxOf(minX, view.width - menuWidth - dp(12))
        val minY = dp(12)
        val maxY = maxOf(minY, view.height - popupHeight - dp(12))
        popupX = popupX.coerceIn(minX, maxX)
        popupY = popupY.coerceIn(minY, maxY)

        popup.layoutParams = FrameLayout.LayoutParams(menuWidth, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
            leftMargin = popupX
            topMargin = popupY
        }

        // Add tap-to-dismiss overlay
        val overlay = View(context).apply {
            tag = POPUP_OVERLAY_TAG
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.argb((0.3 * 255).toInt(), 0, 0, 0))
            setOnClickListener { dismissSettingsPopup() }
            alpha = 0f
        }

        view.addView(overlay)
        view.addView(popup)

        // Animate in
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

    private fun showQualitySubmenu() {
        dismissSettingsPopup()
        val qualities = player.getAvailableQualities()
        val currentQuality = qualities.firstOrNull()?.label
        showSubmenu("Quality", qualities.map { it.label }, currentQuality) { selected ->
            Log.d(TAG, "Quality selected: $selected")
            // Emit quality request event
            player.emitQualityRequest(selected)
        }
    }

    private fun showSpeedSubmenu() {
        dismissSettingsPopup()
        val speeds = player.getAvailableSpeeds()
        val currentSpeed = player.getCurrentSpeed()
        showSubmenu("Speed", speeds.map { formatRate(it) }, formatRate(currentSpeed)) { selected ->
            val rate = selected.removeSuffix("x").toDoubleOrNull() ?: 1.0
            Log.d(TAG, "Speed selected: $rate")
            player.setPlaybackRate(rate)
        }
    }

    private fun showSubmenu(title: String, items: List<String>, current: String?, onSelect: (String) -> Unit) {
        val menuWidth = dp(200)

        val popup = FrameLayout(context).apply {
            tag = POPUP_MENU_TAG
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = dp(13).toFloat()
                setColor(POPUP_BG)
                setStroke(1, POPUP_BORDER)
            }
            elevation = dp(12).toFloat()
        }

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

        popup.addView(content)

        // Measure and position
        popup.measure(
            View.MeasureSpec.makeMeasureSpec(menuWidth, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED)
        )
        val popupHeight = popup.measuredHeight

        val settingsLoc = IntArray(2)
        settingsButton.getLocationOnScreen(settingsLoc)
        val viewLoc = IntArray(2)
        view.getLocationOnScreen(viewLoc)

        val settingsX = settingsLoc[0] - viewLoc[0]
        val settingsY = settingsLoc[1] - viewLoc[1]

        var popupX = settingsX + settingsButton.width / 2 - menuWidth / 2
        var popupY = settingsY - popupHeight - dp(12)

        // Keep within bounds
        val minX = dp(12)
        val maxX = maxOf(minX, view.width - menuWidth - dp(12))
        val minY = dp(12)
        val maxY = maxOf(minY, view.height - popupHeight - dp(12))
        popupX = popupX.coerceIn(minX, maxX)
        popupY = popupY.coerceIn(minY, maxY)

        popup.layoutParams = FrameLayout.LayoutParams(menuWidth, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
            leftMargin = popupX
            topMargin = popupY
        }

        val overlay = View(context).apply {
            tag = POPUP_OVERLAY_TAG
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.argb((0.3 * 255).toInt(), 0, 0, 0))
            setOnClickListener { dismissSettingsPopup() }
            alpha = 0f
        }

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

    private fun createMainMenuButton(title: String, subtitle: String, iconRes: Int, onClick: () -> Unit): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(44)
            )
            background = GradientDrawable().apply {
                cornerRadius = dp(10).toFloat()
                setColor(Color.argb((0.08 * 255).toInt(), 255, 255, 255))
            }
            setPadding(dp(12), 0, dp(12), 0)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick() }

            // Icon
            val icon = ImageView(context).apply {
                layoutParams = LinearLayout.LayoutParams(dp(20), dp(20))
                setImageResource(iconRes)
                setColorFilter(Color.WHITE)
            }
            addView(icon)

            // Text container
            val textContainer = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
                    marginStart = dp(8)
                }
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

    private fun dismissSettingsPopup() {
        val overlay = view.findViewWithTag<View>(POPUP_OVERLAY_TAG)
        val popup = view.findViewWithTag<View>(POPUP_MENU_TAG)

        if (overlay == null && popup == null) return

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

    // Custom gear icon for settings
    private fun createGearIcon(): Drawable {
        val size = dp(24)
        return object : Drawable() {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.WHITE
                style = Paint.Style.STROKE
                strokeWidth = dp(2).toFloat()
                strokeCap = Paint.Cap.ROUND
            }

            override fun draw(canvas: Canvas) {
                val cx = bounds.exactCenterX()
                val cy = bounds.exactCenterY()
                val outerR = minOf(bounds.width(), bounds.height()) / 2f - dp(2)
                val innerR = outerR * 0.55f
                val teethCount = 8

                // Draw center circle
                canvas.drawCircle(cx, cy, innerR * 0.5f, paint)

                // Draw gear teeth
                val path = android.graphics.Path()
                for (i in 0 until teethCount) {
                    val angle = (i * 360f / teethCount - 90) * Math.PI.toFloat() / 180f
                    val nextAngle = ((i + 0.5f) * 360f / teethCount - 90) * Math.PI.toFloat() / 180f

                    val x1 = cx + innerR * kotlin.math.cos(angle)
                    val y1 = cy + innerR * kotlin.math.sin(angle)
                    val x2 = cx + outerR * kotlin.math.cos(angle)
                    val y2 = cy + outerR * kotlin.math.sin(angle)
                    val x3 = cx + outerR * kotlin.math.cos(nextAngle)
                    val y3 = cy + outerR * kotlin.math.sin(nextAngle)
                    val x4 = cx + innerR * kotlin.math.cos(nextAngle)
                    val y4 = cy + innerR * kotlin.math.sin(nextAngle)

                    if (i == 0) path.moveTo(x1, y1)
                    path.lineTo(x2, y2)
                    path.lineTo(x3, y3)
                    path.lineTo(x4, y4)
                }
                path.close()
                canvas.drawPath(path, paint)
            }

            override fun setAlpha(alpha: Int) { paint.alpha = alpha }
            override fun setColorFilter(cf: android.graphics.ColorFilter?) { paint.colorFilter = cf }
            override fun getOpacity() = android.graphics.PixelFormat.TRANSLUCENT
            override fun getIntrinsicWidth() = size
            override fun getIntrinsicHeight() = size
        }
    }

    // Custom fullscreen icons
    private fun createFullscreenIcon(isFullscreen: Boolean): Drawable {
        val size = dp(24)
        return object : Drawable() {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.WHITE
                style = Paint.Style.STROKE
                strokeWidth = dp(2).toFloat()
                strokeCap = Paint.Cap.ROUND
                strokeJoin = Paint.Join.ROUND
            }

            override fun draw(canvas: Canvas) {
                val l = bounds.left.toFloat() + dp(4)
                val t = bounds.top.toFloat() + dp(4)
                val r = bounds.right.toFloat() - dp(4)
                val b = bounds.bottom.toFloat() - dp(4)
                val cornerLen = (r - l) * 0.35f

                if (isFullscreen) {
                    // Exit fullscreen: arrows pointing inward (corners with inward arrows)
                    // Top-left corner pointing inward
                    canvas.drawLine(l, t + cornerLen, l, t, paint)
                    canvas.drawLine(l, t, l + cornerLen, t, paint)
                    // Top-right corner pointing inward
                    canvas.drawLine(r - cornerLen, t, r, t, paint)
                    canvas.drawLine(r, t, r, t + cornerLen, paint)
                    // Bottom-left corner pointing inward
                    canvas.drawLine(l, b - cornerLen, l, b, paint)
                    canvas.drawLine(l, b, l + cornerLen, b, paint)
                    // Bottom-right corner pointing inward
                    canvas.drawLine(r - cornerLen, b, r, b, paint)
                    canvas.drawLine(r, b, r, b - cornerLen, paint)
                } else {
                    // Enter fullscreen: expand corners outward
                    // Top-left
                    canvas.drawLine(l, t + cornerLen, l, t, paint)
                    canvas.drawLine(l, t, l + cornerLen, t, paint)
                    // Top-right
                    canvas.drawLine(r - cornerLen, t, r, t, paint)
                    canvas.drawLine(r, t, r, t + cornerLen, paint)
                    // Bottom-left
                    canvas.drawLine(l, b - cornerLen, l, b, paint)
                    canvas.drawLine(l, b, l + cornerLen, b, paint)
                    // Bottom-right
                    canvas.drawLine(r - cornerLen, b, r, b, paint)
                    canvas.drawLine(r, b, r, b - cornerLen, paint)
                }
            }

            override fun setAlpha(alpha: Int) { paint.alpha = alpha }
            override fun setColorFilter(cf: android.graphics.ColorFilter?) { paint.colorFilter = cf }
            override fun getOpacity() = android.graphics.PixelFormat.TRANSLUCENT
            override fun getIntrinsicWidth() = size
            override fun getIntrinsicHeight() = size
        }
    }

    // Create a play icon drawable (triangle pointing right)
    private fun createPlayIconDrawable(size: Int, color: Int = Color.WHITE): Drawable {
        return object : Drawable() {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                this.color = color
                style = Paint.Style.FILL
            }

            override fun draw(canvas: Canvas) {
                val b = bounds
                val w = b.width().toFloat()
                val h = b.height().toFloat()
                // Draw triangle pointing right
                val path = android.graphics.Path().apply {
                    moveTo(w * 0.25f, h * 0.15f)
                    lineTo(w * 0.25f, h * 0.85f)
                    lineTo(w * 0.8f, h * 0.5f)
                    close()
                }
                canvas.drawPath(path, paint)
            }

            override fun setAlpha(alpha: Int) { paint.alpha = alpha }
            override fun setColorFilter(cf: android.graphics.ColorFilter?) { paint.colorFilter = cf }
            override fun getOpacity() = android.graphics.PixelFormat.TRANSLUCENT
            override fun getIntrinsicWidth() = size
            override fun getIntrinsicHeight() = size
        }
    }

    // Create a pause icon drawable (two vertical bars)
    private fun createPauseIconDrawable(size: Int, color: Int = Color.WHITE): Drawable {
        return object : Drawable() {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                this.color = color
                style = Paint.Style.FILL
            }

            override fun draw(canvas: Canvas) {
                val b = bounds
                val w = b.width().toFloat()
                val h = b.height().toFloat()
                val barWidth = w * 0.2f
                val gap = w * 0.15f
                val left1 = (w - 2 * barWidth - gap) / 2
                val left2 = left1 + barWidth + gap
                val top = h * 0.2f
                val bottom = h * 0.8f
                canvas.drawRoundRect(left1, top, left1 + barWidth, bottom, barWidth / 3, barWidth / 3, paint)
                canvas.drawRoundRect(left2, top, left2 + barWidth, bottom, barWidth / 3, barWidth / 3, paint)
            }

            override fun setAlpha(alpha: Int) { paint.alpha = alpha }
            override fun setColorFilter(cf: android.graphics.ColorFilter?) { paint.colorFilter = cf }
            override fun getOpacity() = android.graphics.PixelFormat.TRANSLUCENT
            override fun getIntrinsicWidth() = size
            override fun getIntrinsicHeight() = size
        }
    }
}
