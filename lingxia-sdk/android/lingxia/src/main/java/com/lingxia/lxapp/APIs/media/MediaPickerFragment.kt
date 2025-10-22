package com.lingxia.lxapp.APIs.media

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.util.Size
import android.util.Log
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import com.lingxia.lxapp.util.WindowInsetsUtils
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import com.lingxia.lxapp.NativeApi
import org.json.JSONObject
import android.view.animation.AccelerateDecelerateInterpolator
import android.graphics.drawable.GradientDrawable

/**
 * Unified custom media picker UI (grid multi-select, confirm), attached to current Activity.
 */
class MediaPickerFragment : Fragment() {
    companion object {
        private const val TAG = "LingXia.MediaPickerFrag"
        private const val ARG_MAX_COUNT = "arg_max_count"
        private const val ARG_CALLBACK_ID = "arg_callback_id"
        private const val ARG_MODE = "arg_mode" // images | videos | mix
        private const val ARG_ALLOW_CAMERA = "arg_allow_camera"
        private const val ARG_MAX_DURATION = "arg_max_duration"
        private const val ARG_CAMERA_FACING = "arg_camera_facing"
        private const val CAMERA_ITEM_TYPE = "__camera__"
        private val CAMERA_PLACEHOLDER_URI: Uri = Uri.parse("lxapp-camera://capture")
        // Sentinel album IDs for pseudo entries
        private const val ALBUM_ALL_VIDEOS_ID: Long = -10001L
        private const val ALBUM_ALL_IMAGES_ID: Long = -10002L

        fun start(
            activity: AppCompatActivity,
            maxCount: Int,
            callbackId: Long,
            mode: String,
            allowCamera: Boolean,
            maxDurationSeconds: Int,
            cameraFacing: Int
        ) {
            val frag = MediaPickerFragment().apply {
                arguments = Bundle().apply {
                    putInt(ARG_MAX_COUNT, maxCount)
                    putLong(ARG_CALLBACK_ID, callbackId)
                    putString(ARG_MODE, mode)
                    putBoolean(ARG_ALLOW_CAMERA, allowCamera)
                    putInt(ARG_MAX_DURATION, maxDurationSeconds)
                    putInt(ARG_CAMERA_FACING, cameraFacing)
                }
            }
            val fm = activity.supportFragmentManager
            fm.beginTransaction().add(android.R.id.content, frag, TAG).commitAllowingStateLoss()
            fm.executePendingTransactions()
        }

        // Lightweight picker for in-app flows (e.g., ScanCode selecting one image)
        fun pick(
            activity: AppCompatActivity,
            maxCount: Int = 1,
            mode: String = "images",
            allowCamera: Boolean = false,
            onPicked: (List<Uri>) -> Unit
        ) {
            val frag = MediaPickerFragment().apply {
                arguments = Bundle().apply {
                    putInt(ARG_MAX_COUNT, maxCount)
                    putLong(ARG_CALLBACK_ID, 0L)
                    putString(ARG_MODE, mode)
                    putBoolean(ARG_ALLOW_CAMERA, allowCamera)
                    putInt(ARG_MAX_DURATION, -1)
                    putInt(ARG_CAMERA_FACING, -1)
                }
                resultListener = onPicked
            }
            val fm = activity.supportFragmentManager
            fm.beginTransaction().add(android.R.id.content, frag, TAG).commitAllowingStateLoss()
            fm.executePendingTransactions()
        }
    }

    private val callbackId: Long
        get() = arguments?.getLong(ARG_CALLBACK_ID) ?: 0L

    private val selectedMode: String
        get() = arguments?.getString(ARG_MODE) ?: "mix"

    private val allowCamera: Boolean
        get() = arguments?.getBoolean(ARG_ALLOW_CAMERA) ?: false

    private val maxCaptureDuration: Int
        get() = arguments?.getInt(ARG_MAX_DURATION) ?: -1

    private val cameraFacingPref: Int
        get() = arguments?.getInt(ARG_CAMERA_FACING) ?: -1

    private var recycler: RecyclerView? = null
    private var sendBtn: TextView? = null
    private var sendBtnBackground: GradientDrawable? = null
    private var selectionSummaryView: TextView? = null
    private var maxSelectable: Int = 1
    private val techBlueColor: Int = Color.parseColor("#1677FF")
    private val lightTechBlueColor: Int = Color.parseColor("#AFCBFF")
    private val disabledBlueColor: Int = Color.parseColor("#80A6D9")
    private val selected = linkedMapOf<Uri, Boolean>()
    private var allItems: List<GridItem> = emptyList()
    private val itemsIndex = HashMap<Uri, GridItem>()
    private var albums: List<AlbumItem> = emptyList()
    private var currentAlbumId: Long? = null // null => all
    private var albumMenuContainer: FrameLayout? = null
    private var albumListView: RecyclerView? = null
    private var albumSelectorView: LinearLayout? = null
    private var pendingOnLoaded: ((List<GridItem>) -> Unit)? = null
    private var resultListener: ((List<Uri>) -> Unit)? = null

