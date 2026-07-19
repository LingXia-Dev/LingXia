package com.lingxia.lxapp.APIs.file

import android.app.Activity
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.text.TextUtils
import android.text.format.DateUtils
import android.text.format.Formatter
import android.view.Gravity
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup
import android.webkit.MimeTypeMap
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.graphics.ColorUtils
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.appbar.MaterialToolbar
import com.google.android.material.color.MaterialColors
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.lxapp.R
import org.json.JSONArray
import java.io.File
import java.io.IOException
import java.util.Locale
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger

internal class LocalFileChooserActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_ROOT_PATH = "com.lingxia.lxapp.filechooser.extra.ROOT_PATH"
        const val EXTRA_MODE = "com.lingxia.lxapp.filechooser.extra.MODE"
        const val EXTRA_TITLE = "com.lingxia.lxapp.filechooser.extra.TITLE"
        const val EXTRA_EXTENSIONS = "com.lingxia.lxapp.filechooser.extra.EXTENSIONS"
        const val EXTRA_FILTERS_JSON = "com.lingxia.lxapp.filechooser.extra.FILTERS_JSON"
        const val EXTRA_SELECTED_PATHS = "selectedPaths"

        const val MODE_FILE = "file"
        const val MODE_DIRECTORY = "directory"
    }

    private enum class EntryType {
        DIRECTORY,
        FILE,
    }

    private enum class ChooserMode {
        FILE,
        DIRECTORY,
    }

    private data class Entry(
        val file: File,
        val type: EntryType,
        val title: String,
        val subtitle: String,
        val badge: String? = null,
    )

    private data class FilterSpec(
        val extensions: Set<String> = emptySet(),
        val exactMimeTypes: Set<String> = emptySet(),
        val wildcardMimeGroups: Set<String> = emptySet(),
        val labels: List<String> = emptyList(),
    ) {
        fun isEmpty(): Boolean =
            extensions.isEmpty() && exactMimeTypes.isEmpty() && wildcardMimeGroups.isEmpty()

        fun matches(file: File): Boolean {
            if (isEmpty()) {
                return true
            }

            val extension = file.extension.lowercase(Locale.US)
            if (extension.isNotEmpty() && extensions.contains(extension)) {
                return true
            }
            if (extension.isEmpty()) {
                return false
            }

            val mime = MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(extension)
                ?.lowercase(Locale.US)
                ?: return false

            if (exactMimeTypes.contains(mime)) {
                return true
            }

            val group = mime.substringBefore('/', "")
            return group.isNotEmpty() && wildcardMimeGroups.contains(group)
        }
    }

    private sealed interface DirectoryLoadResult {
        data class Content(val entries: List<Entry>) : DirectoryLoadResult
        data class Empty(val filtered: Boolean) : DirectoryLoadResult
        object Unavailable : DirectoryLoadResult
    }

    private lateinit var rootDir: File
    private lateinit var currentDir: File
    private var chooserMode: ChooserMode = ChooserMode.FILE
    private var chooserTitle: String = ""
    private var filterSpec: FilterSpec = FilterSpec()

    private lateinit var rootLayout: LinearLayout
    private lateinit var toolbar: MaterialToolbar
    private lateinit var pathCard: LinearLayout
    private lateinit var filterView: TextView
    private lateinit var contentFrame: FrameLayout
    private lateinit var recyclerView: RecyclerView
    private lateinit var stateContainer: LinearLayout
    private lateinit var progressBar: ProgressBar
    private lateinit var stateTitleView: TextView
    private lateinit var stateSubtitleView: TextView
    private lateinit var adapter: EntryAdapter
    private var revealedDeletePath: String? = null

    private val ioExecutor = Executors.newSingleThreadExecutor()
    private val loadToken = AtomicInteger(0)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val rootPath = intent.getStringExtra(EXTRA_ROOT_PATH).orEmpty()
        val root = File(rootPath)
        if (rootPath.isBlank() || !root.isDirectory || !root.canRead()) {
            setResult(Activity.RESULT_CANCELED)
            finish()
            return
        }

        rootDir = root
        currentDir = root
        chooserMode = when (intent.getStringExtra(EXTRA_MODE)) {
            MODE_DIRECTORY -> ChooserMode.DIRECTORY
            else -> ChooserMode.FILE
        }
        chooserTitle = intent.getStringExtra(EXTRA_TITLE)
            ?.takeIf { it.isNotBlank() }
            ?: getString(R.string.lx_file_chooser_default_title)
        filterSpec = parseFilterSpec(
            filtersJson = intent.getStringExtra(EXTRA_FILTERS_JSON),
            fallbackExtensions = intent.getStringArrayListExtra(EXTRA_EXTENSIONS)?.toSet().orEmpty(),
        )

        LxAppActivity.configureTransparentSystemBars(this)
        buildLayout()
        bindWindowInsets()
        updateHeader(rootDir)
        loadDirectory(rootDir)

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (revealedDeletePath != null) {
                    revealedDeletePath = null
                    adapter.notifyDataSetChanged()
                    return
                }
                if (currentDir != rootDir) {
                    currentDir.parentFile
                        ?.takeIf { isWithinRoot(it) }
                        ?.let {
                            updateHeader(it)
                            loadDirectory(it)
                            return
                        }
                }
                cancelAndFinish()
            }
        })
    }

    override fun onDestroy() {
        super.onDestroy()
        ioExecutor.shutdownNow()
    }

    private fun buildLayout() {
        val surface = colorAttr(com.google.android.material.R.attr.colorSurface, Color.WHITE)
        val onSurface = colorAttr(com.google.android.material.R.attr.colorOnSurface, Color.BLACK)
        val onSurfaceVariant = colorAttr(com.google.android.material.R.attr.colorOnSurfaceVariant, 0xFF6B7280.toInt())
        val outline = colorAttr(com.google.android.material.R.attr.colorOutline, 0x1F000000)
        val primary = colorAttr(com.google.android.material.R.attr.colorPrimary, 0xFF1F6BFF.toInt())
        val pageBackground = ColorUtils.blendARGB(surface, primary, 0.035f)

        rootLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
            setBackgroundColor(pageBackground)
        }

        toolbar = MaterialToolbar(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            minimumHeight = dp(56)
            title = chooserTitle
            isTitleCentered = true
            setTitleTextColor(onSurface)
            setBackgroundColor(Color.TRANSPARENT)
            navigationIcon = ContextCompat.getDrawable(context, R.drawable.icon_back)?.mutate()
            navigationIcon?.setTint(onSurface)
            setNavigationOnClickListener { onBackPressedDispatcher.onBackPressed() }
            contentInsetStartWithNavigation = dp(8)
            contentInsetEndWithActions = dp(8)
            setContentInsetsRelative(dp(8), dp(8))
            if (chooserMode == ChooserMode.DIRECTORY) {
                menu.add(getString(R.string.lx_common_done)).apply {
                    setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                    setOnMenuItemClickListener {
                        finishWithSelection(currentDir)
                        true
                    }
                }
            }
        }

        filterView = TextView(this).apply {
            setTextColor(primary)
            textSize = 12f
            visibility = View.GONE
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = dpF(12f)
                setColor(ColorUtils.setAlphaComponent(primary, 24))
            }
            setPadding(dp(10), dp(6), dp(10), dp(6))
        }

        pathCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply {
                setMargins(dp(16), dp(8), dp(16), dp(12))
            }
            setPadding(dp(16), dp(14), dp(16), dp(14))
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = dpF(14f)
                setColor(surface)
                setStroke(dp(1), outline)
            }
            visibility = View.GONE
            addView(filterView, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ))
        }

        adapter = EntryAdapter(
            primary = primary,
            onSurface = onSurface,
            onSurfaceVariant = onSurfaceVariant,
            outline = outline,
            onOpenDirectory = { directory ->
                revealedDeletePath = null
                updateHeader(directory)
                loadDirectory(directory)
            },
            onChooseFile = { file -> finishWithSelection(file) },
            onDeleteEntry = { entry -> revealDelete(entry) },
            isAnyDeleteRevealed = { revealedDeletePath != null },
            isDeleteRevealed = { entry -> revealedDeletePath == entry.file.absolutePath },
            onDeleteTap = { entry -> deleteAndRefresh(entry) },
        )

        recyclerView = RecyclerView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            )
            layoutManager = LinearLayoutManager(this@LocalFileChooserActivity)
            adapter = this@LocalFileChooserActivity.adapter
            setBackgroundColor(surface)
            overScrollMode = RecyclerView.OVER_SCROLL_IF_CONTENT_SCROLLS
            clipToPadding = false
            addItemDecoration(SimpleDividerDecoration(outline, dp(16)))
        }

        progressBar = ProgressBar(this).apply {
            isIndeterminate = true
            visibility = View.GONE
        }

        stateTitleView = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(onSurface)
            textSize = 18f
            setTypeface(typeface, Typeface.BOLD)
        }

        stateSubtitleView = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(onSurfaceVariant)
            textSize = 14f
            maxLines = 3
        }

        stateContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            )
            setPadding(dp(24), dp(24), dp(24), dp(24))
            addView(progressBar)
            addView(stateTitleView, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply {
                topMargin = dp(14)
            })
            addView(stateSubtitleView, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply {
                topMargin = dp(8)
            })
            visibility = View.GONE
        }

        contentFrame = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f,
            )
            addView(recyclerView)
            addView(stateContainer)
        }

        rootLayout.addView(toolbar)
        rootLayout.addView(pathCard)
        rootLayout.addView(contentFrame)
        setContentView(rootLayout)
    }

    private fun bindWindowInsets() {
        ViewCompat.setOnApplyWindowInsetsListener(rootLayout) { _, insets ->
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            toolbar.updatePadding(top = systemBars.top)
            recyclerView.updatePadding(bottom = dp(16) + systemBars.bottom)
            stateContainer.updatePadding(bottom = dp(36) + systemBars.bottom)
            insets
        }
    }

    private fun updateHeader(dir: File) {
        currentDir = dir

        val filterSummary = buildFilterSummary()
        if (filterSummary == null) {
            pathCard.visibility = View.GONE
            filterView.visibility = View.GONE
        } else {
            pathCard.visibility = View.VISIBLE
            filterView.visibility = View.VISIBLE
            filterView.text = getString(R.string.lx_file_chooser_filter_label, filterSummary)
        }
    }

    private fun loadDirectory(dir: File) {
        if (!dir.exists() || !dir.isDirectory || !dir.canRead()) {
            showUnavailableState()
            return
        }

        currentDir = dir
        val token = loadToken.incrementAndGet()
        showLoadingState()
        ioExecutor.execute {
            val result = buildDirectoryLoadResult(dir)
            runOnUiThread {
                if (isFinishing || isDestroyed || token != loadToken.get()) {
                    return@runOnUiThread
                }
                applyLoadResult(result)
            }
        }
    }

    private fun buildDirectoryLoadResult(dir: File): DirectoryLoadResult {
        val children = try {
            dir.listFiles()?.toList().orEmpty()
        } catch (_: SecurityException) {
            return DirectoryLoadResult.Unavailable
        }

        val directories = children
            .filter { it.isDirectory && it.canRead() }
            .sortedBy { it.name.lowercase(Locale.US) }
            .map { directory ->
                Entry(
                    file = directory,
                    type = EntryType.DIRECTORY,
                    title = directory.name.ifBlank { directory.absolutePath },
                    subtitle = getString(R.string.lx_file_chooser_folder_subtitle),
                )
            }

        val matchingFiles = if (chooserMode == ChooserMode.FILE) {
            children
                .filter { it.isFile && it.canRead() && filterSpec.matches(it) }
                .sortedBy { it.name.lowercase(Locale.US) }
                .map { file ->
                    Entry(
                        file = file,
                        type = EntryType.FILE,
                        title = file.name,
                        subtitle = buildFileSubtitle(file),
                        badge = file.extension.takeIf { it.isNotBlank() }?.uppercase(Locale.US),
                    )
                }
        } else {
            emptyList()
        }

        val entries = buildList {
            addAll(directories)
            addAll(matchingFiles)
        }

        if (entries.isEmpty()) {
            val hasReadableFiles = chooserMode == ChooserMode.FILE && children.any { it.isFile && it.canRead() }
            return DirectoryLoadResult.Empty(filtered = !filterSpec.isEmpty() && hasReadableFiles)
        }

        return DirectoryLoadResult.Content(entries)
    }

    private fun applyLoadResult(result: DirectoryLoadResult) {
        when (result) {
            is DirectoryLoadResult.Content -> {
                adapter.submitList(result.entries)
                recyclerView.visibility = View.VISIBLE
                stateContainer.visibility = View.GONE
            }
            is DirectoryLoadResult.Empty -> {
                adapter.submitList(emptyList())
                recyclerView.visibility = View.GONE
                if (result.filtered) {
                    showState(
                        title = getString(R.string.lx_file_chooser_filtered_empty_title),
                        subtitle = getString(R.string.lx_file_chooser_filtered_empty_subtitle),
                        showProgress = false,
                    )
                } else {
                    showState(
                        title = getString(R.string.lx_file_chooser_empty_title),
                        subtitle = getString(R.string.lx_file_chooser_empty_subtitle),
                        showProgress = false,
                    )
                }
            }
            DirectoryLoadResult.Unavailable -> {
                adapter.submitList(emptyList())
                recyclerView.visibility = View.GONE
                showUnavailableState()
            }
        }
    }

    private fun showLoadingState() {
        recyclerView.visibility = View.GONE
        showState(
            title = getString(R.string.lx_common_loading),
            subtitle = "",
            showProgress = true,
        )
    }

    private fun showUnavailableState() {
        showState(
            title = getString(R.string.lx_err_code_1001),
            subtitle = "",
            showProgress = false,
        )
    }

    private fun showState(
        title: String,
        subtitle: String,
        showProgress: Boolean,
    ) {
        progressBar.visibility = if (showProgress) View.VISIBLE else View.GONE
        stateTitleView.text = title
        stateSubtitleView.text = subtitle
        stateContainer.visibility = View.VISIBLE
    }

    private fun revealDelete(entry: Entry) {
        revealedDeletePath = entry.file.absolutePath
        adapter.notifyDataSetChanged()
    }

    private fun deleteAndRefresh(entry: Entry) {
        if (!deleteEntry(entry.file)) {
            android.widget.Toast.makeText(
                this,
                getString(R.string.lx_err_code_1001),
                android.widget.Toast.LENGTH_SHORT
            ).show()
        }
        revealedDeletePath = null
        loadDirectory(currentDir)
    }

    private fun deleteEntry(target: File): Boolean {
        return try {
            if (!isWithinRoot(target)) {
                return false
            }
            if (target.isDirectory) {
                target.deleteRecursively()
            } else {
                target.delete()
            }
        } catch (_: Throwable) {
            false
        }
    }

    private fun buildFileSubtitle(file: File): String {
        val size = Formatter.formatShortFileSize(this, file.length())
        val modified = DateUtils.getRelativeTimeSpanString(
            file.lastModified(),
            System.currentTimeMillis(),
            DateUtils.MINUTE_IN_MILLIS,
            DateUtils.FORMAT_ABBREV_RELATIVE,
        )
        return "$size • $modified"
    }

    private fun buildFilterSummary(): String? {
        if (filterSpec.isEmpty()) {
            return null
        }

        val labels = filterSpec.labels.distinct()
        if (labels.isEmpty()) {
            return null
        }

        return if (labels.size <= 3) {
            labels.joinToString(" · ")
        } else {
            labels.take(3).joinToString(" · ") + " +${labels.size - 3}"
        }
    }

    private fun parseFilterSpec(
        filtersJson: String?,
        fallbackExtensions: Set<String>,
    ): FilterSpec {
        val extensions = linkedSetOf<String>()
        val exactMimeTypes = linkedSetOf<String>()
        val wildcardMimeGroups = linkedSetOf<String>()
        val labels = mutableListOf<String>()

        if (!filtersJson.isNullOrBlank()) {
            try {
                val array = JSONArray(filtersJson)
                for (index in 0 until array.length()) {
                    val raw = array.optString(index).trim()
                    if (raw.isEmpty()) {
                        continue
                    }
                    val normalized = raw.lowercase(Locale.US)
                    if (normalized.contains('/')) {
                        if (normalized.endsWith("/*")) {
                            wildcardMimeGroups.add(normalized.substringBefore('/'))
                        } else {
                            exactMimeTypes.add(normalized)
                        }
                        labels.add(filterLabel(normalized))
                    } else {
                        val ext = normalized.trimStart('.')
                        if (ext.isNotEmpty()) {
                            extensions.add(ext)
                            labels.add(ext.uppercase(Locale.US))
                        }
                    }
                }
            } catch (_: Exception) {
            }
        }

        fallbackExtensions.forEach { raw ->
            val ext = raw.trim().trimStart('.').lowercase(Locale.US)
            if (ext.isNotEmpty() && extensions.add(ext)) {
                labels.add(ext.uppercase(Locale.US))
            }
        }

        return FilterSpec(
            extensions = extensions,
            exactMimeTypes = exactMimeTypes,
            wildcardMimeGroups = wildcardMimeGroups,
            labels = labels,
        )
    }

    private fun filterLabel(value: String): String {
        return when (value) {
            "image/*" -> getString(R.string.lx_file_chooser_filter_images)
            "video/*" -> getString(R.string.lx_file_chooser_filter_videos)
            "audio/*" -> getString(R.string.lx_file_chooser_filter_audio)
            else -> value.substringAfterLast('/').uppercase(Locale.US)
        }
    }

    private fun finishWithSelection(file: File) {
        val result = Intent().apply {
            putStringArrayListExtra(EXTRA_SELECTED_PATHS, arrayListOf(file.absolutePath))
        }
        setResult(Activity.RESULT_OK, result)
        finish()
    }

    private fun cancelAndFinish() {
        setResult(Activity.RESULT_CANCELED)
        finish()
    }

    private fun isWithinRoot(candidate: File): Boolean {
        return try {
            val candidatePath = candidate.canonicalPath
            val rootPath = rootDir.canonicalPath
            val rootPrefix = if (rootPath.endsWith(File.separator)) rootPath else rootPath + File.separator
            candidatePath == rootPath || candidatePath.startsWith(rootPrefix)
        } catch (_: IOException) {
            false
        }
    }

    private fun colorAttr(attr: Int, fallback: Int): Int =
        MaterialColors.getColor(this, attr, fallback)

    private fun dp(value: Int): Int =
        (value * resources.displayMetrics.density).toInt()

    private fun dpF(value: Float): Float =
        value * resources.displayMetrics.density

    private inner class EntryAdapter(
        private val primary: Int,
        private val onSurface: Int,
        private val onSurfaceVariant: Int,
        private val outline: Int,
        private val onOpenDirectory: (File) -> Unit,
        private val onChooseFile: (File) -> Unit,
        private val onDeleteEntry: (Entry) -> Unit,
        private val isAnyDeleteRevealed: () -> Boolean,
        private val isDeleteRevealed: (Entry) -> Boolean,
        private val onDeleteTap: (Entry) -> Unit,
    ) : RecyclerView.Adapter<EntryAdapter.EntryViewHolder>() {
        private val items = mutableListOf<Entry>()

        fun submitList(entries: List<Entry>) {
            items.clear()
            items.addAll(entries)
            notifyDataSetChanged()
        }

        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): EntryViewHolder {
            val row = LinearLayout(parent.context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                layoutParams = RecyclerView.LayoutParams(
                    RecyclerView.LayoutParams.MATCH_PARENT,
                    RecyclerView.LayoutParams.WRAP_CONTENT,
                )
                minimumHeight = dp(72)
                setPadding(dp(16), dp(14), dp(16), dp(14))
                setBackgroundResource(selectableItemBackground(context))
            }

            val badgeView = TextView(parent.context).apply {
                setTextColor(primary)
                textSize = 11f
                setTypeface(typeface, Typeface.BOLD)
                gravity = Gravity.CENTER
                minWidth = dp(44)
                setPadding(dp(10), dp(6), dp(10), dp(6))
            }

            val textContainer = LinearLayout(parent.context).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    marginStart = dp(12)
                }
            }

            val titleView = TextView(parent.context).apply {
                setTextColor(onSurface)
                textSize = 16f
                setTypeface(typeface, Typeface.BOLD)
                maxLines = 1
                ellipsize = TextUtils.TruncateAt.MIDDLE
            }

            val subtitleView = TextView(parent.context).apply {
                setTextColor(onSurfaceVariant)
                textSize = 13f
                maxLines = 1
                ellipsize = TextUtils.TruncateAt.END
            }

            textContainer.addView(titleView)
            textContainer.addView(subtitleView, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply {
                topMargin = dp(4)
            })

            val arrowView = ImageView(parent.context).apply {
                layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                    marginStart = dp(8)
                }
                setImageResource(R.drawable.icon_forward)
                setColorFilter(onSurfaceVariant)
                visibility = View.GONE
            }

            val deleteView = TextView(parent.context).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                ).apply {
                    marginStart = dp(8)
                }
                text = context.getString(R.string.lx_common_delete)
                setTextColor(0xFFD92D20.toInt())
                textSize = 14f
                setTypeface(typeface, Typeface.BOLD)
                visibility = View.GONE
            }

            row.addView(badgeView)
            row.addView(textContainer)
            row.addView(arrowView)
            row.addView(deleteView)

            return EntryViewHolder(
                row = row,
                titleView = titleView,
                subtitleView = subtitleView,
                badgeView = badgeView,
                arrowView = arrowView,
                deleteView = deleteView,
            )
        }

        override fun getItemCount(): Int = items.size

        override fun onBindViewHolder(holder: EntryViewHolder, position: Int) {
            val entry = items[position]
            holder.titleView.text = entry.title

            if (entry.type == EntryType.DIRECTORY) {
                holder.subtitleView.visibility = View.GONE
                holder.badgeView.text = holder.row.context.getString(R.string.lx_file_chooser_folder_subtitle)
                holder.badgeView.background = GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    cornerRadius = dpF(12f)
                    setColor(ColorUtils.setAlphaComponent(primary, 18))
                    setStroke(dp(1), ColorUtils.setAlphaComponent(primary, 42))
                }
                holder.badgeView.visibility = if (isDeleteRevealed(entry)) View.GONE else View.VISIBLE
                holder.arrowView.visibility = if (isDeleteRevealed(entry)) View.GONE else View.VISIBLE
                holder.deleteView.visibility = if (isDeleteRevealed(entry)) View.VISIBLE else View.GONE
                holder.row.setOnClickListener {
                    if (!isAnyDeleteRevealed()) {
                        onOpenDirectory(entry.file)
                    }
                }
                holder.row.setOnLongClickListener {
                    onDeleteEntry(entry)
                    true
                }
            } else {
                holder.subtitleView.text = entry.subtitle
                holder.subtitleView.visibility = View.VISIBLE
                holder.badgeView.text = entry.badge
                holder.badgeView.background = GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    cornerRadius = dpF(12f)
                    setColor(ColorUtils.setAlphaComponent(primary, 18))
                    setStroke(dp(1), ColorUtils.setAlphaComponent(primary, 42))
                }
                holder.badgeView.visibility = if (entry.badge.isNullOrBlank() || isDeleteRevealed(entry)) View.GONE else View.VISIBLE
                holder.arrowView.visibility = View.GONE
                holder.deleteView.visibility = if (isDeleteRevealed(entry)) View.VISIBLE else View.GONE
                holder.row.setOnClickListener {
                    if (!isAnyDeleteRevealed()) {
                        onChooseFile(entry.file)
                    }
                }
                holder.row.setOnLongClickListener {
                    onDeleteEntry(entry)
                    true
                }
            }
            holder.deleteView.setOnClickListener { onDeleteTap(entry) }
        }

        private fun selectableItemBackground(context: android.content.Context): Int {
            val typedArray = context.obtainStyledAttributes(intArrayOf(android.R.attr.selectableItemBackground))
            return try {
                typedArray.getResourceId(0, 0)
            } finally {
                typedArray.recycle()
            }
        }

        inner class EntryViewHolder(
            val row: View,
            val titleView: TextView,
            val subtitleView: TextView,
            val badgeView: TextView,
            val arrowView: ImageView,
            val deleteView: TextView,
        ) : RecyclerView.ViewHolder(row)
    }

    private class SimpleDividerDecoration(
        color: Int,
        private val insetStart: Int,
    ) : RecyclerView.ItemDecoration() {
        private val drawable = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            setColor(ColorUtils.setAlphaComponent(color, 72))
        }

        override fun onDrawOver(c: Canvas, parent: RecyclerView, state: RecyclerView.State) {
            val childCount = parent.childCount
            for (index in 0 until childCount) {
                val child = parent.getChildAt(index)
                val position = parent.getChildAdapterPosition(child)
                if (position == RecyclerView.NO_POSITION || position >= state.itemCount - 1) {
                    continue
                }
                val top = child.bottom
                drawable.setBounds(
                    parent.paddingLeft + insetStart,
                    top,
                    parent.width - parent.paddingRight,
                    top + 1,
                )
                drawable.draw(c)
            }
        }
    }
}
