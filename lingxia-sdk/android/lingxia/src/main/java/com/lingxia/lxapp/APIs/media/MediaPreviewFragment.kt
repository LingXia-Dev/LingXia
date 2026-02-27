package com.lingxia.lxapp.APIs.media

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.graphics.Typeface
import android.graphics.drawable.BitmapDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
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
import androidx.core.view.WindowCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import androidx.exifinterface.media.ExifInterface
import java.io.File
import java.lang.ref.WeakReference
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.Future
import kotlin.math.max

class MediaPreviewFragment : Fragment() {
    private var viewPager: ViewPager2? = null
    private var previewAdapter: PreviewPagerAdapter? = null
    private var indicatorText: TextView? = null
    private var pageChangeCallback: ViewPager2.OnPageChangeCallback? = null
    private var totalItems: Int = 0
    private var originalStatusBarColor: Int? = null
    private var originalNavigationBarColor: Int? = null
    private var dismissed = false

    private var previewItems: List<PreviewItem> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        previewItems = readPreviewItems()
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
            root.post { dismissOverlay() }
            return root
        }

        totalItems = previewItems.size

        val adapter = PreviewPagerAdapter(previewItems) { dismissOverlay() }
        previewAdapter = adapter

        val pager = ViewPager2(context).apply {
            layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            this.adapter = adapter
        }
        adapter.attachToViewPager(pager)
        viewPager = pager
        root.addView(pager)

        val topBar = createTopBar(context, totalItems)
        root.addView(topBar)

        updateIndicator(0)
        adapter.onPageSelected(0)
        val callback = object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                updateIndicator(position)
                adapter.onPageSelected(position)
            }
        }
        pageChangeCallback = callback
        pager.registerOnPageChangeCallback(callback)

        return root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        if (previewItems.isEmpty()) return

        val activity = requireActivity()
        originalStatusBarColor = activity.window.statusBarColor
        originalNavigationBarColor = activity.window.navigationBarColor
        WindowCompat.setDecorFitsSystemWindows(activity.window, false)
        activity.window.statusBarColor = Color.BLACK
        activity.window.navigationBarColor = Color.BLACK

        activity.onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    dismissOverlay()
                }
            }
        )
    }

    override fun onDestroyView() {
        super.onDestroyView()
        pageChangeCallback?.let { callback ->
            viewPager?.unregisterOnPageChangeCallback(callback)
        }
        pageChangeCallback = null
        previewAdapter?.release()
        previewAdapter = null
        viewPager = null
        indicatorText = null

        activity?.let { host ->
            originalStatusBarColor?.let { host.window.statusBarColor = it }
            originalNavigationBarColor?.let { host.window.navigationBarColor = it }
            WindowCompat.setDecorFitsSystemWindows(host.window, true)
        }
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
            visibility = if (itemCount > 1) View.VISIBLE else View.GONE
        }

        indicatorText?.let(topContainer::addView)
        return topContainer
    }

    private fun updateIndicator(position: Int) {
        if (totalItems <= 1) {
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
                coverUri = coverUri
            )
        }
    }

    private fun dismissOverlay() {
        if (dismissed) return
        dismissed = true
        parentFragmentManager.popBackStack(TAG, FragmentManager.POP_BACK_STACK_INCLUSIVE)
    }

    companion object {
        private const val ARG_PAYLOADS = "arg_payloads"
        private const val TAG = "MediaPreviewOverlay"

        fun show(activity: AppCompatActivity, payloads: Array<PreviewMediaPayload>) {
            val fm = activity.supportFragmentManager
            if (fm.findFragmentByTag(TAG) != null) return

            val fragment = MediaPreviewFragment().apply {
                arguments = Bundle().apply {
                    putSerializable(ARG_PAYLOADS, ArrayList(payloads.toList()))
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
    private val onDismiss: () -> Unit
) : RecyclerView.Adapter<PreviewPagerAdapter.MediaViewHolder>() {

    private var viewPager: ViewPager2? = null
    private var currentPosition: Int = 0  // Start at 0 for initial page

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
        private var currentMediaPlayer: LxMediaPlayer? = null
        private var isVideoItem: Boolean = false

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
            currentMediaPlayer?.detach()
            currentMediaPlayer = null
            isVideoItem = false
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
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadLocal(context, uri, zoomImageView, progressBar)
            } else {
                progressBar.visibility = View.VISIBLE
                currentLoader = ImageLoader.loadRemote(uri.toString(), zoomImageView, progressBar)
            }
        }

        private fun bindVideo(item: PreviewItem) {
            val context = container.context
            val mediaUri = item.uri

            if (mediaUri == Uri.EMPTY) {
                val errorView = ImageView(context).apply {
                    layoutParams = FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
                    setBackgroundColor(Color.BLACK)
                    setImageResource(android.R.drawable.ic_dialog_alert)
                    scaleType = ImageView.ScaleType.CENTER
                }
                container.addView(errorView)
                return
            }

            // Create LxMediaPlayer for video playback
            val mediaPlayer = LxMediaPlayer(context, eventSink = { /* events ignored in preview */ })
            mediaPlayer.setShowCloseButton(true)
            mediaPlayer.setShowFullscreenButton(false)
            mediaPlayer.setSuppressAutoShowControls(true)  // Prevent auto-showing controls in preview mode
            mediaPlayer.setCloseRequestListener {
                onDismiss()
            }

            // Configure the player
            val config = LxMediaPlayerConfig(
                src = mediaUri.toString(),
                poster = item.coverUri?.toString(),
                autoplay = false,
                loop = true,
                controls = true,
                objectFit = LxMediaObjectFit.CONTAIN
            )
            mediaPlayer.update(config)

            // Add player view to container
            container.addView(
                mediaPlayer.view,
                FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT)
            )

            currentMediaPlayer = mediaPlayer
            isVideoItem = true

            // Auto-enter fullscreen to mirror video component fullscreen behavior in preview
            container.post {
                if (!mediaPlayer.isFullscreen()) {
                    mediaPlayer.enterFullscreen()
                }
            }
        }

        fun onVisible() {
            if (isVideoItem) {
                currentMediaPlayer?.play()
            }
        }

        fun onHidden() {
            if (isVideoItem) {
                currentMediaPlayer?.pause()
                currentMediaPlayer?.seek(0.0)
                currentMediaPlayer?.exitFullscreen()
            }
        }
    }

    companion object ImageLoader {
        private val executor = Executors.newCachedThreadPool()

        fun loadRemote(url: String, target: ImageView, progressBar: ProgressBar?): Future<*> {
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

        fun loadLocal(
            context: Context,
            uri: Uri,
            target: ImageView,
            progressBar: ProgressBar?
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
                }
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
                        }
                    }
                    else -> {
                        context.contentResolver.openInputStream(uri)?.use { stream ->
                            BitmapFactory.decodeStream(stream)
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
                return BitmapFactory.decodeFile(path)
            }

            val sample = calculateSample(opts.outWidth, opts.outHeight, targetWidth, targetHeight)
            val decodeOpts = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.ARGB_8888
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
