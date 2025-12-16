package com.lingxia.lxapp.APIs.media

import android.Manifest
import android.content.Context
import android.animation.ValueAnimator
import android.graphics.Color
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.graphics.Typeface
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.MotionEvent
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.video.FileOutputOptions
import androidx.camera.video.Quality
import androidx.camera.video.QualitySelector
import androidx.camera.video.Recorder
import androidx.camera.video.Recording
import androidx.camera.video.VideoCapture
import androidx.camera.video.VideoRecordEvent
import androidx.camera.view.PreviewView
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.core.content.ContextCompat
import com.lingxia.lxapp.util.ActivityInsets
import androidx.fragment.app.Fragment
import com.lingxia.lxapp.NativeApi
import com.lingxia.lxapp.R
import android.text.SpannableString
import android.text.style.ForegroundColorSpan
import java.io.File
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.concurrent.Executor
import org.json.JSONObject
import kotlin.math.max
import kotlin.math.min

class MediaCaptureFragment : Fragment() {
    companion object {
        private const val TAG = "LingXia.MediaCapture"
        private const val ARG_MODE = "mode" // image|video
        private const val ARG_CALLBACK_ID = "callback_id"
        private const val ARG_MAX_DURATION = "max_duration"
        private const val ARG_CAMERA_FACING = "camera_facing"

        fun start(
            activity: AppCompatActivity,
            mode: String,
            maxDuration: Int,
            callbackId: Long,
            cameraFacing: Int
        ) {
            val frag = MediaCaptureFragment().apply {
                arguments = Bundle().apply {
                    putString(ARG_MODE, mode)
                    putInt(ARG_MAX_DURATION, maxDuration)
                    putLong(ARG_CALLBACK_ID, callbackId)
                    putInt(ARG_CAMERA_FACING, cameraFacing)
                }
            }
            val fm = activity.supportFragmentManager
            fm.beginTransaction()
                .add(android.R.id.content, frag, TAG)
                .commitAllowingStateLoss()
            fm.executePendingTransactions()
        }
    }

    private val callbackId: Long get() = arguments?.getLong(ARG_CALLBACK_ID) ?: 0L
    private val mode: String get() = arguments?.getString(ARG_MODE) ?: "image"
    private val maxDurationSeconds: Int get() = arguments?.getInt(ARG_MAX_DURATION) ?: -1
    private val initialCameraFacing: Int get() = arguments?.getInt(ARG_CAMERA_FACING) ?: -1

    private var currentLensFacing = if (initialCameraFacing == 0) {
        CameraSelector.LENS_FACING_FRONT
    } else {
        CameraSelector.LENS_FACING_BACK
    }

    private var previewView: PreviewView? = null
    private var captureButton: ShutterButtonView? = null
    private var hintText: TextView? = null
    private var switchCameraButton: CameraSwitchButton? = null
    private var flashButton: FlashButton? = null
    private var backButton: CutoutChevronButton? = null
    private var flashMode: Int = ImageCapture.FLASH_MODE_OFF
    private var torchEnabled: Boolean = false
    private var finishButton: TextView? = null
    private var finishButtonBackground: GradientDrawable? = null
    private var timerText: TextView? = null
    private data class PendingCapture(val file: File, val fileType: String)
    private var pendingCapture: PendingCapture? = null
    private var previewContainer: FrameLayout? = null
    private var previewBackButton: RoundBackArrowButton? = null
    private var previewPlayer: ExoPlayer? = null
    private var previewPlayerView: PlayerView? = null
    private var camera: Camera? = null

    private fun isVideoMode(): Boolean {
        val value = mode.lowercase(Locale.ROOT)
        return value == "video" || value == "videos" || value == "mix"
    }

    private var cameraProvider: ProcessCameraProvider? = null
    private var imageCapture: ImageCapture? = null
    private var videoCapture: VideoCapture<Recorder>? = null
    private var activeRecording: Recording? = null
    private var isRecording = false
    private var timerUpdater: Runnable? = null
    private var recordingStartElapsedMs: Long = -1L

    private lateinit var mainExecutor: Executor

    private val handler = Handler(Looper.getMainLooper())
    private var maxDurationRunnable: Runnable? = null
    private var pendingLongPressStart: Runnable? = null
    private val longPressMs: Long = 280
    private val minRecordingDurationMs: Long = 1000 // Minimum recording duration: 1 second
    private var showingErrorHint: Boolean = false
    private var ignoreRecordingFinalize: Boolean = false

    private val dateFormatter by lazy {
        SimpleDateFormat("yyyyMMdd_HHmmss", Locale.US)
    }

    private val techBlueColor: Int = Color.parseColor("#1677FF")
    private val lightTechBlueColor: Int = Color.parseColor("#AFCBFF")

    // Modern permission launcher (replaces deprecated requestPermissions API)
    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { grants ->
        if (grants.values.all { it }) {
            startCamera()
        } else {
            cancelCapture("Camera permission denied")
        }
    }

    override fun onAttach(context: Context) {
        super.onAttach(context)
        mainExecutor = ContextCompat.getMainExecutor(context)
    }

