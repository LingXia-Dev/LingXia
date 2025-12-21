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
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import com.lingxia.lxapp.NativeApi
import com.lingxia.lxapp.R
import org.json.JSONObject
import android.view.animation.AccelerateDecelerateInterpolator
import android.graphics.drawable.GradientDrawable
import android.graphics.Bitmap
import android.util.LruCache
import java.util.concurrent.Executors

/**
 * Unified custom media picker UI (grid multi-select, confirm), attached to current Activity.
 */
class MediaPickerFragment : Fragment() {
    companion object {
        private const val TAG = "LingXia.MediaPickerFrag"
        private const val STATE_IS_ORIGINAL = "state_is_original"
        private const val ARG_MAX_COUNT = "arg_max_count"
        private const val ARG_CALLBACK_ID = "arg_callback_id"
        private const val ARG_MODE = "arg_mode" // images | videos | mix
        private const val ARG_ALLOW_CAMERA = "arg_allow_camera"
        private const val ARG_MAX_DURATION = "arg_max_duration"
        private const val ARG_CAMERA_FACING = "arg_camera_facing"
        private const val CAMERA_ITEM_TYPE = "__camera__"
        private const val PLUS_ITEM_TYPE = "__plus__"
        private val CAMERA_PLACEHOLDER_URI: Uri = Uri.parse("lxapp-camera://capture")
        private val PLUS_PLACEHOLDER_URI: Uri = Uri.parse("lxapp-plus://limited")
        private const val PERM_READ_MEDIA_VISUAL_USER_SELECTED = "android.permission.READ_MEDIA_VISUAL_USER_SELECTED"
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
            onPicked: (List<Uri>, Boolean) -> Unit
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
    private var isOriginal: Boolean = false
    private var originalOptionView: RadioOptionView? = null
    private var allItems: List<GridItem> = emptyList()
    private val itemsIndex = HashMap<Uri, GridItem>()
    private var albums: List<AlbumItem> = emptyList()
    private var currentAlbumId: Long? = null // null => all
    private var albumMenuContainer: FrameLayout? = null
    private var albumListView: RecyclerView? = null
    private var albumSelectorView: LinearLayout? = null
    private var pendingOnLoaded: ((List<GridItem>) -> Unit)? = null
    private var resultListener: ((List<Uri>, Boolean) -> Unit)? = null
    private var limitedWarningBar: View? = null
    private var baseRecyclerPaddingBottom: Int = 0
    private var bottomBarHeightPx: Int = 0
    private var topBarHeightPx: Int = 0
    private var limitedWarningHeightPx: Int = 0
    private var limitedModeActive: Boolean = false
    private var pendingLimitedReload: Boolean = false

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { results ->
        val ctx = activity ?: return@registerForActivityResult
        val state = permissionState(ctx)
        limitedModeActive = state == PermissionState.LIMITED
        when (state) {
            PermissionState.FULL -> {
                pendingLimitedReload = false
                val callback = pendingOnLoaded
                pendingOnLoaded = null
                setLimitedWarningVisible(false)
                if (callback != null) {
                    loadMedia { items -> callback(items) }
                } else {
                    loadMedia { items -> applyLoadedItems(items) }
                }
            }
            PermissionState.LIMITED -> {
                val shouldReload = pendingLimitedReload
                pendingLimitedReload = false
                val callback = pendingOnLoaded
                pendingOnLoaded = null
                setLimitedWarningVisible(true)
                if (callback != null) {
                    loadMedia { items -> callback(items) }
                } else if (shouldReload) {
                    loadMedia { items -> applyLoadedItems(items) }
                }
            }
            PermissionState.NONE -> {
                limitedModeActive = false
                pendingLimitedReload = false
                sendFailure(3004)
                removeSelf()
                return@registerForActivityResult
            }
        }
        if (allItems.isNotEmpty()) {
            (recycler?.adapter as? MediaGridAdapter)?.submitList(filterByAlbum(allItems))
        }
    }



    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Ensure maxSelectable is available for launcher registration
        maxSelectable = (arguments?.getInt(ARG_MAX_COUNT) ?: 1).coerceAtLeast(1)
        if (savedInstanceState != null) {
            isOriginal = savedInstanceState.getBoolean(STATE_IS_ORIGINAL, false)
        }

        bottomBarHeightPx = dp(requireContext(), 64)
        topBarHeightPx = dp(requireContext(), 56) + statusBarHeight()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean(STATE_IS_ORIGINAL, isOriginal)
    }

