package com.lingxia.lxapp.APIs.document
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
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ImageView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowInsetsControllerCompat
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.appbar.MaterialToolbar
import com.google.android.material.color.MaterialColors
import java.io.File
import java.io.IOException
import kotlin.math.roundToInt
import com.lingxia.lxapp.LxNavBarUtils
import com.lingxia.lxapp.R

class PdfViewerActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_FILE_PATH = "com.lingxia.lxapp.document.extra.FILE_PATH"
        const val EXTRA_DISPLAY_NAME = "com.lingxia.lxapp.document.extra.DISPLAY_NAME"
        const val EXTRA_SHOW_MENU = "com.lingxia.lxapp.document.extra.SHOW_MENU"
        private const val MENU_ID_SHARE = 1
    }

    private var parcelFileDescriptor: ParcelFileDescriptor? = null
    private var pdfRenderer: PdfRenderer? = null

    private lateinit var toolbar: MaterialToolbar
    private lateinit var rootLayout: LinearLayout
    private lateinit var contentFrame: FrameLayout
    private lateinit var zoomContainer: ZoomableRecyclerViewContainer
    private lateinit var recyclerView: RecyclerView
    private lateinit var layoutManager: LinearLayoutManager
    private lateinit var overlayContainer: LinearLayout
    private lateinit var pageIndicator: TextView
    private var overlayHideRunnable: Runnable? = null
    private val overlayHideDelayMs = 1200L

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

        toolbar.title = displayName
        setSupportActionBar(toolbar)
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        supportActionBar?.setHomeButtonEnabled(true)
        LxNavBarUtils.configureToolbarBackButton(toolbar)
        toolbar.setNavigationOnClickListener { onBackPressedDispatcher.onBackPressed() }

        applyChromeStyling()
        invalidateOptionsMenu()

        try {
            openRenderer()
            if (pageCount == 0) {
                finish()
                return
            }
            // Initial display index
            currentIndex = 0
            updateUi()
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
            setIcon(R.drawable.icon_share)
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
                dp(44)
            )
            elevation = dp(1).toFloat()
            contentInsetStartWithNavigation = dp(8)
            contentInsetEndWithActions = dp(8)
            setContentInsetsRelative(dp(8), dp(8))
            isTitleCentered = true
        }

        contentFrame = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }

        // Wrap RecyclerView in ZoomableRecyclerViewContainer for whole-screen zoom
        zoomContainer = ZoomableRecyclerViewContainer(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        recyclerView = RecyclerView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            overScrollMode = RecyclerView.OVER_SCROLL_IF_CONTENT_SCROLLS
        }

        zoomContainer.addView(recyclerView)
        contentFrame.addView(zoomContainer)

        overlayContainer = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(6), dp(12), dp(6))
            elevation = dp(2).toFloat()
            background = createOverlayBackground(Color.argb(220, 0, 0, 0))
            visibility = View.GONE
        }

        pageIndicator = TextView(this).apply {
            textSize = 14f
        }

        overlayContainer.addView(pageIndicator)

        contentFrame.addView(
            overlayContainer,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.TOP or Gravity.START
                leftMargin = dp(16)
                topMargin = dp(12)
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

        // Setup RecyclerView for continuous vertical scrolling
        layoutManager = LinearLayoutManager(this, LinearLayoutManager.VERTICAL, false)
        recyclerView.layoutManager = layoutManager
        recyclerView.adapter = PdfPageAdapter()
        recyclerView.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(rv: RecyclerView, dx: Int, dy: Int) {
                super.onScrolled(rv, dx, dy)
                val firstVisible = layoutManager.findFirstVisibleItemPosition()
                if (firstVisible != RecyclerView.NO_POSITION && firstVisible != currentIndex) {
                    currentIndex = firstVisible
                    updatePageIndicator()
                }
                showPageIndicatorTransient()
            }
        })

        invalidateOptionsMenu()
    }

    private fun closeRenderer() {
        pdfRenderer?.close()
        parcelFileDescriptor?.close()
    }

    private fun updateUi() {
        updatePageIndicator()
        tintToolbarMenuIcons()
    }

    private fun updatePageIndicator() {
        pageIndicator.text = if (pageCount > 0) "${currentIndex + 1} / $pageCount" else "0 / 0"
    }

    private fun showPageIndicatorTransient() {
        if (pageCount <= 0) return
        overlayContainer.visibility = View.VISIBLE
        overlayHideRunnable?.let { overlayContainer.removeCallbacks(it) }
        val r = Runnable { overlayContainer.visibility = View.GONE }
        overlayHideRunnable = r
        overlayContainer.postDelayed(r, overlayHideDelayMs)
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
        val navBarColor = Color.parseColor("#F8F8F8")
        val titleColor = Color.parseColor("#000000")
        val iconColor = Color.parseColor("#333333")
        val contentBgColor = Color.parseColor("#F5F5F5")

        rootLayout.setBackgroundColor(contentBgColor)
        contentFrame.setBackgroundColor(contentBgColor)
        recyclerView.setBackgroundColor(contentBgColor)

        toolbar.setBackgroundColor(navBarColor)
        toolbar.setTitleTextColor(titleColor)
        toolbar.setTitleTextAppearance(this, android.R.style.TextAppearance_Medium)
        toolbar.navigationIcon?.setTint(iconColor)

        overlayContainer.background = createOverlayBackground(
            Color.argb((0.85f * 255).toInt(), 0, 0, 0)
        )
        pageIndicator.setTextColor(Color.WHITE)
        overlayContainer.alpha = 1f

        window.statusBarColor = navBarColor
        window.navigationBarColor = navBarColor

        WindowInsetsControllerCompat(window, window.decorView).apply {
            isAppearanceLightStatusBars = true
            isAppearanceLightNavigationBars = true
        }

        tintToolbarMenuIcons(iconColor)
    }

    private fun dp(value: Int): Int =
        TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value.toFloat(),
            resources.displayMetrics
        ).roundToInt()

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

    private inner class PdfPageAdapter : RecyclerView.Adapter<PdfPageAdapter.PageVH>() {
        override fun getItemCount(): Int = pageCount

        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): PageVH {
            val iv = ImageView(parent.context).apply {
                layoutParams = RecyclerView.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
                adjustViewBounds = true
                scaleType = ImageView.ScaleType.FIT_CENTER
                contentDescription = "PDF page preview"
                setBackgroundColor(Color.TRANSPARENT)
            }
            return PageVH(iv)
        }

        override fun onBindViewHolder(holder: PageVH, position: Int) {
            val renderer = pdfRenderer ?: return
            var page: PdfRenderer.Page? = null
            try {
                page = renderer.openPage(position)
                // Render at higher resolution for better quality when zooming
                val targetWidth = (holder.itemView.width
                    .takeIf { it > 0 }
                    ?: recyclerView.width
                    .takeIf { it > 0 }
                    ?: resources.displayMetrics.widthPixels)

                // Render at 2x resolution for better zoom quality
                val renderScale = 2.0f
                val bmpWidth = (page.width * renderScale).toInt().coerceAtLeast(1)
                val bmpHeight = (page.height * renderScale).toInt().coerceAtLeast(1)

                val bitmap = Bitmap.createBitmap(bmpWidth, bmpHeight, Bitmap.Config.ARGB_8888)
                page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)

                // Set explicit height for the view based on aspect ratio
                val aspectRatio = page.height.toFloat() / page.width.toFloat()
                val viewHeight = (targetWidth * aspectRatio).toInt()
                holder.image.layoutParams = RecyclerView.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    viewHeight
                )

                holder.image.setImageBitmap(bitmap)
            } catch (_: Throwable) {
                // Ignore rendering errors for now
            } finally {
                try { page?.close() } catch (_: Throwable) {}
            }
        }

        override fun onViewRecycled(holder: PageVH) {
            super.onViewRecycled(holder)
            holder.image.setImageDrawable(null)
        }

        inner class PageVH(val image: ImageView) : RecyclerView.ViewHolder(image)
    }
    
}