    override fun onCreateView(
        inflater: android.view.LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val context = requireContext()
        val root = FrameLayout(context).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        // Add gradient overlay for better UI contrast
        val gradientOverlay = View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(
                    Color.parseColor("#40000000"),
                    Color.TRANSPARENT,
                    Color.TRANSPARENT,
                    Color.parseColor("#60000000")
                )
            )
        }

        val preview = PreviewView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            scaleType = PreviewView.ScaleType.FILL_CENTER
        }
        previewView = preview
        root.addView(preview)
        root.addView(gradientOverlay)

        val switchButton = CameraSwitchButton(context).apply {
            contentDescription = "Switch camera"
            setOnClickListener { toggleCamera() }
        }
        switchCameraButton = switchButton
        root.addView(
            switchButton,
            FrameLayout.LayoutParams(dp(context, 44f), dp(context, 44f)).apply {
                gravity = Gravity.TOP or Gravity.END
                val inset = dp(context, 16f)
                setMargins(inset, inset + statusBarHeight(), inset, 0)
            }
        )

        val timerLabel = TextView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
                topMargin = statusBarHeight() + dp(context, 38f)
            }
            setTextColor(Color.WHITE)
            textSize = 16f  // Larger text
            typeface = Typeface.MONOSPACE
            setPadding(dp(context, 16f), dp(context, 8f), dp(context, 16f), dp(context, 8f))  // Larger padding
            background = GradientDrawable().apply {
                cornerRadius = dp(context, 16f).toFloat()  // Larger corner radius
                setColor(Color.parseColor("#80000000"))  // More opaque background
            }
            text = "00:00"
            visibility = View.GONE
        }
        timerText = timerLabel
        root.addView(timerLabel)
        timerLabel.bringToFront()

        // Lift capture controls above nav bar (3-button) while staying flush on gesture nav.
        val navInset = ActivityInsets.contentBottomInset()
        val bottomOverlay = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM
            )
        }
        root.addView(bottomOverlay)

        // Preview container for post-capture playback
        previewContainer = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            visibility = View.GONE
            setBackgroundColor(Color.TRANSPARENT)
        }
        root.addView(previewContainer)

        captureButton = ShutterButtonView(context).also { button ->
            button.isEnabled = false
            button.setOnClickListener { onCapturePressed() }
            button.setOnTouchListener { _, event -> handleCaptureTouch(event) }
            val captureSize = button.diameterPx
            val shutterBottomMargin = navInset + dp(context, 20f)
            val shutterParams = FrameLayout.LayoutParams(
                captureSize,
                captureSize,
                Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            ).apply {
                bottomMargin = shutterBottomMargin
            }
            bottomOverlay.addView(button, shutterParams)

            val hintView = TextView(context).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.WRAP_CONTENT,
                    FrameLayout.LayoutParams.WRAP_CONTENT,
                    Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                ).apply {
                    bottomMargin = shutterBottomMargin + captureSize + dp(context, 45f)
                }
                setTextColor(Color.parseColor("#CCFFFFFF"))
                textSize = 14f
                gravity = Gravity.CENTER
            }
            hintText = hintView
            bottomOverlay.addView(hintView)

            val backBtn = CutoutChevronButton(context).apply {
                contentDescription = context.getString(R.string.lx_common_cancel)
                visibility = View.VISIBLE
                setOnClickListener { handleBackButton() }
            }
            val backParams = FrameLayout.LayoutParams(dp(context, 32f), dp(context, 32f)).apply {
                gravity = Gravity.START or Gravity.BOTTOM
                // Calculate center position between screen edge and big circle left edge
                val screenEdge = dp(context, 20f)
                val screenWidth = context.resources.displayMetrics.widthPixels
                val bigCircleLeftEdge = (screenWidth / 2) - (captureSize / 2)  // Big circle left edge
                val availableSpace = bigCircleLeftEdge - screenEdge
                leftMargin = screenEdge + (availableSpace / 2) - dp(context, 16f)  // Center in available space
                bottomMargin = shutterBottomMargin + (captureSize - dp(context, 32f)) / 2
            }
            bottomOverlay.addView(backBtn, backParams)  // Back to bottomOverlay
            backButton = backBtn

            // Flash button on the right side of capture button
            val flashBtn = FlashButton(context).apply {
                contentDescription = "Toggle flash"
                setOnClickListener { toggleFlash() }
            }
            val flashIconSize = dp(context, 32f)  // Match HarmonyOS ICON_BUTTON_SIZE
            val flashParams = FrameLayout.LayoutParams(flashIconSize, flashIconSize).apply {
                gravity = Gravity.END or Gravity.BOTTOM
                // Mirror position of back button on the right side
                val screenEdge = dp(context, 20f)
                val screenWidth = context.resources.displayMetrics.widthPixels
                val bigCircleRightEdge = (screenWidth / 2) + (captureSize / 2)
                val availableSpace = screenWidth - screenEdge - bigCircleRightEdge
                rightMargin = screenEdge + (availableSpace / 2) - (flashIconSize / 2)
                bottomMargin = shutterBottomMargin + (captureSize - flashIconSize) / 2
            }
            bottomOverlay.addView(flashBtn, flashParams)
            flashButton = flashBtn

            val doneButton = TextView(context).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.WRAP_CONTENT,
                    FrameLayout.LayoutParams.WRAP_CONTENT,
                    Gravity.BOTTOM or Gravity.END
                ).apply {
                    rightMargin = dp(context, 20f)
                    bottomMargin = shutterBottomMargin + dp(context, 6f)
                }
                setPadding(dp(context, 16f), dp(context, 6f), dp(context, 16f), dp(context, 6f))
                val backgroundDrawable = GradientDrawable().apply {
                    cornerRadius = dp(context, 18f).toFloat()
                    setColor(lightTechBlueColor)
                }
                background = backgroundDrawable
                finishButtonBackground = backgroundDrawable
                setTextColor(Color.WHITE)
                text = context.getString(R.string.lx_common_done)
                textSize = 14f
                visibility = View.GONE
                setOnClickListener { completeCapture() }
            }
            finishButton = doneButton
            updateFinishButtonEnabled(false)
            bottomOverlay.addView(doneButton)
        }

        updateHint()
        resetToIdle()
        return root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        requestPermissionsAndStart()
    }

    override fun onDestroyView() {
        super.onDestroyView()
        releaseCamera()
        previewView = null
        captureButton = null
        hintText = null
        switchCameraButton = null
        flashButton = null
        backButton = null
        finishButton = null
        finishButtonBackground = null
        timerText = null
        pendingCapture = null
    }

    private fun requestPermissionsAndStart() {
        val context = context ?: return
        val needed = mutableListOf<String>()
        if (ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.CAMERA
            ) != android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            needed.add(Manifest.permission.CAMERA)
        }
        if (isVideoMode() && ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.RECORD_AUDIO
            ) != android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            needed.add(Manifest.permission.RECORD_AUDIO)
        }
        if (needed.isEmpty()) {
            startCamera()
        } else {
            permissionLauncher.launch(needed.toTypedArray())
        }
    }

    private fun startCamera() {
        val context = context ?: return
        val previewView = previewView ?: return
        captureButton?.isEnabled = false
        timerText?.visibility = View.GONE
        finishButton?.visibility = View.GONE
        updateFinishButtonEnabled(false)
        switchCameraButton?.visibility = View.VISIBLE
        val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
        cameraProviderFuture.addListener({
            try {
                val provider = cameraProviderFuture.get()
                cameraProvider = provider
                bindUseCases(provider, previewView)
                resetToIdle()
            } catch (e: Exception) {
                Log.e(TAG, "startCamera: failed to bind use cases", e)
                cancelCapture(e.message ?: "Unable to start camera preview")
            }
        }, mainExecutor)
    }

    private fun bindUseCases(provider: ProcessCameraProvider, previewView: PreviewView) {
        val selector = CameraSelector.Builder()
            .requireLensFacing(currentLensFacing)
            .build()

        val preview = Preview.Builder().build().apply {
            setSurfaceProvider(previewView.surfaceProvider)
        }

        val useCases = mutableListOf<androidx.camera.core.UseCase>(preview)

        if (isVideoMode()) {
            val recorder = Recorder.Builder()
                .setExecutor(mainExecutor)
                .setQualitySelector(QualitySelector.from(Quality.FHD))
                .build()
            val video = VideoCapture.withOutput(recorder)
            videoCapture = video
            imageCapture = null
            useCases.add(video)
        } else {
            val image = ImageCapture.Builder()
                .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
                .setTargetRotation(previewView.display?.rotation ?: android.view.Surface.ROTATION_0)
                .build()
            image.flashMode = flashMode
            imageCapture = image
            videoCapture = null
            useCases.add(image)
        }

        provider.unbindAll()
        camera = provider.bindToLifecycle(this, selector, *useCases.toTypedArray())
        if (isVideoMode()) {
            camera?.cameraControl?.enableTorch(torchEnabled)
        } else {
            torchEnabled = false
            camera?.cameraControl?.enableTorch(false)
            imageCapture?.flashMode = flashMode
        }
        flashButton?.updateFlashState()
        captureButton?.apply {
            resetState()
            setVideoMode(isVideoMode())
            isEnabled = true
        }
    }

    private fun onCapturePressed() {
        if (pendingCapture != null) return
        if (isVideoMode()) {
            // Video recording handled via touch events (press and hold)
            return
        }
        // Image mode - direct click to capture
        capturePhoto()
    }

    private fun handleCaptureTouch(event: MotionEvent): Boolean {
        if (pendingCapture != null || captureButton?.isEnabled != true) return false
        val video = isVideoMode()
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // Visual feedback for both modes
                captureButton?.pressVisual(true)
                if (video) {
                    pendingLongPressStart?.let { handler.removeCallbacks(it) }
                    val task = Runnable { if (!isRecording && pendingCapture == null) startRecording() }
                    pendingLongPressStart = task
                    handler.postDelayed(task, longPressMs)
                } else {
                    // Photo: trigger immediately on DOWN for snappier feel
                    onCapturePressed()
                }
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                pendingLongPressStart?.let { handler.removeCallbacks(it) }
                pendingLongPressStart = null
                if (video) {
                    // Stop as soon as a recording session exists, even if START event hasn't arrived yet
                    if (activeRecording != null) stopRecording() else captureButton?.pressVisual(false)
                } else {
                    // If photo capture already navigated to preview, the control is hidden; otherwise, restore
                    captureButton?.pressVisual(false)
                }
                return true
            }
        }
        return false
    }

    private fun capturePhoto() {
        val imageCapture = imageCapture
        if (imageCapture == null) {
            cancelCapture("Camera not ready")
            return
        }
        captureButton?.isEnabled = false

        val file = createOutputFile(".jpg")
        val outputOptions = ImageCapture.OutputFileOptions.Builder(file).build()
        imageCapture.takePicture(outputOptions, mainExecutor, object : ImageCapture.OnImageSavedCallback {
            override fun onImageSaved(outputFileResults: ImageCapture.OutputFileResults) {
                onCaptureSuccess(file, "image")
            }

            override fun onError(exception: ImageCaptureException) {
                Log.e(TAG, "capturePhoto: failed", exception)
                cancelCapture(exception.message ?: "Failed to capture photo")
            }
        })
    }

    private fun startRecording() {
        val videoCapture = videoCapture
        if (videoCapture == null) {
            cancelCapture("Video capture not available")
            return
        }
        val file = createOutputFile(".mp4")

        val recording = videoCapture.output
            .prepareRecording(
                requireContext(),
                FileOutputOptions.Builder(file).build()
            )
            .apply {
                if (ContextCompat.checkSelfPermission(
                        requireContext(),
                        Manifest.permission.RECORD_AUDIO
                    ) == android.content.pm.PackageManager.PERMISSION_GRANTED
                ) {
                    withAudioEnabled()
                }
            }
            .start(mainExecutor) { event ->
                when (event) {
                    is VideoRecordEvent.Start -> {
                        // If the session was already stopped before START (quick tap), ignore UI changes
                        if (activeRecording == null) return@start
                        isRecording = true
                        recordingStartElapsedMs = SystemClock.elapsedRealtime() // Record actual start time
                        captureButton?.setRecording(true)
                        captureButton?.setProgress(0f)
                        updateHint()
                        startMaxDurationCountdown()
                        // Keep big shutter + ring visible while recording; hide small bottom cutout
                        backButton?.visibility = View.GONE
                        hintText?.visibility = View.INVISIBLE
                        timerText?.visibility = View.VISIBLE
                        // Red dot + time
                        val initialDisplaySeconds = if (maxDurationSeconds > 0) {
                            maxDurationSeconds
                        } else {
                            0
                        }
                        timerText?.text = formatTimerDisplay(initialDisplaySeconds)
                        startTimerTicker()
                    }

                    is VideoRecordEvent.Finalize -> {
                        // Handle short duration recording specially
                        if (ignoreRecordingFinalize) {
                            ignoreRecordingFinalize = false

                            // First ensure recording state is cleared
                            isRecording = false
                            captureButton?.setRecording(false)
                            captureButton?.setProgress(0f)
                            timerText?.visibility = View.GONE
                            timerText?.text = "00:00"
                            stopMaxDurationCountdown()
                            stopTimerTicker()

                            // Then reset to initial state (but don't overwrite error hint)
                            resetToIdle()
                            // Show error hint after a short delay to ensure it's not overwritten
                            handler.postDelayed({
                                showShortDurationHint()
                            }, 50)
                            return@start
                        }

                        stopMaxDurationCountdown()
                        stopTimerTicker()
                        isRecording = false
                        captureButton?.setRecording(false)
                        captureButton?.setProgress(0f)
                        updateHint()
                        timerText?.visibility = View.GONE
                        activeRecording = null
                        if (event.hasError()) {
                            val errorMsg = event.cause?.message ?: "Video capture error"
                            Log.e(TAG, "startRecording: finalize error ${event.error}", event.cause)
                            cancelCapture(errorMsg)
                        } else {
                            onCaptureSuccess(file, "video")
                        }
                    }

                    is VideoRecordEvent.Status -> {
                        // Keep using Status for progress computation; timer UI is updated by ticker
                        val maxDuration = maxDurationSeconds
                        if (maxDuration > 0) {
                            val durationSeconds = event.recordingStats.recordedDurationNanos / 1_000_000_000.0
                            val progress = (durationSeconds / maxDuration).coerceIn(0.0, 1.0)
                            captureButton?.setProgress(progress.toFloat())
                        }
                    }
                }
            }

        activeRecording = recording
    }

    private fun stopRecording() {
        // Check minimum recording duration BEFORE stopping timers
        val currentElapsed = if (recordingStartElapsedMs > 0) {
            SystemClock.elapsedRealtime() - recordingStartElapsedMs
        } else {
            0L
        }

        if (currentElapsed < minRecordingDurationMs && activeRecording != null) {
            // Recording too short - mark to ignore finalize events and let recording complete normally
            // But we'll discard the result and not proceed to preview
            ignoreRecordingFinalize = true

            // Stop normally but we'll handle the result differently in finalize
            activeRecording?.stop()
            activeRecording = null
            // Clean up timers after checking
            stopMaxDurationCountdown()
            stopTimerTicker()
            return
        }

        // Normal stop - clean up timers
        stopMaxDurationCountdown()
        stopTimerTicker()

        activeRecording?.stop()
        activeRecording = null
    }

    private fun startMaxDurationCountdown() {
        val durationSeconds = maxDurationSeconds
        if (durationSeconds <= 0) return
        stopMaxDurationCountdown()
        val runnable = Runnable {
            stopRecording()
        }
        maxDurationRunnable = runnable
        handler.postDelayed(runnable, durationSeconds * 1000L)
    }

    private fun stopMaxDurationCountdown() {
        maxDurationRunnable?.let { handler.removeCallbacks(it) }
        maxDurationRunnable = null
        pendingLongPressStart?.let { handler.removeCallbacks(it) }
        pendingLongPressStart = null
    }

    private fun onCaptureSuccess(file: File, fileType: String) {
        captureButton?.resetState()
        // Don't disable, just hide the button to avoid alpha change
        captureButton?.visibility = View.INVISIBLE
        switchCameraButton?.visibility = View.GONE
        timerText?.visibility = View.GONE
        timerText?.text = "00:00"
        enterPreviewState(PendingCapture(file, fileType))
    }

    private fun cancelCapture(message: String, isCancel: Boolean = false) {
        captureButton?.resetState()
        stopTimerTicker()
        timerText?.visibility = View.GONE
        timerText?.text = "00:00"
        pendingCapture?.file?.takeIf { it.exists() }?.delete()
        pendingCapture = null
        finishButton?.visibility = View.GONE
        updateFinishButtonEnabled(false)
        val errorPayload = JSONObject().apply {
            put("error", message)
            if (isCancel) put("cancel", true)
            if (message == "Camera permission denied") {
                put("code", "camera_permission_denied")
            }
        }.toString()
        NativeApi.onCallback(callbackId, false, errorPayload)
        NativeApi.onCallback(
            callbackId,
            true,
            JSONObject().apply { put("done", true) }.toString()
        )
        removeSelf()
    }

    private fun completeCapture() {
        val pending = pendingCapture ?: return
        updateFinishButtonEnabled(false)
        try {
            if (!pending.file.exists()) {
                updateFinishButtonEnabled(true)
                cancelCapture("Captured file missing")
                return
            }
            // Return JS array with single item (uri + fileType), no fd
            val arr = org.json.JSONArray().apply {
                put(JSONObject().apply {
                    put("uri", pending.file.absolutePath)
                    put("fileType", pending.fileType)
                })
            }
            NativeApi.onCallback(callbackId, true, arr.toString())
            NativeApi.onCallback(
                callbackId,
                true,
                JSONObject().apply { put("done", true) }.toString()
            )
            pendingCapture = null
            finishButton?.visibility = View.GONE
            removeSelf()
        } catch (e: Exception) {
            Log.e(TAG, "completeCapture: failed", e)
            updateFinishButtonEnabled(true)
            cancelCapture(e.message ?: "Failed to complete capture")
        }
    }

    private fun handleBackButton() {
        val pending = pendingCapture
        if (pending != null) {
            if (pending.file.exists()) pending.file.delete()
            pendingCapture = null
            resetToIdle()
            return
        }
        // User cancelled - return empty result instead of error to avoid toast in JS
        captureButton?.resetState()
        stopTimerTicker()
        timerText?.visibility = View.GONE
        timerText?.text = "00:00"
        finishButton?.visibility = View.GONE
        updateFinishButtonEnabled(false)
        // Return empty array as success (no media selected)
        NativeApi.onCallback(callbackId, true, "[]")
        NativeApi.onCallback(
            callbackId,
            true,
            JSONObject().apply { put("done", true) }.toString()
        )
        removeSelf()
    }

    private fun toggleCamera() {
        currentLensFacing = if (currentLensFacing == CameraSelector.LENS_FACING_FRONT) {
            CameraSelector.LENS_FACING_BACK
        } else {
            CameraSelector.LENS_FACING_FRONT
        }
        startCamera()
    }

    private fun updateHint() {
        // Skip update if showing error hint
        if (showingErrorHint) return

        val isVideoMode = isVideoMode()
        captureButton?.setVideoMode(isVideoMode)
        hintText?.text = when {
            isVideoMode && isRecording -> getString(R.string.lx_camera_release_to_stop)
            isVideoMode -> getString(R.string.lx_camera_long_press_to_record)
            else -> getString(R.string.lx_camera_tap_to_capture)
        }
    }

    private fun resetToIdle() {
        pendingCapture = null
        finishButton?.visibility = View.GONE
        updateFinishButtonEnabled(false)
        showingErrorHint = false
        ignoreRecordingFinalize = false // Reset finalize ignore flag

        captureButton?.apply {
            visibility = View.VISIBLE
            isEnabled = true
            resetState()
            setRecording(false)
            setVideoMode(isVideoMode())
        }
        // Hide and release preview player/views if any
        try {
            previewPlayerView?.player = null
            previewPlayerView = null
            previewPlayer?.release()
        } catch (_: Exception) {}
        previewPlayer = null
        previewContainer?.removeAllViews()
        previewContainer?.visibility = View.GONE
        previewBackButton?.let { (it.parent as? ViewGroup)?.removeView(it) }
        previewBackButton = null
        backButton?.visibility = View.VISIBLE
        switchCameraButton?.visibility = View.VISIBLE
        hintText?.visibility = View.VISIBLE
        stopTimerTicker()
        timerText?.visibility = View.GONE
        timerText?.text = "00:00"
        backButton?.let { button ->
            button.post { placeBackButtonBottom(button) }
        }
        updateHint()
    }

    private fun enterPreviewState(pending: PendingCapture) {
        // Clean any previous pending capture
        pendingCapture?.let { prev -> if (prev.file != pending.file && prev.file.exists()) prev.file.delete() }
        pendingCapture = pending

        // Hide main controls
        captureButton?.apply { visibility = View.INVISIBLE; isEnabled = false }
        switchCameraButton?.visibility = View.GONE
        hintText?.apply { text = ""; visibility = View.INVISIBLE }
        timerText?.visibility = View.GONE

        val container = previewContainer ?: return
        container.visibility = View.VISIBLE
        container.removeAllViews()

        if (pending.fileType == "video") {
            val pv = PlayerView(requireContext()).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                useController = false
                resizeMode = AspectRatioFrameLayout.RESIZE_MODE_ZOOM
                setShutterBackgroundColor(Color.TRANSPARENT)
                setBackgroundColor(Color.TRANSPARENT)
            }
            val player = ExoPlayer.Builder(requireContext()).build().apply {
                repeatMode = Player.REPEAT_MODE_ALL
                setMediaItem(MediaItem.fromUri(Uri.fromFile(pending.file)))
                prepare()
                playWhenReady = true
            }
            pv.player = player
            container.addView(pv)
            previewPlayer = player
            previewPlayerView = pv
        } else {
            val imageView = ImageView(requireContext()).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                scaleType = ImageView.ScaleType.CENTER_CROP
                setBackgroundColor(Color.BLACK)
            }
            // Decode off the UI thread to reduce latency
            container.addView(imageView)
            Thread {
                val bmp = try {
                    val metrics = resources.displayMetrics
                    val targetW = metrics.widthPixels
                    val targetH = metrics.heightPixels
                    val opts = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    android.graphics.BitmapFactory.decodeFile(pending.file.absolutePath, opts)
                    var sample = 1
                    while (opts.outWidth / sample > targetW * 2 || opts.outHeight / sample > targetH * 2) sample *= 2
                    val opts2 = android.graphics.BitmapFactory.Options().apply {
                        inSampleSize = sample
                        inPreferredConfig = android.graphics.Bitmap.Config.RGB_565
                    }
                    android.graphics.BitmapFactory.decodeFile(pending.file.absolutePath, opts2)
                } catch (_: Exception) { null }
                imageView.post {
                    if (bmp != null) imageView.setImageBitmap(bmp) else imageView.setImageURI(Uri.fromFile(pending.file))
                }
            }.start()
        }

        // Top-left back arrow inside preview container
        if (previewBackButton == null) {
            previewBackButton = RoundBackArrowButton(requireContext()).apply {
                contentDescription = getString(R.string.lx_common_back)
                setOnClickListener { handleBackButton() }
            }
            container.addView(
                previewBackButton,
                FrameLayout.LayoutParams(dp(requireContext(), 28f), dp(requireContext(), 28f)).apply {
                    gravity = Gravity.TOP or Gravity.START
                    leftMargin = dp(requireContext(), 16f)
                    topMargin = statusBarHeight() + dp(requireContext(), 12f)
                }
            )
        } else {
            previewBackButton?.visibility = View.VISIBLE
        }

        // Finish button on top of preview container (bottom-right)
        finishButton?.let { btn ->
            val lp = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM or Gravity.END
            ).apply {
                rightMargin = dp(requireContext(), 20f)
                bottomMargin = dp(requireContext(), 20f)
            }
            if (btn.parent !== container) {
                (btn.parent as? ViewGroup)?.removeView(btn)
                container.addView(btn, lp)
            } else {
                btn.layoutParams = lp
            }
            // Apply bottom inset updates via helper
            ActivityInsets.applyBottomMargin(container, btn, dp(requireContext(), 20f))
            btn.visibility = View.VISIBLE
            updateFinishButtonEnabled(true)
            btn.bringToFront()
        }

        container.bringToFront()
    }

    private fun placeBackButtonTop(button: CutoutChevronButton) {
        val params = button.layoutParams as? FrameLayout.LayoutParams ?: return
        params.gravity = Gravity.TOP or Gravity.START
        params.leftMargin = dp(button.context, 16f)
        params.topMargin = statusBarHeight() + dp(button.context, 16f)
        params.bottomMargin = 0
        button.layoutParams = params
    }

    private fun updateFinishButtonEnabled(enabled: Boolean) {
        finishButton?.let { btn ->
            btn.isEnabled = enabled
            finishButtonBackground?.setColor(if (enabled) techBlueColor else lightTechBlueColor)
            btn.alpha = if (enabled) 1f else 0.8f
        }
    }

    private fun placeBackButtonBottom(button: CutoutChevronButton) {
        val params = button.layoutParams as? FrameLayout.LayoutParams ?: return
        val capture = captureButton ?: return
        val captureParams = capture.layoutParams as? FrameLayout.LayoutParams ?: return
        val captureHeight = if (capture.height > 0) capture.height else captureParams.height
        val buttonSize = if (button.height > 0) button.height else params.height.takeIf { it > 0 } ?: dp(button.context, 32f)

        // Calculate position between left edge and big circle left edge
        val screenEdge = dp(button.context, 20f)
        val screenWidth = button.context.resources.displayMetrics.widthPixels
        val bigCircleLeftEdge = (screenWidth / 2) - (captureHeight / 2)  // Big circle left edge
        val availableSpace = bigCircleLeftEdge - screenEdge
        val centerPosition = screenEdge + (availableSpace / 2) - (buttonSize / 2)

        params.gravity = Gravity.START or Gravity.BOTTOM
        params.leftMargin = centerPosition
        params.topMargin = 0
        params.bottomMargin = captureParams.bottomMargin + (captureHeight - buttonSize).coerceAtLeast(0) / 2
        button.layoutParams = params
    }

    private fun releaseCamera() {
        stopMaxDurationCountdown()
        stopTimerTicker()
        try {
            activeRecording?.stop()
        } catch (_: Exception) {
        }
        activeRecording = null
        camera?.cameraControl?.enableTorch(false)
        torchEnabled = false
        try {
            previewPlayerView?.player = null
            previewPlayerView = null
            previewPlayer?.release()
        } catch (_: Exception) {}
        previewPlayer = null
        previewContainer?.removeAllViews()
        previewContainer?.visibility = View.GONE
        previewBackButton?.let { (it.parent as? ViewGroup)?.removeView(it) }
        previewBackButton = null
        cameraProvider?.unbindAll()
        cameraProvider = null
        camera = null
        imageCapture = null
        videoCapture = null
    }

    private fun formatTimerDisplay(secondsInput: Int): SpannableString {
        val totalSeconds = secondsInput.coerceAtLeast(0)
        val minutes = totalSeconds / 60
        val seconds = totalSeconds % 60
        val timeText = String.format(Locale.getDefault(), "● %02d:%02d", minutes, seconds)
        return SpannableString(timeText).apply {
            setSpan(
                ForegroundColorSpan(Color.RED),
                0,
                1,
                android.text.Spannable.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    private fun startTimerTicker() {
        recordingStartElapsedMs = SystemClock.elapsedRealtime()
        timerUpdater?.let { handler.removeCallbacks(it) }
        val runnable = object : Runnable {
            override fun run() {
                val start = recordingStartElapsedMs
                val txt = timerText ?: return
                if (start <= 0L || activeRecording == null) return
                val elapsed = (SystemClock.elapsedRealtime() - start).coerceAtLeast(0L)
                val elapsedSeconds = (elapsed / 1000L).toInt()
                val maxDuration = maxDurationSeconds
                val displaySeconds = if (maxDuration > 0) {
                    (maxDuration - elapsedSeconds).coerceAtLeast(0)
                } else {
                    elapsedSeconds
                }
                txt.text = formatTimerDisplay(displaySeconds)
                handler.postDelayed(this, 200L)
            }
        }
        timerUpdater = runnable
        handler.post(runnable)
    }

    private fun stopTimerTicker() {
        timerUpdater?.let { handler.removeCallbacks(it) }
        timerUpdater = null
        recordingStartElapsedMs = -1L
    }

    private fun showShortDurationHint() {
        showingErrorHint = true
        // Ensure hint is visible and has correct text
        hintText?.visibility = View.VISIBLE
        hintText?.text = getString(R.string.lx_error_video_too_short)
        // Reset hint after 1.5 seconds
        handler.postDelayed({
            showingErrorHint = false
            updateHint()
        }, 1500)
    }

    private fun removeSelf() {
        try {
            activity?.supportFragmentManager
                ?.beginTransaction()
                ?.remove(this)
                ?.commitAllowingStateLoss()
        } catch (e: Exception) {
            Log.w(TAG, "removeSelf: failed", e)
        }
    }

    private fun createOutputFile(suffix: String): File {
        // Strict: LxApp cache dir is guaranteed
        val appId = (activity as com.lingxia.lxapp.LxAppActivity).getAppId()
        val info = NativeApi.getLxAppInfo(appId)!!
        val dir = File(info.cacheDir).apply { if (!exists()) mkdirs() }
        val now = System.currentTimeMillis()
        val name = if (suffix == ".mp4") {
            "video_" + now.toString() + suffix
        } else {
            "photo_" + now.toString() + suffix
        }
        return File(dir, name)
    }

    private fun openFileDescriptor(file: File): Int? {
        return try {
            val pfd = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
            val fd = pfd.detachFd()
            pfd.close()
            fd
        } catch (e: Exception) {
            Log.e(TAG, "openFileDescriptor: failed", e)
            null
        }
    }

    private class ShutterButtonView(context: Context) : FrameLayout(context) {
        private val diameter: Int
        val diameterPx: Int get() = diameter

        private val progressRing: ProgressRingView
        private val innerCircle: InnerCircleView
        private var recording = false

        fun ringStrokeWidthPx(): Float = progressRing.strokeWidthPx

        private class InnerCircleView(context: Context, private val button: ShutterButtonView) : View(context) {
            var transition: Float = 0f
                set(value) {
                    field = value
                    invalidate()
                }

            override fun onDraw(canvas: Canvas) {
                super.onDraw(canvas)
                val cx = width / 2f
                val cy = height / 2f
                val radius = min(width, height) / 2f

                // Draw inner circle (white/gray) — two-layer design (ring + inner circle)
                // Stronger shrink on long-press/recording
                val idleRatio = 0.68f
                val recRatio = 0.30f
                val currentRatio = idleRatio - transition * (idleRatio - recRatio)
                var innerRadius = radius * currentRatio
                val ringStroke = button.ringStrokeWidthPx()
                val ringInner = radius - ringStroke / 2f
                val overlap = 0.5f // pixels, tiny overlap to avoid seam
                if (button.recording) {
                    // Pressed: hug ring inner edge (no gap), shrink comes from thicker ring
                    innerRadius = (ringInner + overlap).coerceAtLeast(0f)
                } else {
                    // Idle: ensure no seam; allow slightly larger inner radius than ring inner edge
                    if (innerRadius < ringInner + overlap) innerRadius = ringInner + overlap
                }
                val innerPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    // More visible color change while pressed/recording
                    color = if (button.recording) Color.parseColor("#E0E0E0") else Color.WHITE
                    style = Paint.Style.FILL
                }
                canvas.drawCircle(cx, cy, innerRadius, innerPaint)
            }
        }

        init {
            val metrics = context.resources.displayMetrics
            val base = min(metrics.widthPixels, metrics.heightPixels)
            diameter = max(dp(context, 88f), (base * 0.16f).toInt())
            layoutParams = LayoutParams(diameter, diameter, Gravity.CENTER)

            // Draw order: ring (with progress) below, inner circle above -> avoids inner-edge seam
            progressRing = ProgressRingView(context).also { ring ->
                ring.layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT)
            }
            addView(progressRing)

            innerCircle = InnerCircleView(context, this)
            addView(innerCircle, LayoutParams(diameter, diameter, Gravity.CENTER))
            // Apply idle ring thickness immediately for clear visibility
            animatePress(false)
            refresh()
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            if (w != oldw || h != oldh) {
                post { refresh() }
            }
        }

        override fun setEnabled(enabled: Boolean) {
            super.setEnabled(enabled)
            alpha = if (enabled) 1f else 0.35f
        }

        fun setVideoMode(isVideo: Boolean) {
            progressRing.showProgress = isVideo
            refresh()
        }

        fun setRecording(value: Boolean) { animatePress(value) }

        fun pressVisual(pressed: Boolean) { if (!recording) animatePress(pressed) }

        private fun animatePress(pressed: Boolean) {
            recording = pressed
            if (!recording) progressRing.progress = 0f
            val d = diameter.toFloat()
            // Make idle ring clearly visible; pressed ring much thicker to shrink the inner circle (no gap)
            val strokeTarget = if (pressed) d * 0.42f else d * 0.22f
            progressRing.animateStrokeTo(strokeTarget, 180)
            val from = innerCircle.transition
            val to = if (pressed) 1f else 0f
            ValueAnimator.ofFloat(from, to).apply {
                duration = 180
                addUpdateListener { va ->
                    innerCircle.transition = va.animatedValue as Float
                }
            }.start()
            refresh()
        }

        fun setProgress(value: Float) {
            progressRing.progress = value
        }

        fun resetState() {
            recording = false
            progressRing.progress = 0f

            // Reset any scale transformations to avoid polygon issue
            innerCircle.scaleX = 1f
            innerCircle.scaleY = 1f

            refresh()
        }

        private fun refresh() {
            innerCircle.invalidate()
        }
    }

    private class ProgressRingView(context: Context) : View(context) {
        private val backgroundPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            // Revert to earlier subtler ring color
            color = Color.parseColor("#553A3A3C")
        }
        private val progressPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            color = Color.parseColor("#FF07C160")
        }
        private val ringRect = RectF()       // ring centerline rect
        private val progressRect = RectF()   // progress centerline rect (outer half of ring)
        var progress: Float = 0f
            set(value) {
                field = value.coerceIn(0f, 1f)
                invalidate()
            }

        val strokeWidthPx: Float
            get() = backgroundPaint.strokeWidth.takeIf { it > 0f } ?: dp(context, 6f).toFloat()

        var showProgress: Boolean = false

        init {
            val stroke = dp(context, 6f).toFloat()
            backgroundPaint.strokeWidth = stroke
            // Keep progress clearly thinner than ring
            progressPaint.strokeWidth = dp(context, 2f).toFloat()
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            val size = min(w, h)
            if (size <= 0) return
            val ringStroke = max(dp(context, 3f).toFloat(), size * 0.075f)
            backgroundPaint.strokeWidth = ringStroke
            // Progress stays constant thickness
            progressPaint.strokeWidth = dp(context, 2f).toFloat()
        }

        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)
            // Compute centerline rects each draw to follow animated stroke
            // Ring centerline
            val ringInset = backgroundPaint.strokeWidth / 2f
            ringRect.set(ringInset, ringInset, width - ringInset, height - ringInset)

            // Progress hugs the ring's outer edge (not inside), use its own thinner stroke centerline
            val progInset = progressPaint.strokeWidth / 2f
            progressRect.set(progInset, progInset, width - progInset, height - progInset)

            // Draw base ring
            canvas.drawOval(ringRect, backgroundPaint)
            // Draw progress along outer edge of ring (not inside)
            if (showProgress && progress > 0f) {
                val sweep = 360f * progress
                canvas.drawArc(progressRect, -90f, sweep, false, progressPaint)
            }
        }

        fun animateStrokeTo(target: Float, duration: Long = 220L) {
            val from = backgroundPaint.strokeWidth
            if (kotlin.math.abs(from - target) < 0.5f) return
            val animator = ValueAnimator.ofFloat(from, target).setDuration(duration)
            animator.addUpdateListener {
                val v = it.animatedValue as Float
                backgroundPaint.strokeWidth = v
                // Progress width stays constant to keep thinner than ring
                progressPaint.strokeWidth = dp(context, 2f).toFloat()
                invalidate()
            }
            animator.start()
        }
    }

    private class CutoutChevronButton(context: Context) : View(context) {
        private val circlePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE
            style = Paint.Style.FILL
        }
        private val shadowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor("#20000000")
            style = Paint.Style.FILL
        }
        private val cutoutPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.TRANSPARENT
            xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
        }

        init {
            isClickable = true
            isFocusable = true
            setLayerType(LAYER_TYPE_SOFTWARE, null)
        }

        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            val desired = dp(context, 32f)  // Smaller size
            val resolvedW = resolveSize(desired, widthMeasureSpec)
            val resolvedH = resolveSize(desired, heightMeasureSpec)
            val size = min(resolvedW, resolvedH)
            setMeasuredDimension(size, size)
        }

        override fun onDraw(canvas: Canvas) {
            val radius = min(width, height) / 2f
            val cx = width / 2f
            val cy = height / 2f

            // Draw subtle shadow
            canvas.drawCircle(cx + 1f, cy + 1f, radius, shadowPaint)

            // Draw white circle background
            canvas.drawCircle(cx, cy, radius, circlePaint)

            // Create V-shaped cutout (V opening upward, point downward)
            val chevronSize = radius * 0.4f
            val strokeWidth = radius * 0.12f

            cutoutPaint.strokeWidth = strokeWidth
            cutoutPaint.style = Paint.Style.STROKE

            val chevronPath = android.graphics.Path().apply {
                // V shape: like the letter V
                moveTo(cx - chevronSize * 0.4f, cy - chevronSize * 0.3f)  // Top left
                lineTo(cx, cy + chevronSize * 0.3f)                       // Bottom center (point)
                lineTo(cx + chevronSize * 0.4f, cy - chevronSize * 0.3f)  // Top right
            }

            // Cut out the V shape with stroke
            canvas.drawPath(chevronPath, cutoutPaint)
        }
    }



    // Top-left back in preview: smaller white circle with cutout curved arrow
    // Camera switch button with cutout refresh arrows
    private inner class CameraSwitchButton(context: Context) : View(context) {
        private val circlePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE
            style = Paint.Style.FILL
        }
        private val cutoutPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.TRANSPARENT
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
            xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
        }

        init {
            isClickable = true
            isFocusable = true
            setLayerType(LAYER_TYPE_SOFTWARE, null)
        }

        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            val desired = dp(context, 44f)
            val resolvedW = resolveSize(desired, widthMeasureSpec)
            val resolvedH = resolveSize(desired, heightMeasureSpec)
            val size = min(resolvedW, resolvedH)
            setMeasuredDimension(size, size)
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            cutoutPaint.strokeWidth = (min(w, h) * 0.045f).coerceAtLeast(1.5f)
        }

        override fun onDraw(canvas: Canvas) {
            val cx = width / 2f
            val cy = height / 2f
            val size = min(width, height).toFloat()

            // Draw camera body (rounded rectangle)
            val bodyWidth = size * 0.65f
            val bodyHeight = size * 0.4f
            val bodyLeft = cx - bodyWidth * 0.5f
            val bodyRight = cx + bodyWidth * 0.5f
            val bodyTop = cy - bodyHeight * 0.2f
            val bodyBottom = cy + bodyHeight * 0.8f
            val bodyRadius = size * 0.05f
            val bodyRect = RectF(bodyLeft, bodyTop, bodyRight, bodyBottom)
            canvas.drawRoundRect(bodyRect, bodyRadius, bodyRadius, circlePaint)

            // Draw camera viewfinder hump (trapezoid)
            val trapPath = android.graphics.Path().apply {
                val topWidth = bodyWidth * 0.35f
                val bottomWidth = bodyWidth * 0.45f
                val trapHeight = bodyHeight * 0.35f
                val topLeft = cx - topWidth * 0.5f
                val topRight = cx + topWidth * 0.5f
                val bottomLeft = cx - bottomWidth * 0.5f
                val bottomRight = cx + bottomWidth * 0.5f
                val trapTop = bodyTop - trapHeight * 0.8f
                val trapBottom = bodyTop + trapHeight * 0.2f
                moveTo(topLeft, trapTop)
                lineTo(topRight, trapTop)
                lineTo(bottomRight, trapBottom)
                lineTo(bottomLeft, trapBottom)
                close()
            }
            canvas.drawPath(trapPath, circlePaint)

            // Draw cutout refresh arrows in the camera body
            val arrowCenterY = (bodyTop + bodyBottom) / 2f
            val arrowRadius = bodyHeight * 0.28f

            val arrowPath = android.graphics.Path().apply {
                // Upper-right arc
                val arcRect1 = RectF(
                    cx - arrowRadius, arrowCenterY - arrowRadius,
                    cx + arrowRadius, arrowCenterY + arrowRadius
                )
                arcTo(arcRect1, -90f, 90f, true)
                // Arrow head for upper-right
                val endX1 = cx + arrowRadius
                val endY1 = arrowCenterY
                moveTo(endX1, endY1)
                lineTo(endX1 - arrowRadius * 0.4f, endY1 - arrowRadius * 0.3f)
                moveTo(endX1, endY1)
                lineTo(endX1 - arrowRadius * 0.3f, endY1 + arrowRadius * 0.4f)

                // Lower-left arc
                arcTo(arcRect1, 90f, 90f, true)
                // Arrow head for lower-left
                val endX2 = cx - arrowRadius
                val endY2 = arrowCenterY
                moveTo(endX2, endY2)
                lineTo(endX2 + arrowRadius * 0.4f, endY2 + arrowRadius * 0.3f)
                moveTo(endX2, endY2)
                lineTo(endX2 + arrowRadius * 0.3f, endY2 - arrowRadius * 0.4f)
            }
            canvas.drawPath(arrowPath, cutoutPaint)
        }
    }

    // Flash button using XML drawable icons (matching HarmonyOS style)
    private inner class FlashButton(context: Context) : ImageButton(context) {
        init {
            setBackgroundColor(Color.TRANSPARENT)
            scaleType = ScaleType.FIT_XY  // Scale to fill container
            setPadding(0, 0, 0, 0)
            updateFlashState()
        }

        fun updateFlashState() {
            val iconRes = if (isVideoMode()) {
                if (torchEnabled) R.drawable.icon_camera_flash_on else R.drawable.icon_camera_flash_off
            } else {
                when (flashMode) {
                    ImageCapture.FLASH_MODE_OFF -> R.drawable.icon_camera_flash_off
                    else -> R.drawable.icon_camera_flash_on
                }
            }
            setImageResource(iconRes)
        }
    }

    private fun toggleFlash() {
        if (isVideoMode()) {
            torchEnabled = !torchEnabled
            camera?.cameraControl?.enableTorch(torchEnabled)
        } else {
            flashMode = when (flashMode) {
                ImageCapture.FLASH_MODE_OFF -> ImageCapture.FLASH_MODE_ON
                ImageCapture.FLASH_MODE_ON -> ImageCapture.FLASH_MODE_AUTO
                else -> ImageCapture.FLASH_MODE_OFF
            }
            imageCapture?.flashMode = flashMode
        }
        flashButton?.updateFlashState()
    }

    // Top-left back in preview: smaller white circle with cutout curved arrow
    private class RoundBackArrowButton(context: Context) : View(context) {
        private val circlePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE
            style = Paint.Style.FILL
        }
        private val cutoutPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.TRANSPARENT
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
            xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
        }

        init {
            isClickable = true
            isFocusable = true
            setLayerType(LAYER_TYPE_SOFTWARE, null)
        }

        override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
            val desired = dp(context, 28f)  // Smaller size
            val resolvedW = resolveSize(desired, widthMeasureSpec)
            val resolvedH = resolveSize(desired, heightMeasureSpec)
            val size = min(resolvedW, resolvedH)
            setMeasuredDimension(size, size)
        }

        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            cutoutPaint.strokeWidth = (min(w, h) * 0.06f).coerceAtLeast(1.5f)  // Much thinner
        }

        override fun onDraw(canvas: Canvas) {
            val cx = width / 2f
            val cy = height / 2f
            val r = min(width, height) / 2f

            // Draw white circle background
            canvas.drawCircle(cx, cy, r, circlePaint)

            // Draw cutout curved return arrow like in reference image
            val arrowPath = android.graphics.Path().apply {
                // Start with left-pointing chevron (<) - moved further left
                val chevronTipX = cx - r * 0.4f  // Moved left from -0.3f to -0.4f
                val chevronY = cy - r * 0.1f
                val chevronSize = r * 0.2f

                // Left chevron (<)
                moveTo(chevronTipX + chevronSize, chevronY - chevronSize * 0.6f)
                lineTo(chevronTipX, chevronY)
                lineTo(chevronTipX + chevronSize, chevronY + chevronSize * 0.6f)

                // Horizontal line from chevron to the right (much longer)
                moveTo(chevronTipX + chevronSize, chevronY)
                lineTo(cx + r * 0.4f, chevronY)  // Extended from 0.35f to 0.4f

                // Create smooth rounded arc turn
                val arcStartX = cx + r * 0.4f
                val arcStartY = chevronY
                val arcEndX = cx + r * 0.25f
                val arcEndY = cy + r * 0.25f

                // Draw smooth arc using quadratic curve for more rounded feel
                moveTo(arcStartX, arcStartY)
                quadTo(
                    arcStartX + r * 0.1f, arcEndY,  // Control point creates rounded arc
                    arcEndX, arcEndY
                )

                // Short line after the turn
                lineTo(cx - r * 0.05f, arcEndY)
            }

            // Cut out the curved arrow path
            canvas.drawPath(arrowPath, cutoutPaint)
        }
    }

    private fun statusBarHeight(): Int {
        val resId = resources.getIdentifier("status_bar_height", "dimen", "android")
        return if (resId > 0) resources.getDimensionPixelSize(resId) else 0
    }

    private fun navBarHeight(): Int {
        val resId = resources.getIdentifier("navigation_bar_height", "dimen", "android")
        return if (resId > 0 && isNavigationBarPresent()) resources.getDimensionPixelSize(resId) else 0
    }

    private fun isNavigationBarPresent(): Boolean {
        val id = resources.getIdentifier("config_showNavigationBar", "bool", "android")
        return id > 0 && resources.getBoolean(id)
    }
}

private fun dp(context: Context, value: Float): Int {
    return (context.resources.displayMetrics.density * value).toInt()
}
