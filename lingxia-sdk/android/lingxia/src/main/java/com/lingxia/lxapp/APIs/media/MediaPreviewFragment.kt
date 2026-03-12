package com.lingxia.lxapp.APIs.media

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.graphics.Typeface
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.exifinterface.media.ExifInterface
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import com.lingxia.lxapp.NativeApi
import java.io.File
import java.io.ByteArrayOutputStream
import java.lang.ref.WeakReference
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.Future
import kotlin.math.max
import org.json.JSONObject

class MediaPreviewFragment : Fragment() {
    private enum class PreviewAdvance {
        MANUAL,
        NEXT,
        LOOP;

        companion object {
            fun fromRaw(value: String?): PreviewAdvance = when (value?.trim()?.lowercase()) {
                "next" -> NEXT
                "loop" -> LOOP
                else -> MANUAL
            }
        }
    }

    private var viewPager: ViewPager2? = null
    private var previewAdapter: PreviewPagerAdapter? = null
    private var indicatorText: TextView? = null
    private var pageChangeCallback: ViewPager2.OnPageChangeCallback? = null
    private var totalItems: Int = 0
    private var windowUiSnapshot: ImmersiveWindowUi.Snapshot? = null
    private var previewItems: List<PreviewItem> = emptyList()
    private var callbackId: Long = 0L
    private var currentIndex: Int = 0
    private var currentPagerPosition: Int = 0
    private var advance: PreviewAdvance = PreviewAdvance.MANUAL
    private var showIndexIndicator: Boolean = false
    private var finished = false
    private val mainHandler = Handler(Looper.getMainLooper())
    private var imageAutoRunnable: Runnable? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        previewItems = readPreviewItems()
        callbackId = arguments?.getLong(ARG_CALLBACK_ID, 0L) ?: 0L
        advance = PreviewAdvance.fromRaw(arguments?.getString(ARG_ADVANCE))
        currentIndex = clampIndex(arguments?.getInt(ARG_START_INDEX, 0) ?: 0)
        currentPagerPosition = initialPagerPosition(currentIndex)
        showIndexIndicator = arguments?.getBoolean(ARG_SHOW_INDEX_INDICATOR, false) ?: false
    }

    override fun onCreateView(
        inflater: android.view.LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val context = requireContext()
        val root = FrameLayout(context).apply {
            setBackgroundColor(Color.BLACK)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
        }

        if (previewItems.isEmpty()) {
            root.post { finishPreview("error") }
            return root
        }

        totalItems = previewItems.size

        val adapter = PreviewPagerAdapter(
            items = previewItems,
            loopEnabled = shouldUseLoopPager(),
            loopSingleItemVideo = shouldLoopSingleItemVideo(),
            onDismiss = { finishPreview("manual") },
            onVideoTerminal = { position, terminal -> onVideoTerminal(position, terminal) }
        )
        previewAdapter = adapter

        val pager = ViewPager2(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            this.adapter = adapter
            setCurrentItem(currentPagerPosition, false)
        }
        adapter.attachToViewPager(pager)
        viewPager = pager
        root.addView(pager)

        val topBar = createTopBar(context, totalItems)
        root.addView(topBar)

        updateIndicator(currentIndex)
        val callback = object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                handlePageSelected(position)
            }
        }
        pageChangeCallback = callback
        pager.registerOnPageChangeCallback(callback)
        root.post {
            if (!finished) {
                handlePageSelected(currentPagerPosition)
            }
        }

        return root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        if (previewItems.isEmpty()) return

        val activity = requireActivity()
        val window = activity.window
        if (windowUiSnapshot == null) {
            windowUiSnapshot = ImmersiveWindowUi.capture(window)
        }
        ImmersiveWindowUi.apply(window, keepScreenOn = false)

        activity.onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    finishPreview("manual")
                }
            }
        )
    }

    override fun onDestroyView() {
        if (!finished) {
            finished = true
            sendPreviewResult("interrupted")
        }
        clearAutoRunnables()
        cleanupPreviewResources()
        super.onDestroyView()
    }

    private fun sendPreviewResult(reason: String) {
        if (callbackId <= 0L) return
        val result = JSONObject()
            .put("reason", reason)
            .put("lastIndex", currentIndex)
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    private fun cleanupPreviewResources() {
        pageChangeCallback?.let { callback ->
            viewPager?.unregisterOnPageChangeCallback(callback)
        }
        pageChangeCallback = null
        previewAdapter?.release()
        // Force ViewPager to detach/recycle pages so video players are destroyed immediately.
        viewPager?.adapter = null
        previewAdapter = null
        viewPager = null
        indicatorText = null

        activity?.window?.let { window ->
            windowUiSnapshot?.let { snapshot ->
                ImmersiveWindowUi.restore(window, snapshot)
            }
        }
        windowUiSnapshot = null
    }

    private fun clearAutoRunnables() {
        imageAutoRunnable?.let(mainHandler::removeCallbacks)
        imageAutoRunnable = null
    }

    private fun createTopBar(context: Context, itemCount: Int): View {
        val topContainer = FrameLayout(context).apply {
            setBackgroundColor(Color.TRANSPARENT)
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.TOP)
        }

        val dpMargin = TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            16f,
            context.resources.displayMetrics
        ).toInt()
        val topOffset = statusBarHeight(context) + dpMargin

        indicatorText = TextView(context).apply {
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            textAlignment = View.TEXT_ALIGNMENT_CENTER
            setShadowLayer(4f, 0f, 0f, Color.parseColor("#66000000"))
            layoutParams = FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply {
                gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL
                setMargins(dpMargin, topOffset, dpMargin, dpMargin)
            }
            visibility = if (showIndexIndicator && itemCount > 0) View.VISIBLE else View.GONE
        }

        indicatorText?.let(topContainer::addView)
        return topContainer
    }

    private fun updateIndicator(position: Int) {
        if (!showIndexIndicator || totalItems <= 0) {
            indicatorText?.visibility = View.GONE
            return
        }
        indicatorText?.visibility = View.VISIBLE
        indicatorText?.text = "${position + 1}/$totalItems"
    }

    private fun readPreviewItems(): List<PreviewItem> {
        val args = arguments ?: return emptyList()
        @Suppress("DEPRECATION")
        val raw = args.getSerializable(ARG_PAYLOADS)
        val payloads: List<PreviewMediaPayload> = when (raw) {
            is Array<*> -> raw.filterIsInstance<PreviewMediaPayload>()
            is List<*> -> raw.filterIsInstance<PreviewMediaPayload>()
            else -> emptyList()
        }

        return payloads.map { payload ->
            val normalizedUri = normalizeUri(payload.path)
            val coverUri = payload.coverPath
                ?.takeIf { it.isNotEmpty() }
                ?.let { normalizeUri(it) }
                ?.takeUnless { it == Uri.EMPTY }
            PreviewItem(
                uri = normalizedUri,
                mediaType = MediaPreviewType.fromInt(payload.type),
                coverUri = coverUri,
                rotate = payload.rotate,
                objectFit = payload.objectFit?.let { LxMediaObjectFit.fromString(it) },
                durationMs = payload.durationMs?.takeIf { it > 0L }
            )
        }
    }

    private fun handlePageSelected(position: Int) {
        currentPagerPosition = position
        currentIndex = realIndexFor(position)
        updateIndicator(currentIndex)
        previewAdapter?.onPageSelected(position)
        scheduleCurrentItemBehavior()
    }

    private fun scheduleCurrentItemBehavior() {
        clearAutoRunnables()
        val item = previewItems.getOrNull(currentIndex) ?: return
        if (item.mediaType == MediaPreviewType.VIDEO) {
            return
        }

        val timeoutMs = item.durationMs
        if (timeoutMs != null && timeoutMs > 0L && advance != PreviewAdvance.MANUAL) {
            val runnable = Runnable {
                imageAutoRunnable = null
                advanceFromCurrentItem()
            }
            imageAutoRunnable = runnable
            mainHandler.postDelayed(runnable, timeoutMs)
        }
    }

    private fun onVideoTerminal(position: Int, terminal: String) {
        if (finished || position != currentPagerPosition) return
        clearAutoRunnables()
        when (terminal) {
            "error" -> finishPreview("error")
            else -> advanceFromCurrentItem()
        }
    }

    private fun advanceFromCurrentItem() {
        when (advance) {
            PreviewAdvance.MANUAL -> Unit
            PreviewAdvance.NEXT -> {
                if (currentIndex < previewItems.lastIndex) {
                    viewPager?.setCurrentItem(currentPagerPosition + 1, true)
                } else {
                    finishPreview("completed")
                }
            }
            PreviewAdvance.LOOP -> {
                if (previewItems.isEmpty()) {
                    finishPreview("completed")
                    return
                }
                if (previewItems.size == 1) {
                    scheduleCurrentItemBehavior()
                    return
                }
                viewPager?.setCurrentItem(currentPagerPosition + 1, true)
            }
        }
    }

    private fun finishPreview(reason: String) {
        if (finished) return
        finished = true
        clearAutoRunnables()
        sendPreviewResult(reason)
        if (!isAdded) {
            cleanupPreviewResources()
            return
        }

        val fm = parentFragmentManager
        if (fm.isStateSaved) {
            fm.beginTransaction()
                .remove(this)
                .commitAllowingStateLoss()
            return
        }

        fm.popBackStack(TAG, FragmentManager.POP_BACK_STACK_INCLUSIVE)
    }

    private fun clampIndex(position: Int): Int {
        if (previewItems.isEmpty()) return 0
        return position.coerceIn(0, previewItems.lastIndex)
    }

    private fun shouldUseLoopPager(): Boolean {
        return advance == PreviewAdvance.LOOP && previewItems.size > 1
    }

    private fun shouldLoopSingleItemVideo(): Boolean {
        return advance == PreviewAdvance.LOOP && previewItems.size == 1
    }

    private fun realIndexFor(position: Int): Int {
        if (previewItems.isEmpty()) return 0
        if (!shouldUseLoopPager()) return clampIndex(position)
        val size = previewItems.size
        val normalized = position % size
        return if (normalized >= 0) normalized else normalized + size
    }

    private fun initialPagerPosition(startIndex: Int): Int {
        if (!shouldUseLoopPager()) {
            return clampIndex(startIndex)
        }
        val size = previewItems.size
        val midpoint = Int.MAX_VALUE / 2
        val base = midpoint - (midpoint % size)
        return base + clampIndex(startIndex)
    }

    companion object {
        private const val ARG_PAYLOADS = "arg_payloads"
        private const val ARG_START_INDEX = "arg_start_index"
        private const val ARG_ADVANCE = "arg_advance"
        private const val ARG_SHOW_INDEX_INDICATOR = "arg_show_index_indicator"
        private const val ARG_CALLBACK_ID = "arg_callback_id"
        private const val TAG = "MediaPreviewOverlay"

        fun show(
            activity: AppCompatActivity,
            payloads: Array<PreviewMediaPayload>,
            startIndex: Int,
            advance: String,
            showIndexIndicator: Boolean,
            callbackId: Long
        ) {
            val fm = activity.supportFragmentManager
            (fm.findFragmentByTag(TAG) as? MediaPreviewFragment)?.finishPreview("interrupted")

            val fragment = MediaPreviewFragment().apply {
                arguments = Bundle().apply {
                    putSerializable(ARG_PAYLOADS, ArrayList(payloads.toList()))
                    putInt(ARG_START_INDEX, startIndex)
                    putString(ARG_ADVANCE, advance)
                    putBoolean(ARG_SHOW_INDEX_INDICATOR, showIndexIndicator)
                    putLong(ARG_CALLBACK_ID, callbackId)
                }
            }

            fm.beginTransaction()
                .setCustomAnimations(
                    android.R.anim.fade_in,
                    android.R.anim.fade_out,
                    android.R.anim.fade_in,
                    android.R.anim.fade_out
                )
                .add(android.R.id.content, fragment, TAG)
                .addToBackStack(TAG)
                .commitAllowingStateLoss()
            fm.executePendingTransactions()
        }

        fun close(activity: AppCompatActivity, callbackId: Long) {
            val fragment = activity.supportFragmentManager.findFragmentByTag(TAG) as? MediaPreviewFragment
            if (fragment != null && fragment.callbackId == callbackId) {
                fragment.finishPreview("interrupted")
            }
        }
    }
}

