package com.lingxia.lxapp.media

import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.VideoView
import androidx.appcompat.app.AppCompatActivity
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import androidx.core.view.WindowCompat
import android.widget.MediaController
import android.util.TypedValue
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.graphics.Typeface
import android.view.MotionEvent
import android.widget.TextView
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.net.URL
import java.lang.ref.WeakReference
import android.graphics.drawable.BitmapDrawable

class MediaPreviewActivity : AppCompatActivity() {
    private lateinit var viewPager: ViewPager2
    private var totalItems: Int = 0
    private var indicatorText: TextView? = null
    private var pageChangeCallback: ViewPager2.OnPageChangeCallback? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = Color.BLACK
        window.navigationBarColor = Color.BLACK

        val payloads: Array<PreviewMediaPayload>? = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            intent.getSerializableExtra(EXTRA_PAYLOADS, Array<PreviewMediaPayload>::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getSerializableExtra(EXTRA_PAYLOADS) as? Array<PreviewMediaPayload>
        }

        if (payloads == null || payloads.isEmpty()) {
            finish()
            return
        }

        val items = payloads.map { payload ->
            PreviewItem(
                url = payload.url,
                mediaType = MediaPreviewType.fromInt(payload.type),
                coverUrl = payload.coverUrl?.takeIf { it.isNotEmpty() }
            )
        }

        val root = FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
        }

        totalItems = items.size

        val previewAdapter = PreviewPagerAdapter(items) { finishWithAnimation() }

        viewPager = ViewPager2(this).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            adapter = previewAdapter
        }
        previewAdapter.attachToViewPager(viewPager)
        root.addView(viewPager)

        val topBar = createTopBar(totalItems)
        root.addView(topBar)

        updateIndicator(0)
        pageChangeCallback = object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                updateIndicator(position)
            }
        }
        pageChangeCallback?.let { viewPager.registerOnPageChangeCallback(it) }

        setContentView(root)
    }

    override fun onDestroy() {
        super.onDestroy()
        pageChangeCallback?.let { viewPager.unregisterOnPageChangeCallback(it) }
    }

    private fun createTopBar(itemCount: Int): View {
        val topContainer = FrameLayout(this).apply {
            setBackgroundColor(Color.TRANSPARENT)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.TOP)
        }

        indicatorText = TextView(this).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            textAlignment = View.TEXT_ALIGNMENT_CENTER
            setShadowLayer(4f, 0f, 0f, Color.parseColor("#66000000"))
            layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply {
                gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
                val margin = TypedValue.applyDimension(
                    TypedValue.COMPLEX_UNIT_DIP,
                    16f,
                    resources.displayMetrics
                ).toInt()
                val top = statusBarHeight() + margin
                setMargins(margin, top, margin, margin)
            }
            visibility = if (itemCount > 1) View.VISIBLE else View.GONE
        }

        indicatorText?.let(topContainer::addView)
        return topContainer
    }

    private fun finishWithAnimation() {
        finish()
        overridePendingTransition(android.R.anim.fade_in, android.R.anim.fade_out)
    }

    private fun statusBarHeight(): Int {
        val resourceId = resources.getIdentifier("status_bar_height", "dimen", "android")
        return if (resourceId > 0) resources.getDimensionPixelSize(resourceId) else 0
    }

    private fun updateIndicator(position: Int) {
        if (totalItems <= 1) {
            indicatorText?.visibility = View.GONE
            return
        }
        indicatorText?.visibility = View.VISIBLE
        indicatorText?.text = "${position + 1}/$totalItems"
    }

    companion object {
        private const val EXTRA_PAYLOADS = "extra_payloads"

        fun launch(
            activity: android.app.Activity,
            payloads: Array<PreviewMediaPayload>
        ) {
            val intent = android.content.Intent(activity, MediaPreviewActivity::class.java).apply {
                putExtra(EXTRA_PAYLOADS, payloads as java.io.Serializable)
            }
            activity.startActivity(intent)
            activity.overridePendingTransition(android.R.anim.fade_in, android.R.anim.fade_out)
        }
    }
}

