package com.lingxia.lxapp.APIs.media

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Bundle
import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.util.Log
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.fragment.app.Fragment
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.view.PreviewView
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import com.lingxia.lxapp.NativeApi
import org.json.JSONObject
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import android.graphics.drawable.GradientDrawable

class ScanCodeFragment : Fragment() {
    companion object {
        private const val TAG = "LingXia.Scan"
        private const val ARG_ONLY_CAMERA = "only_camera"
        private const val ARG_SCAN_TYPES = "scan_types"
        private const val ARG_CALLBACK_ID = "callback_id"

        // Group codes from Rust bridge (see lingxia-platform/src/android/media.rs)
        const val TYPE_QR = 1
        const val TYPE_BAR = 2 // 1D barcodes
        const val TYPE_DATA_MATRIX = 3
        const val TYPE_PDF_417 = 4

        fun start(
            activity: AppCompatActivity,
            scanTypes: IntArray,
            onlyFromCamera: Boolean,
            callbackId: Long
        ) {
            val fragment = ScanCodeFragment().apply {
                arguments = Bundle().apply {
                    putBoolean(ARG_ONLY_CAMERA, onlyFromCamera)
                    putLong(ARG_CALLBACK_ID, callbackId)
                    putIntArray(ARG_SCAN_TYPES, scanTypes)
                }
            }
            val fm = activity.supportFragmentManager
            fm.beginTransaction()
                .add(android.R.id.content, fragment, TAG)
                .commitAllowingStateLoss()
            fm.executePendingTransactions()
        }
    }

    private val callbackId: Long get() = arguments?.getLong(ARG_CALLBACK_ID) ?: 0L
    private val onlyFromCamera: Boolean get() = arguments?.getBoolean(ARG_ONLY_CAMERA) ?: true
    private val scanTypes: IntArray
        get() = arguments?.getIntArray(ARG_SCAN_TYPES) ?: intArrayOf()

    private var previewView: PreviewView? = null
    private var closeButton: ImageButton? = null
    private var galleryButton: View? = null
    private var overlayLayer: FrameLayout? = null
    private var scanLine: View? = null
    private var lineAnimator: ValueAnimator? = null

    private var cameraExecutor: ExecutorService? = null
    private var barcodeScanner: BarcodeScanner? = null
    private var hasReportedResult = false