    // System Photo Picker launchers (Android 13+)
    private var pickSingleLauncher: ActivityResultLauncher<PickVisualMediaRequest>? = null
    private var pickMultipleLauncher: ActivityResultLauncher<PickVisualMediaRequest>? = null
    private var pendingSystemPicker: Boolean = false

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { results ->
        val granted = results.values.all { it }
        if (granted) {
            loadMedia { list ->
                pendingOnLoaded?.invoke(list)
                pendingOnLoaded = null
            }
        } else {
            sendFailure("Permission denied")
            removeSelf()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Ensure maxSelectable is available for launcher registration
        maxSelectable = (arguments?.getInt(ARG_MAX_COUNT) ?: 1).coerceAtLeast(1)

        // Register native Photo Picker launchers (Android 13+)
        if (Build.VERSION.SDK_INT >= 33) {
            pickSingleLauncher = registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
                if (uri == null) {
                    sendCancel(); removeSelf()
                } else {
                    deliverPickedUris(listOf(uri))
                }
            }
            if (maxSelectable > 1) {
                pickMultipleLauncher = registerForActivityResult(
                    ActivityResultContracts.PickMultipleVisualMedia(maxSelectable)
                ) { uris ->
                    if (uris == null || uris.isEmpty()) {
                        sendCancel(); removeSelf()
                    } else {
                        deliverPickedUris(uris)
                    }
                }
            } else {
                pickMultipleLauncher = null
            }
        }
    }