private data class PreviewItem(
    val url: String,
    val mediaType: MediaPreviewType,
    val coverUrl: String?
)

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

private class PreviewPagerAdapter(
    private val items: List<PreviewItem>,
    private val onDismiss: () -> Unit
) : RecyclerView.Adapter<PreviewPagerAdapter.MediaViewHolder>() {

    private var viewPager: ViewPager2? = null

    fun attachToViewPager(pager: ViewPager2) {
        viewPager = pager
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): MediaViewHolder {
        val container = FrameLayout(parent.context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            setBackgroundColor(Color.BLACK)
        }
        return MediaViewHolder(container, { zoomed -> viewPager?.isUserInputEnabled = !zoomed }, onDismiss)
    }

    override fun onBindViewHolder(holder: MediaViewHolder, position: Int) {
        holder.bind(items[position])
    }

    override fun getItemCount(): Int = items.size

    class MediaViewHolder(
        private val container: FrameLayout,
        private val onZoomStateChanged: (Boolean) -> Unit,
        private val onDismiss: () -> Unit
    ) : RecyclerView.ViewHolder(container) {
        private var currentLoader: Future<*>? = null

        fun bind(item: PreviewItem) {
            container.removeAllViews()
            currentLoader?.cancel(true)
            when (item.mediaType) {
                MediaPreviewType.VIDEO -> bindVideo(item)
                MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> bindImage(item)
            }
        }

        private fun bindImage(item: PreviewItem) {
            val context = container.context
            val zoomImageView = ZoomImageView(context)
            val progressBar = ProgressBar(context).apply {
                layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.CENTER)
                visibility = View.GONE
            }

            container.addView(zoomImageView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            container.addView(progressBar)
            zoomImageView.setDismissListener(onDismiss)
            zoomImageView.setOnScaleStateListener { zoomed -> onZoomStateChanged(zoomed) }

            val uri = Uri.parse(item.url)
            if (uri.scheme.isNullOrEmpty() || uri.scheme.equals("file", true) || uri.scheme.equals("content", true)) {
                zoomImageView.setImageURI(uri)
            } else {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.load(item.url, zoomImageView, progressBar)
            }
        }

        private fun bindVideo(item: PreviewItem) {
            val context = container.context
            val videoView = VideoView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
            }

            val mediaController = MediaController(context)
            mediaController.setAnchorView(videoView)
            videoView.setMediaController(mediaController)
            videoView.setVideoURI(Uri.parse(item.url))
            videoView.setOnPreparedListener { mediaPlayer ->
                mediaPlayer.isLooping = true
                videoView.start()
            }

            container.addView(videoView)
            container.setOnClickListener { onDismiss() }
            videoView.setOnClickListener { onDismiss() }

        }
    }

    companion object ImageLoader {
        private val executor = Executors.newCachedThreadPool()

        fun load(url: String, zoomImageView: ZoomImageView, progressBar: ProgressBar): Future<*> {
            val imageRef = WeakReference(zoomImageView)
            val progressRef = WeakReference(progressBar)
            return executor.submit {
                try {
                    val stream = URL(url).openStream()
                    val drawable = BitmapDrawable.createFromStream(stream, "media")
                    val img = imageRef.get()
                    val progress = progressRef.get()
                    if (img != null && drawable != null) {
                        img.post {
                            progress?.visibility = View.GONE
                            img.setImageDrawable(drawable)
                        }
                    }
                } catch (e: Exception) {
                    val img = imageRef.get()
                    val progress = progressRef.get()
                    img?.post {
                        progress?.visibility = View.GONE
                        img.setImageResource(android.R.drawable.ic_dialog_alert)
                    }
                }
            }
        }
    }
}
