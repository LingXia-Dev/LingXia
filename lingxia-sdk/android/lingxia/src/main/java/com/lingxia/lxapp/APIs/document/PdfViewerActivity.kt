package com.lingxia.lxapp.APIs.document

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.util.TypedValue
import android.view.Gravity
import android.view.GestureDetector
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.Toolbar
import androidx.core.view.WindowInsetsControllerCompat
import com.google.android.material.appbar.MaterialToolbar
import com.google.android.material.color.MaterialColors
import java.io.File
import java.io.IOException
import kotlin.math.abs
import kotlin.math.roundToInt

class PdfViewerActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_FILE_PATH = "com.lingxia.lxapp.document.extra.FILE_PATH"
        const val EXTRA_DISPLAY_NAME = "com.lingxia.lxapp.document.extra.DISPLAY_NAME"
        const val EXTRA_SHOW_MENU = "com.lingxia.lxapp.document.extra.SHOW_MENU"
        private const val MENU_ID_SHARE = 1
        private const val FLING_DISTANCE_DP = 48
        private const val FLING_VELOCITY_DP = 24
    }

    private var parcelFileDescriptor: ParcelFileDescriptor? = null
    private var pdfRenderer: PdfRenderer? = null
    private var currentPage: PdfRenderer.Page? = null

    private lateinit var toolbar: MaterialToolbar
    private lateinit var rootLayout: LinearLayout
    private lateinit var contentFrame: FrameLayout
    private lateinit var overlayContainer: LinearLayout
    private lateinit var imageView: ZoomableImageView
    private lateinit var pageIndicator: TextView
    private lateinit var gestureDetector: GestureDetector

    private var pageCount: Int = 0
    private var currentIndex: Int = 0
    private var showMenu: Boolean = true
    private var filePath: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        filePath = intent.getStringExtra(EXTRA_FILE_PATH) ?: ""
        if (filePath.isEmpty()) {
            finish()
            return
        }

        val displayName = intent.getStringExtra(EXTRA_DISPLAY_NAME) ?: File(filePath).name
        showMenu = intent.getBooleanExtra(EXTRA_SHOW_MENU, true)

        buildLayout()

        gestureDetector = GestureDetector(
            this,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent): Boolean = true

                override fun onFling(
                    e1: MotionEvent,
                    e2: MotionEvent,
                    velocityX: Float,
                    velocityY: Float
                ): Boolean {
                    if (imageView.isZoomed()) {
                        return false
                    }
                    val deltaY = e2.y - e1.y
                    val deltaX = e2.x - e1.x
                    val distanceThreshold = dp(FLING_DISTANCE_DP).toFloat()
                    val velocityThreshold = dp(FLING_VELOCITY_DP) * 8f
                    if (abs(deltaY) > abs(deltaX) &&
                        abs(deltaY) > distanceThreshold &&
                        abs(velocityY) > velocityThreshold
                    ) {
                        val moved = if (deltaY < 0) {
                            showPage(currentIndex + 1)
                        } else {
                            showPage(currentIndex - 1)
                        }
                        return moved
                    }
                    return false
                }
            }
        )
        imageView.attachGestureDetector(gestureDetector)

        toolbar.title = displayName
        setSupportActionBar(toolbar)
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        supportActionBar?.setHomeButtonEnabled(true)
        toolbar.setNavigationOnClickListener { onBackPressedDispatcher.onBackPressed() }

        applyChromeStyling()
        invalidateOptionsMenu()

        try {
            openRenderer()
            if (pageCount == 0) {
                finish()
                return
            }
            showPage(0)
        } catch (error: IOException) {
            error.printStackTrace()
            finish()
        }
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menu.clear()
        if (!showMenu) {
            return false
        }

        menu.add(Menu.NONE, MENU_ID_SHARE, Menu.NONE, "Share").apply {
            setIcon(android.R.drawable.ic_menu_share)
            setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
        }

        tintToolbarMenuIcons()
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            android.R.id.home -> {
                finish()
                true
            }
            MENU_ID_SHARE -> {
                shareDocument()
                true
            }
            else -> super.onOptionsItemSelected(item)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        closeRenderer()
    }

    private fun buildLayout() {
        rootLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT
            )
        }

        toolbar = MaterialToolbar(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                actionBarSize()
            )
            elevation = dp(4).toFloat()
        }

        contentFrame = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }

        imageView = ZoomableImageView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            contentDescription = "PDF page preview"
        }

        overlayContainer = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            setPadding(dp(16), dp(10), dp(16), dp(10))
            elevation = dp(2).toFloat()
            background = createOverlayBackground(Color.argb(220, 0, 0, 0))
            visibility = View.GONE
        }

        pageIndicator = TextView(this).apply {
            textSize = 16f
        }

        overlayContainer.addView(pageIndicator)

        contentFrame.addView(imageView)
        contentFrame.addView(
            overlayContainer,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(24)
            }
        )

        rootLayout.addView(toolbar)
        rootLayout.addView(contentFrame)

        setContentView(rootLayout)
    }

    private fun openRenderer() {
        val file = File(filePath)
        parcelFileDescriptor = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        pdfRenderer = PdfRenderer(parcelFileDescriptor!!)
        pageCount = pdfRenderer?.pageCount ?: 0
        overlayContainer.visibility = if (pageCount > 0) View.VISIBLE else View.GONE
        invalidateOptionsMenu()
    }

    private fun closeRenderer() {
        currentPage?.close()
        pdfRenderer?.close()
        parcelFileDescriptor?.close()
    }

    private fun showPage(index: Int): Boolean {
        val renderer = pdfRenderer ?: return false
        if (index < 0 || index >= renderer.pageCount) {
            return false
        }

        currentPage?.close()
        currentPage = renderer.openPage(index)

        val targetWidth = imageView.width.takeIf { it > 0 }
            ?: imageView.measuredWidth.takeIf { it > 0 }
            ?: resources.displayMetrics.widthPixels
        val scale = targetWidth.toFloat() / currentPage!!.width.toFloat()
        val bitmapWidth = (currentPage!!.width * scale).toInt()
        val bitmapHeight = (currentPage!!.height * scale).toInt()

        val bitmap = Bitmap.createBitmap(bitmapWidth, bitmapHeight, Bitmap.Config.ARGB_8888)
        currentPage?.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
        imageView.setImageBitmap(bitmap)
        imageView.resetZoom()

        currentIndex = index
        updateUi()
        return true
    }

    private fun updateUi() {
        pageIndicator.text = if (pageCount > 0) {
            "${currentIndex + 1} / $pageCount"
        } else {
            "0 / 0"
        }
        overlayContainer.visibility = if (pageCount > 0) View.VISIBLE else View.GONE
        tintToolbarMenuIcons()
    }

    private fun shareDocument() {
        try {
            val file = File(filePath)
            val contentUri: Uri = LingxiaDocumentProvider.uriForFile(this, file)

            val shareIntent = Intent(Intent.ACTION_SEND).apply {
                type = "application/pdf"
                putExtra(Intent.EXTRA_STREAM, contentUri)
                putExtra(Intent.EXTRA_TEXT, file.name)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }

            startActivity(Intent.createChooser(shareIntent, "Share PDF"))
        } catch (error: Exception) {
            Toast.makeText(this, "Failed to share document", Toast.LENGTH_SHORT).show()
        }
    }

    private fun applyChromeStyling() {
        val surfaceColor = MaterialColors.getColor(
            toolbar,
            com.google.android.material.R.attr.colorSurface,
            Color.WHITE
        )
        val onSurfaceColor = MaterialColors.getColor(
            toolbar,
            com.google.android.material.R.attr.colorOnSurface,
            Color.BLACK
        )
        val backgroundColor = MaterialColors.getColor(
            rootLayout,
            android.R.attr.colorBackground,
            Color.WHITE
        )

        rootLayout.setBackgroundColor(backgroundColor)
        contentFrame.setBackgroundColor(backgroundColor)
        imageView.setBackgroundColor(backgroundColor)

        toolbar.setBackgroundColor(surfaceColor)
        toolbar.setTitleTextColor(onSurfaceColor)
        toolbar.navigationIcon?.setTint(onSurfaceColor)

        overlayContainer.background = createOverlayBackground(
            androidx.core.graphics.ColorUtils.setAlphaComponent(surfaceColor, (0.92f * 255).toInt())
        )
        pageIndicator.setTextColor(onSurfaceColor)
        overlayContainer.alpha = 0.95f

        window.statusBarColor = surfaceColor
        window.navigationBarColor = surfaceColor

        WindowInsetsControllerCompat(window, window.decorView).apply {
            val lightBars = MaterialColors.isColorLight(surfaceColor)
            isAppearanceLightStatusBars = lightBars
            isAppearanceLightNavigationBars = lightBars
        }

        tintToolbarMenuIcons(onSurfaceColor)
    }

    private fun dp(value: Int): Int =
        TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value.toFloat(),
            resources.displayMetrics
        ).roundToInt()

    private fun actionBarSize(): Int {
        val styledAttributes = obtainStyledAttributes(intArrayOf(android.R.attr.actionBarSize))
        val size = styledAttributes.getDimensionPixelSize(0, dp(56))
        styledAttributes.recycle()
        return size
    }

    private fun tintToolbarMenuIcons(colorOverride: Int? = null) {
        val tintColor = colorOverride ?: MaterialColors.getColor(
            toolbar,
            com.google.android.material.R.attr.colorOnSurface,
            Color.BLACK
        )
        for (i in 0 until toolbar.menu.size()) {
            toolbar.menu.getItem(i).icon?.mutate()?.setTint(tintColor)
        }
    }

    private fun createOverlayBackground(color: Int): GradientDrawable {
        return GradientDrawable().apply {
            cornerRadius = dp(20).toFloat()
            setColor(color)
        }
    }

    private class ZoomableImageView(context: Context) : androidx.appcompat.widget.AppCompatImageView(context) {
        private val scaleDetector = ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    val newScale = (scaleFactor * detector.scaleFactor).coerceIn(MIN_SCALE, MAX_SCALE)
                    if (newScale == scaleFactor) {
                        return false
                    }
                    scaleFactor = newScale
                    pivotX = detector.focusX
                    pivotY = detector.focusY
                    scaleX = scaleFactor
                    scaleY = scaleFactor
                    return true
                }
            }
        )
        private var scaleFactor: Float = 1f
        private var gestureDetector: GestureDetector? = null

        init {
            scaleType = ImageView.ScaleType.FIT_CENTER
            isClickable = true
            isFocusable = true
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            gestureDetector?.onTouchEvent(event)
            scaleDetector.onTouchEvent(event)
            return true
        }

        fun attachGestureDetector(detector: GestureDetector) {
            gestureDetector = detector
        }

        fun resetZoom() {
            scaleFactor = 1f
            pivotX = width / 2f
            pivotY = height / 2f
            scaleX = 1f
            scaleY = 1f
        }

        fun isZoomed(): Boolean = scaleFactor > 1.05f

        companion object {
            private const val MIN_SCALE = 1f
            private const val MAX_SCALE = 4f
        }
    }
}