    override fun onStart() {
        super.onStart()
        if (pendingSystemPicker) {
            pendingSystemPicker = false
            // Post to ensure we are fully in STARTED state
            view?.post { launchSystemPhotoPicker() }
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        val context = requireContext()
        val root = FrameLayout(context).apply {
            setBackgroundColor(Color.parseColor("#F7F8FA"))
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val argLimit = arguments?.getInt(ARG_MAX_COUNT) ?: 1
        maxSelectable = argLimit.coerceAtLeast(1)

        // Top bar
        val topBar = FrameLayout(context).apply {
            setBackgroundColor(Color.WHITE)
            val h = dp(context, 56) + statusBarHeight()
            setPadding(dp(context, 16), statusBarHeight(), dp(context, 16), dp(context, 12))
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                h
            ).apply { gravity = Gravity.TOP }
            ViewCompat.setElevation(this, dp(context, 2).toFloat())
        }
        // Custom drawn close "X" for better visual integration
        val backBtn = CloseXView(context).apply {
            isClickable = true
            isFocusable = true
            layoutParams = FrameLayout.LayoutParams(dp(context, 48), dp(context, 48)).apply {
                gravity = Gravity.START or Gravity.CENTER_VERTICAL
            }
            setOnClickListener { sendCancel(); removeSelf() }
            contentDescription = "关闭"
        }
        // Center album selector pill
        val selector = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(dp(context, 12), dp(context, 6), dp(context, 10), dp(context, 6))
            background = GradientDrawable().apply {
                cornerRadius = dp(context, 20).toFloat()
                setColor(Color.parseColor("#F2F3F5"))
            }
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply { gravity = Gravity.CENTER }
            isClickable = true
            isFocusable = true
            setOnClickListener { toggleAlbumMenu() }
        }
        val selectorText = TextView(context).apply {
            setTextColor(Color.parseColor("#1F1F1F"))
            // Do not set default text to avoid flicker; will set after album chosen
            text = ""
            textSize = 16f
        }
        val arrowCircle = FrameLayout(context).apply {
            background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(Color.parseColor("#E6E6E6"))
            }
            layoutParams = LinearLayout.LayoutParams(dp(context, 18), dp(context, 18)).apply {
                setMargins(dp(context, 4), 0, 0, 0)
                gravity = Gravity.CENTER_VERTICAL
            }
        }
        val arrow = ArrowView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setDirectionUp(false)
        }
        arrowCircle.addView(arrow)
        selector.addView(selectorText)
        selector.addView(arrowCircle)
        albumSelectorView = selector
        topBar.addView(selector)
        topBar.addView(backBtn)
        root.addView(topBar)

        // Recycler grid
        val rv = RecyclerView(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ).apply {
                topMargin = dp(context, 56) + statusBarHeight()
                bottomMargin = dp(context, 64)
            }
            setBackgroundColor(Color.parseColor("#F7F8FA"))
        }
        val spanCount = 4
        rv.layoutManager = GridLayoutManager(context, spanCount)
        rv.addItemDecoration(HairlineDividerDecoration(context, 0.5f, Color.parseColor("#E5E6EB")))
        val adapter = MediaGridAdapter(
            context,
            onMediaClick = { item -> toggleSelection(item) },
            onCameraClick = { launchCameraFromPicker() }
        )
        rv.adapter = adapter
        root.addView(rv)

        // Bottom bar
        val bottom = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.WHITE)
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                dp(context, 64)
            ).apply { gravity = Gravity.BOTTOM }
            setPadding(dp(context, 16), dp(context, 10), dp(context, 16), dp(context, 14))
            gravity = Gravity.CENTER_VERTICAL
        }
        val summaryView = TextView(context).apply {
            text = "已选 0/$maxSelectable"
            setTextColor(disabledBlueColor)
            textSize = 15f
            typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }
        selectionSummaryView = summaryView

        val sendBackground = GradientDrawable().apply {
            cornerRadius = dp(context, 18).toFloat()
            setColor(lightTechBlueColor)
        }
        sendBtnBackground = sendBackground
        val send = TextView(context).apply {
            text = "完成"
            setTextColor(Color.WHITE)
            textSize = 16f
            typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
            background = sendBackground
            gravity = Gravity.CENTER
            setPadding(dp(context, 18), dp(context, 8), dp(context, 18), dp(context, 8))
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            setOnClickListener { confirmSelection() }
        }

        bottom.addView(summaryView)
        bottom.addView(send)
        root.addView(bottom)

        // Lift bottom bar and content above system navigation bar using helper
        WindowInsetsUtils.applyBottomMargin(root, bottom, 0)
        WindowInsetsUtils.applyBottomMargin(root, rv, dp(context, 64))

        recycler = rv
        sendBtn = send

        // Album dropdown container (overlay)
        val albumContainer = FrameLayout(context).apply {
            setBackgroundColor(Color.parseColor("#33000000"))
            visibility = View.GONE
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }
        val albumList = RecyclerView(context).apply {
            setBackgroundColor(Color.WHITE)
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.TOP
                topMargin = dp(context, 56) + statusBarHeight()
            }
            layoutManager = androidx.recyclerview.widget.LinearLayoutManager(context)
        }
        albumContainer.addView(albumList)
        root.addView(albumContainer)
        albumMenuContainer = albumContainer
        albumListView = albumList

        ensurePermissionsThenLoad { items ->
            applyLoadedItems(items)
        }

        return root
    }

    // Permissions and loading
    private fun ensurePermissionsThenLoad(onLoaded: (List<GridItem>) -> Unit) {
        val act = activity ?: return
        // Restricted access on Android 13+: use system Photo Picker only
        if (Build.VERSION.SDK_INT >= 33 && !hasMediaPermission(act)) {
            // Defer launching until Fragment is STARTED to avoid lifecycle crash
            pendingSystemPicker = true
            return
        }
        if (!hasMediaPermission(act)) {
            pendingOnLoaded = onLoaded
            permissionLauncher.launch(getNeededPermissions())
        } else {
            loadMedia(onLoaded)
        }
    }

    private fun applyLoadedItems(items: List<GridItem>) {
        allItems = items
        itemsIndex.clear()
        for (it in items) itemsIndex[it.uri] = it
        // build albums from items (group by bucket)
        val albumMap = LinkedHashMap<Long, AlbumItemBuilder>()
        for (item in items) {
            val b = albumMap.getOrPut(item.bucketId ?: -1L) {
                AlbumItemBuilder(item.bucketId, item.bucketName ?: "")
            }
            b.count += 1
            if (b.coverUri == null) b.coverUri = item.uri
        }
        val systemAlbums = ArrayList<AlbumItem>()
        for ((_, builder) in albumMap) {
            if (builder.id != null) {
                systemAlbums.add(AlbumItem(builder.id, builder.name, builder.count, builder.coverUri))
            }
        }

        // Inject pseudo albums at top based on mode
        val lowerMode = selectedMode.lowercase()
        val allCount = items.size
        val firstAllCover = items.firstOrNull()?.uri
        val videos = items.filter { it.fileType == "video" }
        val images = items.filter { it.fileType == "image" }
        val firstVideoCover = videos.firstOrNull()?.uri
        val firstImageCover = images.firstOrNull()?.uri

        val albumList = ArrayList<AlbumItem>()
        when (lowerMode) {
            "videos" -> {
                albumList.add(AlbumItem(null, "所有视频", allCount, firstAllCover))
                albumList.addAll(systemAlbums)
            }
            "images" -> {
                albumList.add(AlbumItem(null, "所有图片", allCount, firstAllCover))
                albumList.addAll(systemAlbums)
            }
            else -> {
                albumList.add(AlbumItem(null, "图片和视频", allCount, firstAllCover))
                albumList.add(AlbumItem(ALBUM_ALL_VIDEOS_ID, "所有视频", videos.size, firstVideoCover))
                albumList.addAll(systemAlbums)
            }
        }
        albums = albumList
        setupAlbumList()
        val defaultTitle = when (lowerMode) {
            "videos" -> "所有视频"
            "images" -> "所有图片"
            else -> "图片和视频"
        }
        currentAlbumId = if (lowerMode == "mix") null else null
        (albumSelectorView?.getChildAt(0) as? TextView)?.text = defaultTitle
        (recycler?.adapter as? ListAdapter<GridItem, *>)?.submitList(filterByAlbum(allItems))
        albumMenuContainer?.visibility = View.GONE
        applySendButtonStyle(0)
    }

    private fun launchSystemPhotoPicker() {
        if (Build.VERSION.SDK_INT < 33) {
            // Fallback safeguard; on <33 we keep our own picker
            loadMedia { items ->
                allItems = items
                recycler?.adapter?.let { (it as? ListAdapter<GridItem, *>)?.submitList(filterByAlbum(items)) }
            }
            return
        }
        val mediaType = when (selectedMode.lowercase()) {
            "videos" -> ActivityResultContracts.PickVisualMedia.VideoOnly
            "images" -> ActivityResultContracts.PickVisualMedia.ImageOnly
            else -> ActivityResultContracts.PickVisualMedia.ImageAndVideo
        }
        val request = PickVisualMediaRequest.Builder()
            .setMediaType(mediaType)
            .build()
        if (maxSelectable <= 1) {
            pickSingleLauncher?.launch(request)
        } else {
            pickMultipleLauncher?.launch(request)
        }
    }

    private fun deliverPickedUris(uris: List<Uri>) {
        val listener = resultListener
        if (listener != null) {
            listener.invoke(uris)
            activity?.runOnUiThread { removeSelf() }
            return
        }
        val cbId = callbackId
        Thread {
            try {
                val arr = org.json.JSONArray()
                for (uri in uris.take(maxSelectable)) {
                    val typeStr = detectFileType(uri)
                    val obj = org.json.JSONObject().apply {
                        put("uri", uri.toString())
                        put("fileType", typeStr)
                    }
                    arr.put(obj)
                }
                NativeApi.onCallback(cbId, true, arr.toString())
            } catch (e: Exception) {
                NativeApi.onCallback(cbId, false, (e.message ?: "build result failed"))
            } finally {
                activity?.runOnUiThread { removeSelf() }
            }
        }.start()
    }

    private fun detectFileType(uri: Uri): String {
        return try {
            val mime = requireContext().contentResolver.getType(uri) ?: ""
            if (mime.startsWith("video")) "video" else "image"
        } catch (_: Exception) {
            "image"
        }
    }

    private fun hasMediaPermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= 33) {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_IMAGES) == PackageManager.PERMISSION_GRANTED &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_VIDEO) == PackageManager.PERMISSION_GRANTED
        } else {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        }
    }

    private fun getNeededPermissions(): Array<String> {
        return if (Build.VERSION.SDK_INT >= 33) {
            arrayOf(Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO)
        } else {
            arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }
    }

    data class GridItem(
        val uri: Uri,
        val fileType: String,
        val durationSec: Double,
        val dateAdded: Long,
        val bucketId: Long?,
        val bucketName: String?
    )

    data class AlbumItem(val id: Long?, val name: String, val count: Int, val coverUri: Uri?)
    private class AlbumItemBuilder(val id: Long?, val name: String) { var count = 0; var coverUri: Uri? = null }

    private fun loadMedia(onLoaded: (List<GridItem>) -> Unit) {
        Thread {
            val out = mutableListOf<GridItem>()
            try {
                val resolver = requireContext().contentResolver
                val collection = MediaStore.Files.getContentUri("external")
                val projection = arrayOf(
                    MediaStore.Files.FileColumns._ID,
                    MediaStore.Files.FileColumns.MEDIA_TYPE,
                    MediaStore.Files.FileColumns.DATE_ADDED,
                    MediaStore.Video.VideoColumns.DURATION,
                    MediaStore.Images.Media.BUCKET_ID,
                    MediaStore.Images.Media.BUCKET_DISPLAY_NAME
                )
                val sel = when (selectedMode.lowercase()) {
                    "videos" -> "${MediaStore.Files.FileColumns.MEDIA_TYPE}=${MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO}"
                    "images" -> "${MediaStore.Files.FileColumns.MEDIA_TYPE}=${MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE}"
                    else -> "${MediaStore.Files.FileColumns.MEDIA_TYPE} in (${MediaStore.Files.FileColumns.MEDIA_TYPE_IMAGE}, ${MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO})"
                }
                val sort = "${MediaStore.Files.FileColumns.DATE_ADDED} DESC"
                resolver.query(collection, projection, sel, null, sort)?.use { c ->
                    val idIdx = c.getColumnIndexOrThrow(MediaStore.Files.FileColumns._ID)
                    val typeIdx = c.getColumnIndexOrThrow(MediaStore.Files.FileColumns.MEDIA_TYPE)
                    val dateIdx = c.getColumnIndexOrThrow(MediaStore.Files.FileColumns.DATE_ADDED)
                    val durIdx = c.getColumnIndexOrThrow(MediaStore.Video.VideoColumns.DURATION)
                    val bIdIdx = c.getColumnIndexOrThrow(MediaStore.Images.Media.BUCKET_ID)
                    val bNameIdx = c.getColumnIndexOrThrow(MediaStore.Images.Media.BUCKET_DISPLAY_NAME)
                    while (c.moveToNext()) {
                        val id = c.getLong(idIdx)
                        val mediaType = c.getInt(typeIdx)
                        val date = c.getLong(dateIdx)
                        val durMs = c.getLong(durIdx)
                        val bId = if (!c.isNull(bIdIdx)) c.getLong(bIdIdx) else null
                        val bName = if (!c.isNull(bNameIdx)) c.getString(bNameIdx) else null
                        val isVideo = mediaType == MediaStore.Files.FileColumns.MEDIA_TYPE_VIDEO
                        val uri = if (isVideo) ContentUris.withAppendedId(MediaStore.Video.Media.EXTERNAL_CONTENT_URI, id)
                        else ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, id)
                        out.add(
                            GridItem(
                                uri,
                                if (isVideo) "video" else "image",
                                if (isVideo) durMs / 1000.0 else 0.0,
                                date,
                                bId,
                                bName
                            )
                        )
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "loadMedia failed: ${e.message}", e)
            }
            activity?.runOnUiThread { onLoaded(out) }
        }.start()
    }

    private fun toggleSelection(item: GridItem) {
        if (item.fileType == CAMERA_ITEM_TYPE) {
            launchCameraFromPicker()
            return
        }
        if (selected.containsKey(item.uri)) {
            selected.remove(item.uri)
        } else {
            selected[item.uri] = true
            if (selected.size > maxSelectable) {
                val first = selected.keys.firstOrNull()
                if (first != null) selected.remove(first)
            }
        }
        updateSelectionUI()
    }

    private fun updateSelectionUI() {
        val count = selected.size
        applySendButtonStyle(count)
        (recycler?.adapter as? MediaGridAdapter)?.setSelected(selected.keys)
    }

    private fun launchCameraFromPicker() {
        if (!allowCamera) return
        val host = activity as? AppCompatActivity
        if (host == null) {
            sendFailure("Activity is not AppCompatActivity")
            removeSelf()
            return
        }
        val captureMode = when (selectedMode.lowercase()) {
            "videos" -> "video"
            else -> "image"
        }
        MediaCaptureFragment.start(
            host,
            captureMode,
            maxCaptureDuration,
            callbackId,
            cameraFacingPref
        )
        removeSelf()
    }

    private fun applySendButtonStyle(count: Int) {
        val enabled = count > 0
        val label = maxSelectable.toString()
        selectionSummaryView?.let { summary ->
            val txt = "已选 $count/$label"
            summary.text = txt
            summary.setTextColor(if (enabled) techBlueColor else disabledBlueColor)
        }

        sendBtn?.let { btn ->
            btn.isEnabled = enabled
            btn.text = "完成"
            btn.alpha = if (enabled) 1f else 0.8f
            val background = sendBtnBackground
            background?.setColor(if (enabled) techBlueColor else lightTechBlueColor)
            if (background != null && btn.background !== background) {
                btn.background = background
            }
            btn.setTextColor(Color.WHITE)
        }

    }

    private fun confirmSelection() {
        val keys = selected.keys.toList()
        // Prefer in-memory listener when present (embedded flows like ScanCode)
        resultListener?.let { listener ->
            listener.invoke(keys)
            activity?.runOnUiThread { removeSelf() }
            return
        }
        val cbId = callbackId
        Thread {
            try {
                val arr = org.json.JSONArray()
                for (uri in keys) {
                    val typeStr = when (itemsIndex[uri]?.fileType) { "video" -> "video"; else -> "image" }
                    val obj = org.json.JSONObject().apply {
                        put("uri", uri.toString())
                        put("fileType", typeStr)
                    }
                    arr.put(obj)
                }
                NativeApi.onCallback(cbId, true, arr.toString())
            } catch (e: Exception) {
                NativeApi.onCallback(cbId, false, (e.message ?: "build result failed"))
            } finally {
                activity?.runOnUiThread { removeSelf() }
            }
        }.start()
    }

    private class MediaGridAdapter(
        private val context: Context,
        private val onMediaClick: (GridItem) -> Unit,
        private val onCameraClick: () -> Unit
    ) : ListAdapter<GridItem, RecyclerView.ViewHolder>(Diff()) {
        companion object {
            private const val TYPE_MEDIA = 0
            private const val TYPE_CAMERA = 1
        }

        private val accentBlue = Color.parseColor("#1677FF")

        private val selected = HashSet<Uri>()

        fun setSelected(keys: Collection<Uri>) {
            selected.clear(); selected.addAll(keys)
            notifyDataSetChanged()
        }

        override fun getItemViewType(position: Int): Int {
            return if (getItem(position).fileType == CAMERA_ITEM_TYPE) TYPE_CAMERA else TYPE_MEDIA
        }

        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder {
            return if (viewType == TYPE_CAMERA) {
                val size = parent.measuredWidth / 4
                val container = FrameLayout(context).apply {
                    layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, size)
                    background = GradientDrawable().apply {
                        cornerRadius = dp(context, 12).toFloat()
                        setColor(Color.WHITE)
                    }
                }
                val icon = ImageView(context).apply {
                    setImageResource(android.R.drawable.ic_menu_camera)
                    setColorFilter(accentBlue)
                    layoutParams = FrameLayout.LayoutParams(dp(context, 36), dp(context, 36)).apply {
                        gravity = Gravity.CENTER_HORIZONTAL
                        topMargin = dp(context, 16)
                    }
                }
                val label = TextView(context).apply {
                    text = "拍摄"
                    setTextColor(Color.parseColor("#1F1F1F"))
                    textSize = 14f
                    gravity = Gravity.CENTER
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.WRAP_CONTENT
                    ).apply {
                        gravity = Gravity.BOTTOM
                        bottomMargin = dp(context, 16)
                    }
                }
                container.addView(icon)
                container.addView(label)
                CameraVH(container)
            } else {
                val size = parent.measuredWidth / 4
                val container = FrameLayout(context).apply {
                    layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, size)
                }
                val iv = ImageView(context).apply {
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT
                    )
                    scaleType = ImageView.ScaleType.CENTER_CROP
                    setBackgroundColor(Color.parseColor("#F0F0F0"))
                }
                val overlay = View(context).apply {
                    setBackgroundColor(Color.parseColor("#66000000"))
                    visibility = View.GONE
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT
                    )
                }
                val badgeSize = dp(context, 22)
                val badge = TextView(context).apply {
                    setTextColor(Color.WHITE)
                    textSize = 14f
                    typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
                    gravity = Gravity.CENTER
                    text = "✓"
                    background = GradientDrawable().apply {
                        shape = GradientDrawable.OVAL
                        setColor(accentBlue)
                    }
                    layoutParams = FrameLayout.LayoutParams(badgeSize, badgeSize).apply {
                        gravity = Gravity.TOP or Gravity.END
                        val m = dp(context, 6)
                        setMargins(m, m, m, m)
                    }
                    visibility = View.GONE
                }
                val ring = FrameLayout(context).apply {
                    layoutParams = FrameLayout.LayoutParams(badgeSize, badgeSize).apply {
                        gravity = Gravity.TOP or Gravity.END
                        val m = dp(context, 6)
                        setMargins(m, m, m, m)
                    }
                    val bg = View(context).apply {
                        background = GradientDrawable().apply {
                            shape = GradientDrawable.OVAL
                            setColor(Color.parseColor("#14000000"))
                        }
                        layoutParams = FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT)
                    }
                    val stroke = View(context).apply {
                        background = GradientDrawable().apply {
                            shape = GradientDrawable.OVAL
                            setColor(Color.TRANSPARENT)
                            setStroke(dp(context, 2), Color.parseColor("#D0D5DD"))
                        }
                        layoutParams = FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT)
                    }
                    addView(bg)
                    addView(stroke)
                    visibility = View.VISIBLE
                }
                val durationLabel = TextView(context).apply {
                    setTextColor(Color.WHITE)
                    textSize = 12f
                    setPadding(dp(context, 4), dp(context, 2), dp(context, 4), dp(context, 2))
                    background = GradientDrawable().apply {
                        shape = GradientDrawable.RECTANGLE
                        cornerRadius = dp(context, 8).toFloat()
                        setColor(Color.parseColor("#88000000"))
                    }
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.WRAP_CONTENT,
                        FrameLayout.LayoutParams.WRAP_CONTENT
                    ).apply {
                        gravity = Gravity.BOTTOM or Gravity.END
                        val m = dp(context, 6)
                        setMargins(m, m, m, m)
                    }
                    visibility = View.GONE
                }
                container.addView(iv)
                container.addView(overlay)
                container.addView(durationLabel)
                container.addView(badge)
                container.addView(ring)
                MediaVH(container, iv, overlay, badge, ring, durationLabel)
            }
        }

        override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
            val item = getItem(position)
            if (holder is CameraVH) {
                holder.itemView.setOnClickListener { onCameraClick() }
            } else if (holder is MediaVH) {
                holder.itemView.setOnClickListener { onMediaClick(item) }
                try {
                    val bmp = context.contentResolver.loadThumbnail(item.uri, Size(300, 300), null)
                    holder.image.setImageBitmap(bmp)
                } catch (_: Exception) {
                    holder.image.setImageDrawable(null)
                    holder.image.setBackgroundColor(Color.parseColor("#D9D9D9"))
                }
                val isSel = selected.contains(item.uri)
                holder.overlay.visibility = if (isSel) View.VISIBLE else View.GONE
                if (isSel) {
                    holder.badge.visibility = View.VISIBLE
                    holder.ring.visibility = View.GONE
                } else {
                    holder.badge.visibility = View.GONE
                    holder.ring.visibility = View.VISIBLE
                }
                if (item.fileType == "video" && item.durationSec > 0) {
                    holder.duration.visibility = View.VISIBLE
                    holder.duration.text = formatDuration(item.durationSec)
                } else {
                    holder.duration.visibility = View.GONE
                }
            }
        }

        private class CameraVH(view: View) : RecyclerView.ViewHolder(view)
        private class MediaVH(
            view: View,
            val image: ImageView,
            val overlay: View,
            val badge: TextView,
            val ring: View,
            val duration: TextView
        ) : RecyclerView.ViewHolder(view)

        private class Diff : DiffUtil.ItemCallback<GridItem>() {
            override fun areItemsTheSame(oldItem: GridItem, newItem: GridItem): Boolean {
                return oldItem.uri == newItem.uri && oldItem.fileType == newItem.fileType
            }

            override fun areContentsTheSame(oldItem: GridItem, newItem: GridItem): Boolean = oldItem == newItem
        }
    }

    private fun setupAlbumList() {
        val ctx = requireContext()
        val list = albumListView ?: return
        list.adapter = object : RecyclerView.Adapter<AlbumVH>() {
            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): AlbumVH {
                val row = LinearLayout(ctx).apply {
                    orientation = LinearLayout.HORIZONTAL
                    setPadding(dp(ctx, 12), dp(ctx, 10), dp(ctx, 12), dp(ctx, 10))
                    layoutParams = RecyclerView.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT
                    )
                    setBackgroundColor(Color.WHITE)
                }
                val cover = ImageView(ctx).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(ctx, 48), dp(ctx, 48))
                    scaleType = ImageView.ScaleType.CENTER_CROP
                    setBackgroundColor(Color.parseColor("#D9D9D9"))
                }
                val texts = LinearLayout(ctx).apply {
                    orientation = LinearLayout.VERTICAL
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                    setPadding(dp(ctx, 12), 0, 0, 0)
                }
                val name = TextView(ctx).apply { setTextColor(Color.parseColor("#1F1F1F")); textSize = 16f }
                val count = TextView(ctx).apply { setTextColor(Color.parseColor("#8C8C8C")); textSize = 12f }
                texts.addView(name); texts.addView(count)
                row.addView(cover); row.addView(texts)
                return AlbumVH(row, cover, name, count)
            }
            override fun getItemCount() = albums.size
            override fun onBindViewHolder(holder: AlbumVH, position: Int) {
                val item = albums[position]
                holder.name.text = item.name
                holder.count.text = "${item.count}"
                try {
                    val uri = item.coverUri
                    if (uri != null) {
                        val bmp = ctx.contentResolver.loadThumbnail(uri, Size(200, 200), null)
                        holder.cover.setImageBitmap(bmp)
                    } else {
                        holder.cover.setImageDrawable(null)
                        holder.cover.setBackgroundColor(Color.parseColor("#D9D9D9"))
                    }
                } catch (_: Exception) {
                    holder.cover.setImageDrawable(null)
                    holder.cover.setBackgroundColor(Color.parseColor("#D9D9D9"))
                }
                // Right selection check for selected album (including null id)
                if (currentAlbumId == item.id) {
                    if (holder.itemView.findViewWithTag<View>("sel_check") == null) {
                        val size = dp(ctx, 18)
                        val check = CheckMarkView(ctx, this@MediaPickerFragment.techBlueColor, dp(ctx, 2).toFloat()).apply {
                            tag = "sel_check"
                            layoutParams = LinearLayout.LayoutParams(size, size).apply {
                                gravity = Gravity.CENTER_VERTICAL
                                setMargins(dp(ctx, 8), 0, 0, 0)
                            }
                        }
                        (holder.itemView as ViewGroup).addView(check)
                    }
                    holder.itemView.setBackgroundColor(Color.parseColor("#E8F2FF"))
                } else {
                    holder.itemView.findViewWithTag<View>("sel_check")?.let { (holder.itemView as ViewGroup).removeView(it) }
                    holder.itemView.setBackgroundColor(Color.WHITE)
                }
                holder.itemView.setOnClickListener {
                    currentAlbumId = item.id
                    (recycler?.adapter as? MediaGridAdapter)?.submitList(filterByAlbum(allItems))
                    updateSelectionUI()
                    albumMenuContainer?.visibility = View.GONE
                    (albumSelectorView?.getChildAt(0) as? TextView)?.text = item.name
                }
            }
        }
    }

    private fun toggleAlbumMenu() {
        albumMenuContainer?.let { container ->
            val show = container.visibility != View.VISIBLE
            container.visibility = if (show) View.VISIBLE else View.GONE
            // Rotate arrow only; keep pill's arrow circle grey always
            val arrowCircle = (albumSelectorView?.getChildAt(1) as? FrameLayout)
            val arrow = arrowCircle?.getChildAt(0) as? ArrowView
            arrow?.animate()?.rotationBy(180f)?.setDuration(200)?.setInterpolator(AccelerateDecelerateInterpolator())?.start()
            (arrowCircle?.background as? GradientDrawable)?.setColor(Color.parseColor("#D9D9D9"))
        }
    }

    private fun filterByAlbum(list: List<GridItem>): List<GridItem> {
        val id = currentAlbumId
        val filtered = when (id) {
            null -> list // All of current mode (for mix: all images + videos)
            ALBUM_ALL_VIDEOS_ID -> list.filter { it.fileType == "video" }
            ALBUM_ALL_IMAGES_ID -> list.filter { it.fileType == "image" }
            else -> list.filter { it.bucketId == id }
        }
        return if (allowCamera) {
            ArrayList<GridItem>(filtered.size + 1).apply {
                add(cameraGridItem())
                addAll(filtered)
            }
        } else {
            filtered
        }
    }

    private class AlbumVH(view: View, val cover: ImageView, val name: TextView, val count: TextView) : RecyclerView.ViewHolder(view)

    // Insets helper
    private fun statusBarHeight(): Int {
        val resId = resources.getIdentifier("status_bar_height", "dimen", "android")
        return if (resId > 0) resources.getDimensionPixelSize(resId) else 0
    }

    // Draws 0.5dp hairline dividers instead of spacing gaps
    private class HairlineDividerDecoration(context: Context, private val dpWidth: Float, private val color: Int) : RecyclerView.ItemDecoration() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = this@HairlineDividerDecoration.color
            style = Paint.Style.STROKE
            strokeWidth = context.resources.displayMetrics.density * dpWidth
        }
        override fun onDraw(c: Canvas, parent: RecyclerView, state: RecyclerView.State) {
            val childCount = parent.childCount
            for (i in 0 until childCount) {
                val v = parent.getChildAt(i)
                val params = v.layoutParams as RecyclerView.LayoutParams
                // vertical line on right
                val x = (v.right + params.rightMargin).toFloat()
                c.drawLine(x, v.top.toFloat(), x, v.bottom.toFloat(), paint)
                // horizontal line on bottom
                val y = (v.bottom + params.bottomMargin).toFloat()
                c.drawLine(v.left.toFloat(), y, v.right.toFloat(), y, paint)
            }
        }
    }

    private fun sendFailure(message: String) {
        resultListener?.let {
            // For embedded flows, just dismiss
            activity?.runOnUiThread { removeSelf() }
            return
        }
        try {
            val payload = JSONObject().apply { put("error", message) }
            NativeApi.onCallback(callbackId, false, payload.toString())
        } catch (_: Exception) { }
    }

    private fun sendCancel() {
        resultListener?.let {
            // For embedded flows, just dismiss
            activity?.runOnUiThread { removeSelf() }
            return
        }
        try {
            val payload = JSONObject().apply { put("cancel", true) }
            NativeApi.onCallback(callbackId, true, payload.toString())
        } catch (_: Exception) { }
    }

    private fun removeSelf() {
        try {
            activity?.supportFragmentManager?.beginTransaction()?.remove(this)?.commitAllowingStateLoss()
        } catch (_: Exception) { }
    }

    private fun cameraGridItem(): GridItem = GridItem(
        CAMERA_PLACEHOLDER_URI,
        CAMERA_ITEM_TYPE,
        0.0,
        Long.MAX_VALUE,
        null,
        null
    )
}