    private val permissionLauncher =
        registerForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                startCamera()
            } else {
                deliverFailure("Camera permission denied")
            }
        }

    override fun onAttach(context: Context) {
        super.onAttach(context)
        cameraExecutor = Executors.newSingleThreadExecutor()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        barcodeScanner = buildScanner()
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val ctx = requireContext()
        val root = FrameLayout(ctx).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.BLACK)
        }

        previewView = PreviewView(ctx).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            scaleType = PreviewView.ScaleType.FILL_CENTER
        }
        root.addView(previewView)

        // Overlay layer to host the full-width scan line (no scan box)
        overlayLayer = FrameLayout(ctx).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }
        root.addView(overlayLayer)

        // Add a running scan line (full-width, blue with long trailing shadow)
        scanLine = View(ctx).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                dp(24f),
                Gravity.TOP
            )
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(
                    Color.parseColor("#CC1677FF"), // bright blue head
                    Color.parseColor("#801677FF"), // mid tail
                    Color.parseColor("#331677FF"), // long tail
                    Color.parseColor("#001677FF")  // fade out
                )
            ).apply { shape = GradientDrawable.RECTANGLE }
        }
        overlayLayer?.addView(scanLine)

        closeButton = ImageButton(ctx).apply {
            // Circle background with an "x" icon
            background = android.graphics.drawable.GradientDrawable().apply {
                shape = android.graphics.drawable.GradientDrawable.OVAL
                setColor(Color.parseColor("#66000000"))
            }
            setImageResource(android.R.drawable.ic_menu_close_clear_cancel)
            setColorFilter(Color.WHITE)
            scaleType = android.widget.ImageView.ScaleType.CENTER
            contentDescription = "Close"
            setOnClickListener { deliverCancelled() }
            // Ensure a circular touch target
            val pad = dp(8f)
            setPadding(pad, pad, pad, pad)
        }
        val closeParams = FrameLayout.LayoutParams(
            dp(40f),
            dp(40f),
            Gravity.START or Gravity.TOP
        )
        // Move the close button down a bit more
        closeParams.topMargin = dp(56f)
        closeParams.marginStart = dp(16f)
        root.addView(closeButton, closeParams)

        if (!onlyFromCamera) {
            // Bottom-center album button with icon + label
            val container = LinearLayout(ctx).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER_HORIZONTAL
                // No rectangular background; only show the circular icon below
                setPadding(0, 0, 0, 0)
                isClickable = true
                isFocusable = true
                setOnClickListener { openAlbumPicker() }
            }
            val iconWrap = FrameLayout(ctx).apply {
                layoutParams = LinearLayout.LayoutParams(dp(72f), dp(72f)).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                }
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(Color.parseColor("#66000000"))
                }
            }
            val icon = ImageView(ctx).apply {
                setImageResource(android.R.drawable.ic_menu_gallery)
                setColorFilter(Color.WHITE)
                layoutParams = FrameLayout.LayoutParams(dp(36f), dp(36f), Gravity.CENTER)
            }
            iconWrap.addView(icon)
            val label = TextView(ctx).apply {
                text = "图库"
                setTextColor(Color.WHITE)
                textSize = 16f
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                    topMargin = dp(8f)
                }
            }
            container.addView(iconWrap)
            container.addView(label)
            val galleryParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            ).apply {
                bottomMargin = dp(96f) // not too bottom
            }
            root.addView(container, galleryParams)
            galleryButton = container
        }

        return root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        ensurePermissionAndStart()
        // Start scan line animation when layout is ready
        overlayLayer?.viewTreeObserver?.addOnGlobalLayoutListener(object : android.view.ViewTreeObserver.OnGlobalLayoutListener {
            override fun onGlobalLayout() {
                overlayLayer?.viewTreeObserver?.removeOnGlobalLayoutListener(this)
                startScanLineAnimation()
            }
        })
    }

    override fun onDestroyView() {
        super.onDestroyView()
        stopScanLineAnimation()
        previewView = null
        closeButton = null
        galleryButton = null
        scanLine = null
        overlayLayer = null
    }

    override fun onDestroy() {
        super.onDestroy()
        hasReportedResult = true
        cameraExecutor?.shutdown()
        cameraExecutor = null
        try {
            barcodeScanner?.close()
        } catch (e: Exception) {
            Log.w(TAG, "Failed to close barcode scanner", e)
        }
        barcodeScanner = null
    }

    private fun ensurePermissionAndStart() {
        val ctx = context ?: return
        if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            startCamera()
        } else {
            permissionLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    private fun buildScanner(): BarcodeScanner {
        if (scanTypes.isEmpty()) {
            BarcodeScanning.getClient()
        }
        val formats = mutableListOf<Int>()
        scanTypes.forEach { code ->
            when (code) {
                TYPE_QR -> formats.add(Barcode.FORMAT_QR_CODE)
                TYPE_DATA_MATRIX -> formats.add(Barcode.FORMAT_DATA_MATRIX)
                TYPE_PDF_417 -> formats.add(Barcode.FORMAT_PDF417)
                TYPE_BAR -> {
                    formats.add(Barcode.FORMAT_CODE_128)
                    formats.add(Barcode.FORMAT_CODE_39)
                    formats.add(Barcode.FORMAT_CODE_93)
                    formats.add(Barcode.FORMAT_CODABAR)
                    formats.add(Barcode.FORMAT_EAN_8)
                    formats.add(Barcode.FORMAT_EAN_13)
                    formats.add(Barcode.FORMAT_ITF)
                    formats.add(Barcode.FORMAT_UPC_A)
                    formats.add(Barcode.FORMAT_UPC_E)
                }
            }
        }
        val unique = formats.distinct()
        if (unique.isEmpty()) {
            return BarcodeScanning.getClient()
        }
        val primary = unique.first()
        val options = BarcodeScannerOptions.Builder()
            .setBarcodeFormats(primary, *unique.drop(1).toIntArray())
            .build()
        return BarcodeScanning.getClient(options)
    }

    private fun startCamera() {
        val ctx = context ?: return
        val preview = previewView ?: return
        val executor = ContextCompat.getMainExecutor(ctx)

        val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)
        cameraProviderFuture.addListener({
            try {
                val cameraProvider = cameraProviderFuture.get()
                val previewUseCase = Preview.Builder().build().also {
                    it.setSurfaceProvider(preview.surfaceProvider)
                }
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                val executorService = cameraExecutor ?: Executors.newSingleThreadExecutor().also {
                    cameraExecutor = it
                }
                analysis.setAnalyzer(executorService, BarcodeAnalyzer())

                cameraProvider.unbindAll()
                cameraProvider.bindToLifecycle(
                    this,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    previewUseCase,
                    analysis
                )
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start camera", e)
                deliverFailure("Unable to start camera: ${e.message}")
            }
        }, executor)
    }

    private fun openAlbumPicker() {
        val host = activity as? AppCompatActivity ?: run {
            Toast.makeText(requireContext(), "Host not AppCompatActivity", Toast.LENGTH_SHORT).show()
            return
        }
        MediaPickerFragment.pick(
            host,
            maxCount = 1,
            mode = "images",
            allowCamera = false
        ) { uris ->
            val first = uris.firstOrNull() ?: return@pick
            try {
                val ctx = requireContext()
                val image = InputImage.fromFilePath(ctx, first)
                barcodeScanner?.process(image)
                    ?.addOnSuccessListener { barcodes ->
                        if (!barcodes.isNullOrEmpty()) {
                            handleBarcode(barcodes.first())
                        } else {
                            Toast.makeText(ctx, "No code detected", Toast.LENGTH_SHORT).show()
                        }
                    }
                    ?.addOnFailureListener { error ->
                        Log.e(TAG, "Gallery scan failed", error)
                        Toast.makeText(ctx, "Failed to scan image", Toast.LENGTH_SHORT).show()
                    }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to process selected image", e)
                context?.let {
                    Toast.makeText(it, "Failed to process image", Toast.LENGTH_SHORT).show()
                }
            }
        }
    }

    private inner class BarcodeAnalyzer : ImageAnalysis.Analyzer {
        override fun analyze(imageProxy: androidx.camera.core.ImageProxy) {
            if (hasReportedResult) {
                imageProxy.close()
                return
            }
            val mediaImage = imageProxy.image
            if (mediaImage == null) {
                imageProxy.close()
                return
            }
            val image =
                InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
            try {
                barcodeScanner?.process(image)
                    ?.addOnSuccessListener { barcodes ->
                        if (!barcodes.isNullOrEmpty()) {
                            handleBarcode(barcodes.first())
                        }
                    }
                    ?.addOnFailureListener { error ->
                        Log.w(TAG, "Camera scan failed: ${error.message}")
                    }
                    ?.addOnCompleteListener {
                        imageProxy.close()
                    }
            } catch (e: Exception) {
                Log.e(TAG, "Barcode processing error", e)
                imageProxy.close()
            }
        }
    }

    private fun handleBarcode(barcode: Barcode) {
        if (hasReportedResult) {
            return
        }
        val value = barcode.rawValue ?: barcode.displayValue
        if (value.isNullOrEmpty()) {
            return
        }
        hasReportedResult = true
        val type = mapBarcodeFormatToType(barcode.format)
        val payload = JSONObject().apply {
            put("scanResult", value)
            put("scanType", type)
        }
        NativeApi.onCallback(callbackId, true, payload.toString())
        safeClose()
    }

    private fun mapBarcodeFormatToType(format: Int): String {
        return when (format) {
            Barcode.FORMAT_QR_CODE -> "QR_CODE"
            Barcode.FORMAT_DATA_MATRIX -> "DATA_MATRIX"
            Barcode.FORMAT_PDF417 -> "PDF_417"
            Barcode.FORMAT_AZTEC -> "AZTEC"
            Barcode.FORMAT_CODABAR -> "CODABAR"
            Barcode.FORMAT_CODE_39 -> "CODE_39"
            Barcode.FORMAT_CODE_93 -> "CODE_93"
            Barcode.FORMAT_CODE_128 -> "CODE_128"
            Barcode.FORMAT_EAN_8 -> "EAN_8"
            Barcode.FORMAT_EAN_13 -> "EAN_13"
            Barcode.FORMAT_ITF -> "ITF"
            Barcode.FORMAT_UPC_A -> "UPC_A"
            Barcode.FORMAT_UPC_E -> "UPC_E"
            else -> "UNKNOWN"
        }
    }

    private fun deliverCancelled() {
        if (hasReportedResult) {
            safeClose()
            return
        }
        hasReportedResult = true
        NativeApi.onCallback(callbackId, true, "")
        safeClose()
    }

    private fun deliverFailure(message: String) {
        if (hasReportedResult) {
            safeClose()
            return
        }
        hasReportedResult = true
        NativeApi.onCallback(callbackId, false, message)
        safeClose()
    }

    private fun safeClose() {
        previewView = null
        if (isAdded) {
            parentFragmentManager.beginTransaction()
                .remove(this)
                .commitAllowingStateLoss()
        }
    }

    private fun startScanLineAnimation() {
        val layer = overlayLayer ?: return
        val line = scanLine ?: return
        val totalH = layer.height
        val lineH = line.layoutParams.height
        if (totalH <= 0 || lineH <= 0) return
        // Scan within central band of the screen (e.g., 20% to 80%)
        val startY = (totalH * 0.20f)
        val endY = (totalH * 0.80f - lineH)
        if (endY <= startY) return
        line.translationY = startY
        lineAnimator?.cancel()
        lineAnimator = ObjectAnimator.ofFloat(line, View.TRANSLATION_Y, startY, endY).apply {
            duration = 1800L
            repeatCount = ValueAnimator.INFINITE
            repeatMode = ValueAnimator.REVERSE // up and down
            start()
        }
    }

    private fun stopScanLineAnimation() {
        lineAnimator?.cancel()
        lineAnimator = null
    }

    private fun dp(value: Float): Int {
        val d = resources.displayMetrics.density
        return (value * d + 0.5f).toInt()
    }
}