    override fun onResume() {
        super.onResume()
        val ctx = activity ?: return
        val state = permissionState(ctx)
        limitedModeActive = state == PermissionState.LIMITED
        setLimitedWarningVisible(limitedModeActive)
        if (pendingLimitedReload) {
            pendingLimitedReload = false
            loadMedia { items -> applyLoadedItems(items) }
        }
        if (pendingOnLoaded != null) {
            val callback = pendingOnLoaded
            when (state) {
                PermissionState.FULL -> {
                    pendingOnLoaded = null
                    if (callback != null) {
                        loadMedia { items -> callback(items) }
                    }
                }
                PermissionState.LIMITED -> {
                    pendingOnLoaded = null
                    if (callback != null) {
                        loadMedia { items -> callback(items) }
                    }
                }
                PermissionState.NONE -> {
                    // still waiting for permissions
                }
            }
        }
        if (allItems.isNotEmpty()) {
            (recycler?.adapter as? MediaGridAdapter)?.submitList(filterByAlbum(allItems))
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        recycler = null
        sendBtn = null
        sendBtnBackground = null
        selectionSummaryView = null
        originalOptionView = null
        albumMenuContainer = null
        albumListView = null
        albumSelectorView = null
        limitedWarningBar = null
        pendingLimitedReload = false
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
            val h = topBarHeightPx
            setPadding(dp(context, 16), statusBarHeight(), dp(context, 16), dp(context, 12))
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                h
            ).apply { gravity = Gravity.TOP }
            ViewCompat.setElevation(this, dp(context, 2).toFloat())
        }
        // Use icon_close_x for consistent cross-platform close button
        val backBtn = ImageView(context).apply {
            layoutParams = FrameLayout.LayoutParams(dp(context, 48), dp(context, 48)).apply {
                gravity = Gravity.START or Gravity.CENTER_VERTICAL
            }
            setImageResource(R.drawable.icon_close_x)
            setColorFilter(Color.parseColor("#1F1F1F"))
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(context, 12), dp(context, 12), dp(context, 12), dp(context, 12))
            isClickable = true
            isFocusable = true
            setOnClickListener { sendCancel(); removeSelf() }
            contentDescription = getString(com.lingxia.lxapp.R.string.lx_common_close)
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
            onCameraClick = { launchCameraFromPicker() },
            onPlusClick = { requestLimitedAccessExpansion() },
            plusHintProvider = { limitedPlusHintText() }
        )
        rv.adapter = adapter
        root.addView(rv)

        // Bottom bar
        val bottom = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.WHITE)
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                bottomBarHeightPx
            ).apply { gravity = Gravity.BOTTOM }
            setPadding(dp(context, 16), dp(context, 10), dp(context, 16), dp(context, 14))
            gravity = Gravity.CENTER_VERTICAL
        }
        val summaryView = TextView(context).apply {
            text = "${getString(com.lingxia.lxapp.R.string.lx_album_selected)} 0/$maxSelectable"
            setTextColor(disabledBlueColor)
            textSize = 15f
            typeface = android.graphics.Typeface.create(android.graphics.Typeface.DEFAULT, android.graphics.Typeface.BOLD)
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        }
        selectionSummaryView = summaryView

        // Spacer to push "原图" to center
        val spacer = View(context).apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }

        // Original image option (hidden for video mode, centered)
        val originalOption = RadioOptionView(context, getString(com.lingxia.lxapp.R.string.lx_album_original_image), techBlueColor).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            setOnClickListener {
                isOriginal = !isOriginal
                setChecked(isOriginal)
            }
        }
        originalOptionView = originalOption

        // Hide "原图" option for video mode
        val isVideoMode = selectedMode.lowercase() == "video" || selectedMode.lowercase() == "videos"
        if (isVideoMode) {
            isOriginal = false
            originalOption.visibility = View.GONE
            originalOption.setChecked(false)
        } else {
            originalOption.visibility = View.VISIBLE
            originalOption.setChecked(isOriginal)
        }

        // Another spacer to balance the layout
        val spacer2 = View(context).apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }

        val sendBackground = GradientDrawable().apply {
            cornerRadius = dp(context, 18).toFloat()
            setColor(lightTechBlueColor)
        }
        sendBtnBackground = sendBackground
        val send = TextView(context).apply {
            text = getString(com.lingxia.lxapp.R.string.lx_common_done)
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
        bottom.addView(spacer)
        bottom.addView(originalOption)
        bottom.addView(spacer2)
        bottom.addView(send)
        root.addView(bottom)

        val warningHeight = dp(context, 48)
        limitedWarningHeightPx = warningHeight
        val warningBar = createLimitedWarningBar(context, warningHeight)
        root.addView(warningBar)
        warningBar.visibility = View.GONE
        limitedWarningBar = warningBar
        com.lingxia.lxapp.util.ActivityInsets.applyBottomMargin(root, warningBar, bottomBarHeightPx)

        // Lift bottom bar and content above system navigation bar using Activity provider
        com.lingxia.lxapp.util.ActivityInsets.applyBottomMargin(root, bottom, 0)
        com.lingxia.lxapp.util.ActivityInsets.applyBottomMargin(root, rv, bottomBarHeightPx)

        recycler = rv
        sendBtn = send
        baseRecyclerPaddingBottom = rv.paddingBottom

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
        val state = permissionState(act)
        limitedModeActive = state == PermissionState.LIMITED
        when (state) {
            PermissionState.FULL -> {
                pendingOnLoaded = null
                setLimitedWarningVisible(false)
                loadMedia(onLoaded)
            }
            PermissionState.LIMITED -> {
                setLimitedWarningVisible(true)
                pendingOnLoaded = null
                loadMedia(onLoaded)
            }
            PermissionState.NONE -> {
                limitedModeActive = false
                pendingOnLoaded = onLoaded
                permissionLauncher.launch(getNeededPermissions())
            }
        }
        if (state != PermissionState.NONE && allItems.isNotEmpty()) {
            (recycler?.adapter as? MediaGridAdapter)?.submitList(filterByAlbum(allItems))
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
                albumList.add(AlbumItem(null, getString(com.lingxia.lxapp.R.string.lx_album_all_videos), allCount, firstAllCover))
                albumList.addAll(systemAlbums)
            }
            "images" -> {
                albumList.add(AlbumItem(null, getString(com.lingxia.lxapp.R.string.lx_album_all_photos), allCount, firstAllCover))
                albumList.addAll(systemAlbums)
            }
            else -> {
                albumList.add(AlbumItem(null, getString(com.lingxia.lxapp.R.string.lx_album_all_media), allCount, firstAllCover))
                albumList.add(AlbumItem(ALBUM_ALL_VIDEOS_ID, getString(com.lingxia.lxapp.R.string.lx_album_all_videos), videos.size, firstVideoCover))
                albumList.addAll(systemAlbums)
            }
        }
        albums = albumList
        setupAlbumList()
        val defaultTitle = when (lowerMode) {
            "videos" -> getString(com.lingxia.lxapp.R.string.lx_album_all_videos)
            "images" -> getString(com.lingxia.lxapp.R.string.lx_album_all_photos)
            else -> getString(com.lingxia.lxapp.R.string.lx_album_all_media)
        }
        currentAlbumId = null
        (albumSelectorView?.getChildAt(0) as? TextView)?.text = defaultTitle
        (recycler?.adapter as? ListAdapter<GridItem, *>)?.submitList(filterByAlbum(allItems))
        albumMenuContainer?.visibility = View.GONE
        applySendButtonStyle(0)
    }

    private fun getNeededPermissions(): Array<String> {
        val needImages = needsImageAccess()
        val needVideos = needsVideoAccess()
        return when {
            Build.VERSION.SDK_INT >= 34 -> {
                // In limited mode, re-requesting VISUAL permission shows the system sheet again
                if (limitedModeActive) {
                    arrayOf(PERM_READ_MEDIA_VISUAL_USER_SELECTED)
                } else {
                    val perms = mutableListOf<String>()
                    if (needImages) perms += Manifest.permission.READ_MEDIA_IMAGES
                    if (needVideos) perms += Manifest.permission.READ_MEDIA_VIDEO
                    if (perms.isEmpty()) perms += Manifest.permission.READ_MEDIA_IMAGES
                    perms.distinct().toTypedArray()
                }
            }
            Build.VERSION.SDK_INT >= 33 -> {
                val perms = mutableListOf<String>()
                if (needImages) perms += Manifest.permission.READ_MEDIA_IMAGES
                if (needVideos) perms += Manifest.permission.READ_MEDIA_VIDEO
                if (perms.isEmpty()) perms += Manifest.permission.READ_MEDIA_IMAGES
                perms.distinct().toTypedArray()
            }
            else -> arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
        }
    }

    private fun needsImageAccess(): Boolean {
        val mode = selectedMode.lowercase()
        return mode != "video" && mode != "videos"
    }

    private fun needsVideoAccess(): Boolean {
        val mode = selectedMode.lowercase()
        return mode != "image" && mode != "images"
    }

    private fun permissionState(context: Context): PermissionState {
        if (Build.VERSION.SDK_INT < 33) {
            val granted = ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.READ_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
            return if (granted) PermissionState.FULL else PermissionState.NONE
        }
        if (Build.VERSION.SDK_INT >= 34) {
            val visualGranted = ContextCompat.checkSelfPermission(
                context,
                PERM_READ_MEDIA_VISUAL_USER_SELECTED
            ) == PackageManager.PERMISSION_GRANTED
            if (visualGranted) {
                return PermissionState.LIMITED
            }
        }
        val needImages = needsImageAccess()
        val needVideos = needsVideoAccess()
        val hasImages = !needImages || ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_IMAGES) == PackageManager.PERMISSION_GRANTED
        val hasVideos = !needVideos || ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_VIDEO) == PackageManager.PERMISSION_GRANTED
        if (hasImages && hasVideos) {
            return PermissionState.FULL
        }
        return PermissionState.NONE
    }

    private fun createLimitedWarningBar(context: Context, height: Int): View {
        val bar = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(Color.WHITE)
            setPadding(dp(context, 12), dp(context, 8), dp(context, 12), dp(context, 8))
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                height
            ).apply {
                gravity = Gravity.BOTTOM
                bottomMargin = bottomBarHeightPx
            }
        }
        val iconSize = dp(context, 20)
        val iconBg = FrameLayout(context).apply {
            background = GradientDrawable().apply {
                cornerRadius = iconSize / 2f
                setColor(Color.parseColor("#FFEFD2"))
            }
            layoutParams = LinearLayout.LayoutParams(iconSize, iconSize)
        }
        val icon = TextView(context).apply {
            text = "!"
            textSize = 12f
            gravity = Gravity.CENTER
            setTextColor(Color.parseColor("#FA8C16"))
        }
        iconBg.addView(
            icon,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
                Gravity.CENTER
            )
        )
        val message = TextView(context).apply {
            text = context.getString(com.lingxia.lxapp.R.string.lx_permission_limited_access_warning)
            textSize = 13f
            setTextColor(Color.parseColor("#595959"))
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            setPadding(dp(context, 8), 0, dp(context, 8), 0)
        }
        val arrow = TextView(context).apply {
            text = ">"
            textSize = 14f
            setTextColor(Color.parseColor("#BFBFBF"))
        }
        bar.addView(iconBg)
        bar.addView(message)
        bar.addView(arrow)
        bar.setOnClickListener { requestLimitedAccessExpansion() }
        return bar
    }

    private fun setLimitedWarningVisible(visible: Boolean) {
        val bar = limitedWarningBar ?: return
        if ((bar.visibility == View.VISIBLE) == visible) return
        bar.visibility = if (visible) View.VISIBLE else View.GONE
        updateRecyclerBottomPadding(if (visible) limitedWarningHeightPx else 0)
    }

    private fun requestLimitedAccessExpansion() {
        if (Build.VERSION.SDK_INT >= 33 && limitedModeActive) {
            pendingLimitedReload = true
            val perms = arrayOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VIDEO
            )
            permissionLauncher.launch(perms)
        }
    }

    private fun updateRecyclerBottomPadding(extra: Int) {
        val rv = recycler ?: return
        rv.setPadding(rv.paddingLeft, rv.paddingTop, rv.paddingRight, baseRecyclerPaddingBottom + extra)
    }

    private enum class PermissionState { FULL, LIMITED, NONE }

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
            sendFailure(1000)
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
            val txt = "${getString(com.lingxia.lxapp.R.string.lx_album_selected)} $count/$label"
            summary.text = txt
            summary.setTextColor(if (enabled) techBlueColor else disabledBlueColor)
        }

        sendBtn?.let { btn ->
            btn.isEnabled = enabled
            btn.text = getString(com.lingxia.lxapp.R.string.lx_common_done)
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
            listener.invoke(keys, isOriginal)
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
                        put("isOriginal", isOriginal)
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
        private val onCameraClick: () -> Unit,
        private val onPlusClick: () -> Unit,
        private val plusHintProvider: () -> String
    ) : ListAdapter<GridItem, RecyclerView.ViewHolder>(Diff()) {
        companion object {
            private const val TYPE_MEDIA = 0
            private const val TYPE_CAMERA = 1
            private const val TYPE_PLUS = 2
            private const val THUMB_PLACEHOLDER_COLOR: Int = 0xFFF0F0F0.toInt()
            private const val THUMB_ERROR_COLOR: Int = 0xFFD9D9D9.toInt()
            // Shared LruCache for thumbnail bitmaps (max ~20MB)
            private val cacheLock = Any()
            private val thumbnailCache: LruCache<Uri, Bitmap> = object : LruCache<Uri, Bitmap>(
                ((Runtime.getRuntime().maxMemory() / 1024) / 8).toInt().coerceAtMost(20 * 1024)
            ) {
                override fun sizeOf(key: Uri, value: Bitmap): Int = value.byteCount / 1024
            }
        }

        private val accentBlue = Color.parseColor("#1677FF")
        private val thumbnailExecutor = Executors.newFixedThreadPool(2)

        private val selected = HashSet<Uri>()

        fun setSelected(keys: Collection<Uri>) {
            selected.clear(); selected.addAll(keys)
            notifyDataSetChanged()
        }

        override fun onDetachedFromRecyclerView(recyclerView: RecyclerView) {
            super.onDetachedFromRecyclerView(recyclerView)
            thumbnailExecutor.shutdownNow()
        }

        override fun getItemViewType(position: Int): Int {
            val type = getItem(position).fileType
            return when (type) {
                CAMERA_ITEM_TYPE -> TYPE_CAMERA
                PLUS_ITEM_TYPE -> TYPE_PLUS
                else -> TYPE_MEDIA
            }
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
                    text = context.getString(com.lingxia.lxapp.R.string.lx_camera_label)
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
            } else if (viewType == TYPE_PLUS) {
                val size = parent.measuredWidth / 4
                val container = LinearLayout(context).apply {
                    orientation = LinearLayout.VERTICAL
                    gravity = Gravity.CENTER
                    layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, size)
                    background = GradientDrawable().apply {
                        cornerRadius = dp(context, 12).toFloat()
                        setColor(Color.parseColor("#F5F6F7"))
                        setStroke(dp(context, 1), Color.parseColor("#D9D9D9"))
                    }
                    setPadding(dp(context, 8), dp(context, 8), dp(context, 8), dp(context, 8))
                }
                val plusLabel = TextView(context).apply {
                    text = "+"
                    textSize = 32f
                    gravity = Gravity.CENTER
                    setTextColor(accentBlue)
                }
                val hint = TextView(context).apply {
                    text = plusHintProvider()
                    gravity = Gravity.CENTER
                    textSize = 11f
                    setTextColor(Color.parseColor("#8C8C8C"))
                }
                container.addView(plusLabel)
                container.addView(hint)
                PlusVH(container, hint)
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
            } else if (holder is PlusVH) {
                holder.hint.text = plusHintProvider()
                holder.itemView.setOnClickListener { onPlusClick() }
            } else if (holder is MediaVH) {
                holder.itemView.setOnClickListener { onMediaClick(item) }
                // Async thumbnail loading with cache
                holder.image.tag = item.uri
                val cached = synchronized(cacheLock) { thumbnailCache.get(item.uri) }
                if (cached != null) {
                    holder.image.background = null
                    holder.image.setImageBitmap(cached)
                } else {
                    holder.image.setImageDrawable(null)
                    holder.image.setBackgroundColor(THUMB_PLACEHOLDER_COLOR)
                    thumbnailExecutor.execute {
                        try {
                            val bmp = context.contentResolver.loadThumbnail(item.uri, Size(300, 300), null)
                            synchronized(cacheLock) { thumbnailCache.put(item.uri, bmp) }
                            holder.image.post {
                                if (holder.image.tag == item.uri) {
                                    holder.image.background = null
                                    holder.image.setImageBitmap(bmp)
                                }
                            }
                        } catch (_: Exception) {
                            holder.image.post {
                                if (holder.image.tag == item.uri) {
                                    holder.image.setImageDrawable(null)
                                    holder.image.setBackgroundColor(THUMB_ERROR_COLOR)
                                }
                            }
                        }
                    }
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
        private class PlusVH(view: View, val hint: TextView) : RecyclerView.ViewHolder(view)

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
        val extras = (if (allowCamera) 1 else 0) + (if (shouldShowLimitedPlusTile()) 1 else 0)
        if (extras == 0) {
            return filtered
        }
        return ArrayList<GridItem>(filtered.size + extras).apply {
            if (allowCamera) add(cameraGridItem())
            addAll(filtered)
            if (shouldShowLimitedPlusTile()) add(plusGridItem())
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

    private fun sendFailure(code: Int) {
        resultListener?.let {
            // For embedded flows, just dismiss
            activity?.runOnUiThread { removeSelf() }
            return
        }
        try {
            NativeApi.onCallback(callbackId, false, code.toString())
        } catch (_: Exception) { }
    }

    private fun sendCancel() {
        resultListener?.let {
            // For embedded flows, just dismiss
            activity?.runOnUiThread { removeSelf() }
            return
        }
        try {
            NativeApi.onCallback(callbackId, false, "2000")
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

    private fun plusGridItem(): GridItem = GridItem(
        PLUS_PLACEHOLDER_URI,
        PLUS_ITEM_TYPE,
        0.0,
        Long.MAX_VALUE - 1,
        null,
        null
    )

    private fun shouldShowLimitedPlusTile(): Boolean {
        return limitedModeActive && Build.VERSION.SDK_INT >= 33
    }

    private fun limitedPlusHintText(): String {
        return when (selectedMode.lowercase()) {
            "videos" -> getString(com.lingxia.lxapp.R.string.lx_album_add_more_videos)
            "images" -> getString(com.lingxia.lxapp.R.string.lx_album_add_more_photos)
            else -> getString(com.lingxia.lxapp.R.string.lx_album_add_more_media)
        }
    }
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

// Radio button option view (similar to iOS RadioOptionView)
private class RadioOptionView(context: Context, title: String, private val accentColor: Int) : LinearLayout(context) {
    private val radioButton: View
    private val ringPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = dp(context, 2).toFloat()
        color = Color.LTGRAY
    }
    private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.FILL
        color = accentColor
    }
    private var isChecked = false

    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL

        // Radio circle
        radioButton = object : View(context) {
            override fun onDraw(canvas: Canvas) {
                super.onDraw(canvas)
                val cx = width / 2f
                val cy = height / 2f
                val radius = (minOf(width, height) / 2f) - dp(context, 2)

                // Draw ring
                canvas.drawCircle(cx, cy, radius, ringPaint)

                // Draw dot when checked
                if (isChecked) {
                    canvas.drawCircle(cx, cy, radius * 0.6f, dotPaint)
                }
            }
        }.apply {
            val size = dp(context, 20)
            layoutParams = LayoutParams(size, size)
        }
        addView(radioButton)

        // Label
        val label = TextView(context).apply {
            text = title
            setTextColor(Color.DKGRAY)
            textSize = 14f
            layoutParams = LayoutParams(
                LayoutParams.WRAP_CONTENT,
                LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(dp(context, 6), 0, 0, 0)
            }
        }
        addView(label)

        isClickable = true
        isFocusable = true
    }

    fun setChecked(checked: Boolean) {
        isChecked = checked
        radioButton.invalidate()
    }
}