// Top-level helpers for nested classes
private fun dp(context: Context, v: Int): Int = (v * context.resources.displayMetrics.density).toInt()

private fun formatDuration(d: Double): String {
    var sec = d.toInt()
    val h = sec / 3600; sec %= 3600
    val m = sec / 60; val s = sec % 60
    return if (h > 0) String.format("%d:%02d:%02d", h, m, s) else String.format("%02d:%02d", m, s)
}

// Custom-drawn arrow view (inside grey circle) for album selector
private class ArrowView(context: Context) : View(context) {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#595959")
        style = Paint.Style.STROKE
        strokeWidth = (context.resources.displayMetrics.density * 1.0f)
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }
    private var up = false
    fun setDirectionUp(value: Boolean) { up = value; invalidate() }
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat(); val h = height.toFloat()
        val left = w * 0.32f; val right = w * 0.68f
        val top = h * 0.38f; val bottom = h * 0.62f
        val path = Path()
        if (up) {
            // Upward chevron '^'
            path.moveTo(left, bottom)
            path.lineTo(w / 2f, top)
            path.lineTo(right, bottom)
        } else {
            // Downward chevron 'v'
            path.moveTo(left, top)
            path.lineTo(w / 2f, bottom)
            path.lineTo(right, top)
        }
        canvas.drawPath(path, paint)
    }
}

