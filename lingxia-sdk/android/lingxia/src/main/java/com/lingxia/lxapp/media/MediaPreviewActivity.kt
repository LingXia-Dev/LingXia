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
import androidx.appcompat.app.AppCompatActivity
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import androidx.core.view.WindowCompat
import android.util.TypedValue
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.graphics.Typeface
import android.widget.TextView
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.net.URL
import java.lang.ref.WeakReference
import java.io.File
import android.graphics.drawable.BitmapDrawable
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView

class MediaPreviewActivity : AppCompatActivity() {
    private lateinit var viewPager: ViewPager2
    private lateinit var previewAdapter: PreviewPagerAdapter
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
            val normalizedUri = normalizeUri(payload.url)
            val coverUri = payload.coverUrl?.takeIf { it.isNotEmpty() }?.let { normalizeUri(it) }?.takeUnless { it == Uri.EMPTY }
            PreviewItem(
                uri = normalizedUri,
                mediaType = MediaPreviewType.fromInt(payload.type),
                coverUri = coverUri
            )
        }

        val root = FrameLayout(this).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
        }

        totalItems = items.size

        previewAdapter = PreviewPagerAdapter(items) { finishWithAnimation() }

        viewPager = ViewPager2(this).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            adapter = previewAdapter
        }
        previewAdapter.attachToViewPager(viewPager)
        root.addView(viewPager)

        val topBar = createTopBar(totalItems)
        root.addView(topBar)

        updateIndicator(0)
        previewAdapter.onPageSelected(0)
        pageChangeCallback = object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                updateIndicator(position)
                previewAdapter.onPageSelected(position)
            }
        }
        pageChangeCallback?.let { viewPager.registerOnPageChangeCallback(it) }

        setContentView(root)
    }

    override fun onDestroy() {
        super.onDestroy()
        pageChangeCallback?.let { viewPager.unregisterOnPageChangeCallback(it) }
        previewAdapter.release()
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
    val uri: Uri,
    val mediaType: MediaPreviewType,
    val coverUri: Uri?
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

private fun normalizeUri(raw: String?): Uri {
    if (raw.isNullOrBlank()) return Uri.EMPTY
    val trimmed = raw.trim()
    return try {
        val parsed = Uri.parse(trimmed)
        if (parsed.scheme.isNullOrEmpty()) {
            val file = File(trimmed)
            Uri.fromFile(file)
        } else {
            parsed
        }
    } catch (_: Exception) {
        Uri.EMPTY
    }
}

private fun isLocalUri(uri: Uri): Boolean {
    if (uri == Uri.EMPTY) return false
    val scheme = uri.scheme
    return scheme.isNullOrEmpty() || scheme.equals("file", true) || scheme.equals("content", true)
}

private class PreviewPagerAdapter(
    private val items: List<PreviewItem>,
    private val onDismiss: () -> Unit
) : RecyclerView.Adapter<PreviewPagerAdapter.MediaViewHolder>() {

    private var viewPager: ViewPager2? = null
    private var currentPosition: Int = RecyclerView.NO_POSITION

    fun attachToViewPager(pager: ViewPager2) {
        viewPager = pager
    }

    fun release() {
        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView
        recyclerView?.let { rv ->
            for (index in 0 until rv.childCount) {
                val holder = rv.getChildViewHolder(rv.getChildAt(index))
                if (holder is MediaViewHolder) {
                    holder.onHidden()
                    holder.reset()
                }
            }
        }
        viewPager = null
        currentPosition = RecyclerView.NO_POSITION
    }

    fun onPageSelected(position: Int) {
        if (position == currentPosition) return

        val recyclerView = viewPager?.getChildAt(0) as? RecyclerView

        if (currentPosition != RecyclerView.NO_POSITION) {
            recyclerView
                ?.findViewHolderForAdapterPosition(currentPosition)
                ?.let { holder ->
                    if (holder is MediaViewHolder) holder.onHidden()
                }
        }

        currentPosition = position

        recyclerView
            ?.findViewHolderForAdapterPosition(position)
            ?.let { holder ->
                if (holder is MediaViewHolder) holder.onVisible()
            }
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
        if (position == currentPosition) {
            holder.onVisible()
        }
    }

    override fun getItemCount(): Int = items.size

    override fun onViewRecycled(holder: MediaViewHolder) {
        holder.onHidden()
        holder.reset()
        super.onViewRecycled(holder)
    }

    class MediaViewHolder(
        private val container: FrameLayout,
        private val onZoomStateChanged: (Boolean) -> Unit,
        private val onDismiss: () -> Unit
    ) : RecyclerView.ViewHolder(container) {
        private var currentLoader: Future<*>? = null
        private var currentPlayer: ExoPlayer? = null
        private var currentPlayerView: PlayerView? = null
        private var isVideoItem: Boolean = false
        private var videoThumbnail: ImageView? = null
        private var videoProgress: ProgressBar? = null
        private var controlsVisible: Boolean = false

        fun bind(item: PreviewItem) {
            reset()
            container.removeAllViews()
            when (item.mediaType) {
                MediaPreviewType.VIDEO -> bindVideo(item)
                MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> bindImage(item)
            }
        }

        fun reset() {
            currentLoader?.cancel(true)
            currentLoader = null
            currentPlayerView?.player = null
            currentPlayerView = null
            currentPlayer?.let { player ->
                player.clearMediaItems()
                player.release()
            }
            currentPlayer = null
            isVideoItem = false
            videoThumbnail = null
            videoProgress = null
            controlsVisible = false
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

            val uri = item.uri

            if (uri == Uri.EMPTY) {
                zoomImageView.setImageResource(android.R.drawable.ic_dialog_alert)
                return
            }

            if (isLocalUri(uri)) {
                zoomImageView.setImageURI(uri)
            } else {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.load(uri.toString(), zoomImageView, progressBar)
            }
        }

        private fun bindVideo(item: PreviewItem) {
            val context = container.context
            val playerView = PlayerView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                useController = true
                setControllerShowTimeoutMs(3_000)
                setShowBuffering(PlayerView.SHOW_BUFFERING_WHEN_PLAYING)
                setControllerAutoShow(false)
            }

            val thumbnailView = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                scaleType = ImageView.ScaleType.CENTER_CROP
                setBackgroundColor(Color.BLACK)
                visibility = View.GONE
            }

            val progressBar = ProgressBar(context).apply {
                layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.CENTER)
                visibility = View.VISIBLE
            }

            val mediaUri = item.uri

            if (mediaUri == Uri.EMPTY) {
                progressBar.visibility = View.GONE
                thumbnailView.visibility = View.VISIBLE
                thumbnailView.setImageResource(android.R.drawable.ic_dialog_alert)
                return
            }

            val coverUri = item.coverUri
            if (coverUri != null && coverUri != Uri.EMPTY) {
                if (isLocalUri(coverUri)) {
                    thumbnailView.visibility = View.VISIBLE
                    thumbnailView.setImageURI(coverUri)
                } else {
                    thumbnailView.visibility = View.VISIBLE
                    currentLoader = ImageLoader.load(coverUri.toString(), thumbnailView, null)
                }
            }

            val player = ExoPlayer.Builder(context).build().apply {
                repeatMode = Player.REPEAT_MODE_ALL
                setMediaItem(MediaItem.fromUri(mediaUri))
                playWhenReady = false
                addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        when (playbackState) {
                            Player.STATE_READY -> {
                                videoProgress?.visibility = View.GONE
                                videoThumbnail?.visibility = View.GONE
                            }
                            Player.STATE_ENDED -> {
                                videoProgress?.visibility = View.GONE
                            }
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        videoProgress?.visibility = View.GONE
                        videoThumbnail?.visibility = View.VISIBLE
                    }
                })
                prepare()
            }

            playerView.player = player
            playerView.setControllerVisibilityListener(
                PlayerView.ControllerVisibilityListener { visibility ->
                    controlsVisible = visibility == View.VISIBLE
                }
            )

            playerView.setOnClickListener {
                if (controlsVisible) {
                    // Already visible, let Playerview auto-hide on timeout instead of forcing hide
                    return@setOnClickListener
                }
                playerView.showController()
            }

            container.addView(playerView)
            container.addView(thumbnailView)
            container.addView(progressBar)
            container.setOnClickListener(null)
            currentPlayer = player
            currentPlayerView = playerView
            isVideoItem = true
            videoThumbnail = thumbnailView
            videoProgress = progressBar
            playerView.hideController()
            controlsVisible = false
        }

        fun onVisible() {
            if (isVideoItem) {
                val player = currentPlayer ?: return
                when (player.playbackState) {
                    Player.STATE_READY -> {
                        videoProgress?.visibility = View.GONE
                        videoThumbnail?.visibility = View.GONE
                    }
                    Player.STATE_BUFFERING -> {
                        videoProgress?.visibility = View.VISIBLE
                        videoThumbnail?.visibility = View.GONE
                    }
                    else -> {
                        videoProgress?.visibility = View.VISIBLE
                    }
                }
                player.playWhenReady = true
                player.play()
                currentPlayerView?.hideController()
                controlsVisible = false
            }
        }

        fun onHidden() {
            if (isVideoItem) {
                currentPlayer?.pause()
                currentPlayer?.seekTo(0)
                videoThumbnail?.visibility = View.VISIBLE
                videoProgress?.visibility = View.GONE
                currentPlayerView?.hideController()
            }
        }
    }

    companion object ImageLoader {
        private val executor = Executors.newCachedThreadPool()

        fun load(url: String, target: ImageView, progressBar: ProgressBar?): Future<*> {
            val imageRef = WeakReference(target)
            val progressRef = progressBar?.let { WeakReference(it) }
            return executor.submit {
                try {
                    URL(url).openStream().use { stream ->
                        val drawable = BitmapDrawable.createFromStream(stream, "media")
                        val imageView = imageRef.get()
                        if (imageView != null && drawable != null) {
                            imageView.post {
                                progressRef?.get()?.visibility = View.GONE
                                imageView.setImageDrawable(drawable)
                            }
                        }
                    }
                } catch (e: Exception) {
                    val imageView = imageRef.get()
                    imageView?.post {
                        progressRef?.get()?.visibility = View.GONE
                        imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                    }
                }
            }
        }
    }
}