private data class PreviewItem(
    val uri: Uri,
    val mediaType: MediaPreviewType,
    val coverUri: Uri?,
    val rotate: Int?,
    val objectFit: LxMediaObjectFit?,
    val durationMs: Long?
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

private fun statusBarHeight(context: Context): Int {
    val resourceId = context.resources.getIdentifier("status_bar_height", "dimen", "android")
    return if (resourceId > 0) context.resources.getDimensionPixelSize(resourceId) else 0
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
    private val loopEnabled: Boolean,
    private val loopSingleItemVideo: Boolean,
    private val onDismiss: () -> Unit,
    private val onVideoTerminal: (Int, String) -> Unit
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
        return MediaViewHolder(
            container = container,
            onZoomStateChanged = { zoomed -> viewPager?.isUserInputEnabled = !zoomed },
            onDismiss = onDismiss,
            onVideoTerminal = { _ -> }
        )
    }

    override fun onBindViewHolder(holder: MediaViewHolder, position: Int) {
        holder.setVideoTerminalHandler { terminal ->
            val adapterPosition = holder.bindingAdapterPosition
            if (adapterPosition != RecyclerView.NO_POSITION) {
                onVideoTerminal(adapterPosition, terminal)
            }
        }
        holder.bind(items[realIndexFor(position)], loopSingleItemVideo)
        if (position == currentPosition) {
            holder.onVisible()
        }
    }

    override fun getItemCount(): Int = if (loopEnabled) Int.MAX_VALUE else items.size

    override fun onViewAttachedToWindow(holder: MediaViewHolder) {
        super.onViewAttachedToWindow(holder)
        if (holder.bindingAdapterPosition == currentPosition) {
            holder.onVisible()
        }
    }

    override fun onViewRecycled(holder: MediaViewHolder) {
        holder.onHidden()
        holder.reset()
        super.onViewRecycled(holder)
    }

    private fun realIndexFor(position: Int): Int {
        if (items.isEmpty()) return 0
        if (!loopEnabled) return position.coerceIn(0, items.lastIndex)
        val normalized = position % items.size
        return if (normalized >= 0) normalized else normalized + items.size
    }

    class MediaViewHolder(
        private val container: FrameLayout,
        private val onZoomStateChanged: (Boolean) -> Unit,
        private val onDismiss: () -> Unit,
        private var onVideoTerminal: (String) -> Unit
    ) : RecyclerView.ViewHolder(container) {
        private var currentLoader: Future<*>? = null
        private var currentMediaPlayer: LxMediaPlayer? = null
        private var boundItem: PreviewItem? = null
        private var loopVideoPlayback: Boolean = false
        private var videoPosterView: ImageView? = null
        private var gatePlaybackOnPosterReady: Boolean = false
        private var posterReadyForPlayback: Boolean = true
        private var pendingPlayUntilPosterReady: Boolean = false
        private var posterLoadGeneration: Long = 0L

        fun bind(item: PreviewItem, loopVideoPlayback: Boolean) {
            reset()
            container.removeAllViews()
            boundItem = item
            this.loopVideoPlayback = loopVideoPlayback
            when (item.mediaType) {
                MediaPreviewType.VIDEO -> bindVideoPlaceholder(item, preparePoster = true)
                MediaPreviewType.IMAGE, MediaPreviewType.UNKNOWN -> bindImage(item)
            }
        }

        fun setVideoTerminalHandler(handler: (String) -> Unit) {
            onVideoTerminal = handler
        }

        fun reset() {
            currentLoader?.cancel(true)
            currentLoader = null
            currentMediaPlayer?.detach()
            currentMediaPlayer = null
            boundItem = null
            videoPosterView = null
            gatePlaybackOnPosterReady = false
            posterReadyForPlayback = true
            pendingPlayUntilPosterReady = false
            posterLoadGeneration += 1L
            clearImageReferences(container)
            container.removeAllViews()
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
            zoomImageView.setTapToDismissEnabled(true)
            zoomImageView.setDismissListener(onDismiss)
            zoomImageView.setOnScaleStateListener { zoomed -> onZoomStateChanged(zoomed) }
            zoomImageView.setPreviewRotationDegrees(item.rotate)
            zoomImageView.setPreviewObjectFit(item.objectFit)

            val uri = item.uri

            if (uri == Uri.EMPTY) {
                zoomImageView.setImageResource(android.R.drawable.ic_dialog_alert)
                return
            }

            if (isLocalUri(uri)) {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadLocal(context, uri, zoomImageView, progressBar)
            } else {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadRemote(uri.toString(), zoomImageView, progressBar)
            }
        }

        private fun bindVideoPlaceholder(item: PreviewItem, preparePoster: Boolean) {
            currentLoader?.cancel(true)
            currentLoader = null
            val generation = posterLoadGeneration + 1L
            posterLoadGeneration = generation

            val context = container.context
            val mediaUri = item.uri

            val posterView = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                scaleType = posterScaleType(item.objectFit)
            }
            videoPosterView = posterView
            container.addView(
                posterView,
                FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            )

            gatePlaybackOnPosterReady = false
            posterReadyForPlayback = true
            pendingPlayUntilPosterReady = false

            if (mediaUri == Uri.EMPTY) {
                posterView.setImageResource(android.R.drawable.ic_dialog_alert)
                posterView.scaleType = ImageView.ScaleType.CENTER
                return
            }

            if (!preparePoster) {
                return
            }

            val coverUri = item.coverUri
            if (coverUri != null && coverUri != Uri.EMPTY) {
                currentLoader = if (isLocalUri(coverUri)) {
                    ImageLoader.loadLocal(context, coverUri, posterView, null)
                } else {
                    ImageLoader.loadRemote(coverUri.toString(), posterView, null)
                }
                return
            }

            if (isLocalUri(mediaUri)) {
                gatePlaybackOnPosterReady = true
                posterReadyForPlayback = false
                currentLoader = ImageLoader.loadVideoFirstFrame(
                    context = context,
                    uri = mediaUri,
                    target = posterView,
                    onComplete = { onVideoPosterReady(generation) }
                )
            }
        }

        private fun posterScaleType(objectFit: LxMediaObjectFit?): ImageView.ScaleType {
            return when (objectFit ?: LxMediaObjectFit.CONTAIN) {
                LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
                LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
                LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
            }
        }

        private fun onVideoPosterReady(generation: Long) {
            if (generation != posterLoadGeneration) return
            posterReadyForPlayback = true
            if (pendingPlayUntilPosterReady) {
                pendingPlayUntilPosterReady = false
                currentMediaPlayer?.play()
            }
        }

        private fun hideVideoPoster() {
            videoPosterView?.visibility = View.GONE
        }

        private fun ensureVideoPlayer(item: PreviewItem) {
            if (currentMediaPlayer != null) return
            val context = container.context
            val mediaUri = item.uri
            if (mediaUri == Uri.EMPTY) {
                container.removeAllViews()
                showVideoError(context)
                onVideoTerminal("error")
                return
            }

            // Create LxMediaPlayer for video playback
            val mediaPlayer = LxMediaPlayer(
                context,
                eventSink = { payload ->
                    if (payload["event"] == "playing") {
                        container.post { hideVideoPoster() }
                    }
                },
                typedEventSink = { event ->
                    when (event) {
                        is LxMediaEvent.Ended -> onVideoTerminal("ended")
                        is LxMediaEvent.Error -> onVideoTerminal("error")
                        else -> Unit
                    }
                }
            )
            mediaPlayer.setShowCloseButton(true)
            mediaPlayer.setShowFullscreenButton(false)
            mediaPlayer.setShowLoadingIndicator(false)
            mediaPlayer.setSuppressAutoShowControls(true)  // Prevent auto-showing controls in preview mode
            mediaPlayer.setCloseRequestListener {
                onDismiss()
            }

            // Configure the player
            val config = LxMediaPlayerConfig(
                src = mediaUri.toString(),
                poster = item.coverUri?.toString(),
                autoplay = false,
                loop = loopVideoPlayback,
                controls = true,
                objectFit = item.objectFit ?: LxMediaObjectFit.CONTAIN,
                rotateDegrees = item.rotate
            )
            mediaPlayer.update(config)

            // Keep poster view on top until player starts rendering.
            container.addView(
                mediaPlayer.view,
                0,
                FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            )
            videoPosterView?.bringToFront()

            currentMediaPlayer = mediaPlayer
        }

        private fun showVideoError(context: Context) {
            val errorView = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                setImageResource(android.R.drawable.ic_dialog_alert)
                scaleType = ImageView.ScaleType.CENTER
            }
            container.addView(errorView)
        }

        private fun clearImageReferences(view: View?) {
            view ?: return
            if (view is ImageView) {
                view.setImageDrawable(null)
            }
            if (view is ViewGroup) {
                for (index in 0 until view.childCount) {
                    clearImageReferences(view.getChildAt(index))
                }
            }
        }

        fun onVisible() {
            val item = boundItem
            if (item?.mediaType == MediaPreviewType.VIDEO) {
                ensureVideoPlayer(item)
                if (gatePlaybackOnPosterReady && !posterReadyForPlayback) {
                    pendingPlayUntilPosterReady = true
                } else {
                    pendingPlayUntilPosterReady = false
                    currentMediaPlayer?.play()
                }
            }
        }

        fun onHidden() {
            val item = boundItem
            if (item?.mediaType == MediaPreviewType.VIDEO) {
                currentLoader?.cancel(true)
                currentLoader = null
                currentMediaPlayer?.pause()
                currentMediaPlayer?.seek(0.0)
                currentMediaPlayer?.exitFullscreen()
                currentMediaPlayer?.detach()
                currentMediaPlayer = null
                container.removeAllViews()
                bindVideoPlaceholder(item, preparePoster = false)
            }
        }
    }

    companion object ImageLoader {
        private const val MAX_REMOTE_IMAGE_BYTES = 12 * 1024 * 1024
        private const val MAX_LOCAL_CONTENT_IMAGE_BYTES = 12 * 1024 * 1024

        private val executor = Executors.newFixedThreadPool(2) { runnable ->
            Thread(runnable, "LingXiaPreviewImage").apply { isDaemon = true }
        }

        fun loadRemote(
            url: String,
            target: ImageView,
            progressBar: ProgressBar?,
            onComplete: (() -> Unit)? = null
        ): Future<*> {
            val imageRef = WeakReference(target)
            val progressRef = progressBar?.let { WeakReference(it) }
            val metrics = target.context.resources.displayMetrics
            val targetW = metrics?.widthPixels ?: 1080
            val targetH = metrics?.heightPixels ?: 1920
            return executor.submit {
                try {
                    val bytes = downloadBytesWithLimit(url, MAX_REMOTE_IMAGE_BYTES)
                    val bitmap = bytes?.let { decodeSampledBitmap(it, targetW, targetH) }
                    val imageView = imageRef.get()
                    if (imageView != null) {
                        imageView.post {
                            progressRef?.get()?.visibility = View.GONE
                            if (bitmap != null) {
                                imageView.setImageBitmap(bitmap)
                            } else {
                                imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                            }
                            onComplete?.invoke()
                        }
                    }
                } catch (e: Exception) {
                    val imageView = imageRef.get()
                    imageView?.post {
                        progressRef?.get()?.visibility = View.GONE
                        imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                        onComplete?.invoke()
                    }
                }
            }
        }

        fun loadLocal(
            context: Context,
            uri: Uri,
            target: ImageView,
            progressBar: ProgressBar?,
            onComplete: (() -> Unit)? = null
        ): Future<*> {
            val appContext = context.applicationContext
            val imageRef = WeakReference(target)
            val progressRef = progressBar?.let { WeakReference(it) }
            return executor.submit {
                val imageView = imageRef.get()
                val metrics = imageView?.context?.resources?.displayMetrics
                val targetW = metrics?.widthPixels ?: 1080
                val targetH = metrics?.heightPixels ?: 1920
                val bitmap = decodeLocalBitmap(appContext, uri, targetW, targetH)
                imageView?.post {
                    progressRef?.get()?.visibility = View.GONE
                    if (bitmap != null) {
                        imageView.setImageBitmap(bitmap)
                    } else {
                        imageView.setImageResource(android.R.drawable.ic_dialog_alert)
                    }
                    onComplete?.invoke()
                }
            }
        }

        fun loadVideoFirstFrame(
            context: Context,
            uri: Uri,
            target: ImageView,
            onComplete: (() -> Unit)? = null
        ): Future<*> {
            val appContext = context.applicationContext
            val imageRef = WeakReference(target)
            return executor.submit {
                val imageView = imageRef.get()
                val metrics = imageView?.context?.resources?.displayMetrics
                val targetW = metrics?.widthPixels ?: 1080
                val targetH = metrics?.heightPixels ?: 1920
                val bitmap = decodeLocalVideoFirstFrame(appContext, uri, targetW, targetH)
                imageView?.post {
                    if (bitmap != null) {
                        imageView.setImageBitmap(bitmap)
                    }
                    onComplete?.invoke()
                }
            }
        }

        private fun decodeLocalVideoFirstFrame(
            context: Context,
            uri: Uri,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap? {
            val retriever = MediaMetadataRetriever()
            return try {
                when {
                    uri.scheme.isNullOrEmpty() || uri.scheme.equals("file", true) -> {
                        val path = uri.path ?: return null
                        retriever.setDataSource(path)
                    }
                    uri.scheme.equals("content", true) -> {
                        retriever.setDataSource(context, uri)
                    }
                    else -> return null
                }
                val frame = retriever.getFrameAtTime(0L, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
                    ?: retriever.getFrameAtTime(-1L)
                    ?: return null
                downscaleBitmapIfNeeded(frame, targetWidth, targetHeight)
            } catch (_: Exception) {
                null
            } finally {
                try {
                    retriever.release()
                } catch (_: Exception) {
                }
            }
        }

        private fun downscaleBitmapIfNeeded(
            bitmap: Bitmap,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap {
            val maxEdge = max(targetWidth, targetHeight).coerceAtLeast(720)
            val longest = max(bitmap.width, bitmap.height)
            if (longest <= maxEdge) return bitmap

            val scale = maxEdge.toFloat() / longest.toFloat()
            val scaledWidth = max(1, (bitmap.width * scale).toInt())
            val scaledHeight = max(1, (bitmap.height * scale).toInt())
            return try {
                Bitmap.createScaledBitmap(bitmap, scaledWidth, scaledHeight, true).also { scaled ->
                    if (scaled != bitmap) {
                        bitmap.recycle()
                    }
                }
            } catch (_: Exception) {
                bitmap
            }
        }

        private fun decodeLocalBitmap(
            context: Context,
            uri: Uri,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap? {
            return try {
                when {
                    uri.scheme.isNullOrEmpty() || uri.scheme.equals("file", true) -> {
                        val path = uri.path ?: return null
                        decodeFileWithSample(path, targetWidth, targetHeight)
                    }
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.P -> {
                        val source = ImageDecoder.createSource(context.contentResolver, uri)
                        ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                            val sample = calculateSample(info.size.width, info.size.height, targetWidth, targetHeight)
                            val width = max(1, info.size.width / sample)
                            val height = max(1, info.size.height / sample)
                            decoder.setTargetSize(width, height)
                            decoder.isMutableRequired = false
                            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                        }
                    }
                    else -> {
                        context.contentResolver.openInputStream(uri)?.use { stream ->
                            val bytes = readBytesWithLimit(stream, MAX_LOCAL_CONTENT_IMAGE_BYTES)
                            bytes?.let { decodeSampledBitmap(it, targetWidth, targetHeight) }
                        }
                    }
                }
            } catch (_: Exception) {
                null
            }
        }

        private fun decodeFileWithSample(path: String, targetWidth: Int, targetHeight: Int): Bitmap? {
            val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(path, opts)
            if (opts.outWidth <= 0 || opts.outHeight <= 0) {
                return BitmapFactory.decodeFile(
                    path,
                    BitmapFactory.Options().apply {
                        inPreferredConfig = Bitmap.Config.RGB_565
                    }
                )
            }

            val sample = calculateSample(opts.outWidth, opts.outHeight, targetWidth, targetHeight)
            val decodeOpts = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.RGB_565
            }

            val decoded = BitmapFactory.decodeFile(path, decodeOpts) ?: return null
            val orientation = try {
                ExifInterface(path).getAttributeInt(
                    ExifInterface.TAG_ORIENTATION,
                    ExifInterface.ORIENTATION_NORMAL
                )
            } catch (_: Exception) {
                ExifInterface.ORIENTATION_UNDEFINED
            }

            val matrix = Matrix()
            when (orientation) {
                ExifInterface.ORIENTATION_ROTATE_90 -> matrix.postRotate(90f)
                ExifInterface.ORIENTATION_ROTATE_180 -> matrix.postRotate(180f)
                ExifInterface.ORIENTATION_ROTATE_270 -> matrix.postRotate(270f)
                ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> matrix.postScale(-1f, 1f)
                ExifInterface.ORIENTATION_FLIP_VERTICAL -> matrix.postScale(1f, -1f)
            }

            return if (!matrix.isIdentity) {
                try {
                    Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, matrix, true)
                        .also { if (it != decoded) decoded.recycle() }
                } catch (_: Exception) {
                    decoded
                }
            } else {
                decoded
            }
        }

        private fun downloadBytesWithLimit(url: String, maxBytes: Int): ByteArray? {
            var connection: HttpURLConnection? = null
            return try {
                connection = (URL(url).openConnection() as HttpURLConnection).apply {
                    connectTimeout = 5_000
                    readTimeout = 10_000
                    instanceFollowRedirects = true
                    doInput = true
                }
                connection.connect()
                if (connection.responseCode !in 200..299) {
                    null
                } else {
                    connection.inputStream.use { readBytesWithLimit(it, maxBytes) }
                }
            } finally {
                connection?.disconnect()
            }
        }

        private fun readBytesWithLimit(input: java.io.InputStream, limitBytes: Int): ByteArray? {
            val output = ByteArrayOutputStream(limitBytes.coerceAtMost(16 * 1024))
            val buffer = ByteArray(8 * 1024)
            var total = 0
            while (true) {
                val read = input.read(buffer)
                if (read <= 0) break
                total += read
                if (total > limitBytes) return null
                output.write(buffer, 0, read)
            }
            return output.toByteArray()
        }

        private fun decodeSampledBitmap(
            bytes: ByteArray,
            targetWidth: Int,
            targetHeight: Int
        ): Bitmap? {
            if (bytes.isEmpty()) return null
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                return BitmapFactory.decodeByteArray(
                    bytes,
                    0,
                    bytes.size,
                    BitmapFactory.Options().apply {
                        inPreferredConfig = Bitmap.Config.RGB_565
                    }
                )
            }
            val sample = calculateSample(bounds.outWidth, bounds.outHeight, targetWidth, targetHeight)
            val opts = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.RGB_565
            }
            return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
        }

        private fun calculateSample(
            width: Int,
            height: Int,
            targetWidth: Int,
            targetHeight: Int
        ): Int {
            var sample = 1
            var outWidth = width
            var outHeight = height
            while (outWidth / 2 >= targetWidth || outHeight / 2 >= targetHeight) {
                outWidth /= 2
                outHeight /= 2
                sample *= 2
            }
            return sample.coerceAtLeast(1)
        }
    }
}