// Custom-drawn green check mark for selected album in list
private class CheckMarkView(context: Context, color: Int, private val stroke: Float) : View(context) {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        this.color = color
        style = Paint.Style.STROKE
        strokeWidth = stroke
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat(); val h = height.toFloat()
        val path = Path()
        val startX = w * 0.20f; val startY = h * 0.55f
        val midX = w * 0.42f; val midY = h * 0.78f
        val endX = w * 0.78f; val endY = h * 0.30f
        path.moveTo(startX, startY)
        path.lineTo(midX, midY)
        path.lineTo(endX, endY)
        canvas.drawPath(path, paint)
    }
}

// Custom-drawn close X view
private class CloseXView(context: Context) : View(context) {
    private val linePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#1F1F1F")
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
        strokeWidth = dp(context, 2).toFloat()
    }
    private val pressPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#1A000000")
        style = Paint.Style.FILL
    }
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat()
        val h = height.toFloat()
        // Smaller X by increasing inner padding proportionally
        val pad = (minOf(w, h) * 0.42f)
        if (isPressed) {
            val r = kotlin.math.min(w, h) * 0.45f
            canvas.drawCircle(w / 2f, h / 2f, r, pressPaint)
        }
        canvas.drawLine(pad, pad, w - pad, h - pad, linePaint)
        canvas.drawLine(w - pad, pad, pad, h - pad, linePaint)
    }
}
