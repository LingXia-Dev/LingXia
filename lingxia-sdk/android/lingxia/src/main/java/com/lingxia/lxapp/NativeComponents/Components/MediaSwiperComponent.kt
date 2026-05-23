package com.lingxia.lxapp.NativeComponents.Components

import android.content.Context
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import com.lingxia.lxapp.APIs.media.LxMediaCommand
import com.lingxia.lxapp.APIs.media.LxMediaEvent
import com.lingxia.lxapp.APIs.media.LxMediaObjectFit
import com.lingxia.lxapp.APIs.media.LxMediaPlayer
import com.lingxia.lxapp.APIs.media.LxMediaPlayerConfig
import com.lingxia.app.LxApp
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.NativeComponents.LxNativeComponent
import com.lingxia.lxapp.NativeComponents.LxNativeComponentFactory
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import kotlin.math.max

private const val SWIPER_TAG = "LxMediaSwiper"

internal class MediaSwiperComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit) =
        MediaSwiperComponent(id, initialProps, eventSink)
}

internal class MediaSwiperComponent(
    override val id: String,
    private val initialProps: Map<String, Any?>,
    private val eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {
    private var context: Context? = null
    private val root: FrameLayout by lazy {
        FrameLayout(context!!).apply { setBackgroundColor(Color.BLACK) }
    }
    override val view: View get() = root

    private var pager: ViewPager2? = null
    private var dotsView: LinearLayout? = null
    private var adapter: MediaSwiperAdapter? = null
    private var config = MediaSwiperConfig()
    private var currentIndex = 0
    private var lastFrame: RectF? = null
    private var suppressNextSelection = false
    private var pendingTransitionPrevious: Int? = null
    private var pendingTransitionSource: String? = null
    private var autoplayRunnable: Runnable? = null
    private val mainHandler = Handler(Looper.getMainLooper())

    override fun mount(host: ViewGroup) {
        context = LxApp.getCurrentActivity() ?: host.context
        root.layoutParams = FrameLayout.LayoutParams(1, 1)
        host.addView(root)
        setupPager()
        update(initialProps)
    }

    override fun update(props: Map<String, Any?>) {
        val previousConfig = config
        val previousItems = config.items
        val previousIndex = currentIndex
        val next = MediaSwiperConfig.from(props, config)
        config = next
        val pager = pager ?: run {
            Log.w(SWIPER_TAG, "[$id] update before pager setup; items=${next.items.size}")
            return
        }
        if (previousItems != next.items) {
            val priorItem = previousItems.getOrNull(previousIndex)
            adapter?.release()
            adapter = MediaSwiperAdapter(
                componentId = id,
                items = next.items,
                configProvider = { config },
                eventSink = ::handlePageEvent,
                tapSink = tapSink@{ index ->
                    val item = config.items.getOrNull(index) ?: return@tapSink
                    emit("tap", mapOf("index" to index, "item" to item.toPayload()))
                }
            )
            pager.adapter = adapter
            currentIndex = resolveIndexForItemsChange(next, previousItems, previousIndex, priorItem)
            adapter?.lastSettledIndex = currentIndex
            adapter?.setCurrentIndex(currentIndex)
            suppressNextSelection = true
            pager.setCurrentItem(currentIndex, false)
            updateDots()
        } else {
            // Only props changed; renderKey rebinds individual holders only when item-affecting
            // props change. Avoid notifyDataSetChanged here so video players survive prop updates.
            val controlledIndex = next.index
            if (controlledIndex != null) {
                val resolved = controlledIndex.coerceIn(0, max(0, next.items.lastIndex))
                if (resolved != currentIndex) {
                    currentIndex = resolved
                    adapter?.lastSettledIndex = resolved
                    adapter?.setCurrentIndex(resolved)
                    suppressNextSelection = true
                    pager.setCurrentItem(resolved, false)
                    updateDots()
                }
            }
        }

        pager.orientation = if (next.direction == "vertical") ViewPager2.ORIENTATION_VERTICAL else ViewPager2.ORIENTATION_HORIZONTAL
        pager.isUserInputEnabled = next.swipeEnabled
        pager.offscreenPageLimit = if (next.items.size > 1) 1 else ViewPager2.OFFSCREEN_PAGE_LIMIT_DEFAULT
        applyPeekPadding(pager, next)
        if (previousIndex != currentIndex) updateDots()
        // Only force-restart the autoplay timer when something the timer cares
        // about actually changed; otherwise let the existing countdown keep
        // running. Without this guard, frequent rect/layout prop updates were
        // resetting the timer faster than `interval` and autoplay never fired.
        val autoplayConfigChanged =
            previousItems != next.items ||
            previousIndex != currentIndex ||
            previousConfig.autoplay != next.autoplay ||
            previousConfig.loop != next.loop ||
            previousConfig.interval != next.interval
        if (autoplayConfigChanged) restartAutoplay() else scheduleAutoplay()
    }

    /**
     * ViewPager2's snap behaviour stays at item boundaries even when the inner
     * RecyclerView has horizontal/vertical padding with `clipToPadding=false`. With
     * MATCH_PARENT page items, each item measures to (container - padding) so the
     * peek values become the visible slice of the previous/next page on each side.
     */
    private fun applyPeekPadding(pager: ViewPager2, cfg: MediaSwiperConfig) {
        val rv = pager.getChildAt(0) as? RecyclerView ?: return
        val density = pager.context.resources.displayMetrics.density
        val previousPx = (cfg.peekPrevious * density).toInt().coerceAtLeast(0)
        val nextPx = (cfg.peekNext * density).toInt().coerceAtLeast(0)
        if (cfg.direction == "vertical") {
            rv.setPadding(0, previousPx, 0, nextPx)
        } else {
            rv.setPadding(previousPx, 0, nextPx, 0)
        }
        rv.clipToPadding = previousPx == 0 && nextPx == 0
    }

    override fun setFrame(frame: RectF) {
        lastFrame = RectF(frame)
        val lp = root.layoutParams as? FrameLayout.LayoutParams
        val width = frame.width().toInt()
        val height = frame.height().toInt()
        if (lp == null || lp.width != width || lp.height != height) {
            root.layoutParams = FrameLayout.LayoutParams(width, height)
        }
        root.translationX = frame.left
        root.translationY = frame.top
        adapter?.setPageSize(width, height)
    }

    override fun focus() {
        lastFrame?.let { setFrame(it) }
    }

    override fun blur() {}

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        when (name) {
            "next" -> goBy(1, "api")
            "previous" -> goBy(-1, "api")
            "goToIndex" -> {
                val index = (params?.get("index") as? Number)?.toInt() ?: return
                if (index !in config.items.indices) return
                goTo(index, "api", animated = config.animation != "none")
            }
        }
    }

    override fun unmount() {
        stopAutoplay()
        adapter?.release()
        adapter = null
        pager?.adapter = null
        pager = null
        root.removeAllViews()
        (root.parent as? ViewGroup)?.removeView(root)
    }

    private fun setupPager() {
        val ctx = context ?: return
        val pager = ViewPager2(ctx).apply {
            layoutParams = FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
            offscreenPageLimit = 1
            registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
                override fun onPageSelected(position: Int) {
                    if (suppressNextSelection) {
                        suppressNextSelection = false
                        this@MediaSwiperComponent.adapter?.setCurrentIndex(position)
                        updateDots()
                        return
                    }
                    val previous = currentIndex
                    currentIndex = position
                    this@MediaSwiperComponent.adapter?.setCurrentIndex(position)
                    updateDots()
                    if (previous != position) {
                        pendingTransitionPrevious = previous
                        pendingTransitionSource = "touch"
                        emitChange(position, previous, "touch")
                        restartAutoplay()
                    }
                }

                override fun onPageScrollStateChanged(state: Int) {
                    if (state == ViewPager2.SCROLL_STATE_IDLE) {
                        val previous = pendingTransitionPrevious ?: this@MediaSwiperComponent.adapter?.lastSettledIndex ?: currentIndex
                        val source = pendingTransitionSource ?: "touch"
                        pendingTransitionPrevious = null
                        pendingTransitionSource = null
                        this@MediaSwiperComponent.adapter?.lastSettledIndex = currentIndex
                        if (previous != currentIndex) {
                            emit("transitionend", changeDetail(currentIndex, previous, source))
                        }
                    }
                }
            })
        }
        this.pager = pager
        root.addView(pager)
        dotsView = LinearLayout(ctx).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            ).apply { bottomMargin = (12 * ctx.resources.displayMetrics.density).toInt() }
            // ViewPager2 wraps a RecyclerView whose item views can sit on top of overlay
            // siblings depending on draw order. Elevation guarantees dots render above
            // the page content, and bringToFront keeps z-order stable across re-layouts.
            elevation = 4f
        }
        root.addView(dotsView)
        dotsView?.bringToFront()
    }

    private fun resolveInitialIndex(config: MediaSwiperConfig): Int {
        val raw = config.index ?: config.initialIndex
        return raw.coerceIn(0, max(0, config.items.lastIndex))
    }

    /**
     * When items change, prefer keeping the user on the same logical item (matched by id) so a
     * dynamic list update does not reset their position. Falls back to controlled index, then
     * initialIndex, when the prior item disappears or no prior item existed.
     */
    private fun resolveIndexForItemsChange(
        next: MediaSwiperConfig,
        previousItems: List<MediaSwiperItem>,
        previousIndex: Int,
        priorItem: MediaSwiperItem?,
    ): Int {
        val controlled = next.index
        if (controlled != null) return controlled.coerceIn(0, max(0, next.items.lastIndex))
        if (priorItem != null && previousItems.isNotEmpty()) {
            val matched = next.items.indexOfFirst { it.id == priorItem.id }
            if (matched >= 0) return matched
        }
        return resolveInitialIndex(next)
    }

    private fun goBy(delta: Int, source: String) {
        val itemCount = config.items.size
        if (itemCount == 0) return
        val target = currentIndex + delta
        if (target !in 0 until itemCount) {
            if (config.loop && itemCount > 1) {
                goTo(if (delta > 0) 0 else itemCount - 1, source, animated = config.animation != "none")
            } else {
                emit("endreached", mapOf("index" to currentIndex, "item" to config.items[currentIndex].toPayload(), "source" to source))
                if (source == "autoplay") stopAutoplay()
            }
            return
        }
        goTo(target, source, animated = config.animation != "none")
        if (source == "autoplay" && !config.loop && target == itemCount - 1) {
            emit("endreached", mapOf("index" to target, "item" to config.items[target].toPayload(), "source" to source))
            stopAutoplay()
        }
    }

    private fun goTo(target: Int, source: String, animated: Boolean) {
        if (target == currentIndex) return
        val previous = currentIndex
        currentIndex = target
        adapter?.setCurrentIndex(target)
        suppressNextSelection = true
        if (animated) {
            pendingTransitionPrevious = previous
            pendingTransitionSource = source
        } else {
            pendingTransitionPrevious = null
            pendingTransitionSource = null
        }
        pager?.setCurrentItem(target, animated)
        emitChange(target, previous, source)
        if (!animated) {
            adapter?.lastSettledIndex = target
            emit("transitionend", changeDetail(target, previous, source))
        }
        updateDots()
        restartAutoplay()
    }

    private fun emitChange(index: Int, previous: Int, source: String) {
        emit("change", changeDetail(index, previous, source))
    }

    private fun changeDetail(index: Int, previous: Int, source: String): Map<String, Any> {
        val item = config.items.getOrNull(index)?.toPayload() ?: emptyMap<String, Any>()
        return mapOf("index" to index, "previousIndex" to previous, "item" to item, "source" to source)
    }

    private fun emit(event: String, detail: Map<String, Any>) {
        eventSink(mapOf("event" to event, "detail" to detail))
    }

    private fun handlePageEvent(pageIndex: Int, event: LxMediaEvent) {
        when (event) {
            is LxMediaEvent.Ended -> {
                if (pageIndex != currentIndex) return
                val item = config.items.getOrNull(pageIndex) ?: return
                emit("videoended", mapOf("index" to pageIndex, "item" to item.toPayload()))
            }
            is LxMediaEvent.Error -> emit(
                "error",
                mapOf(
                    "index" to pageIndex,
                    "item" to config.items.getOrNull(pageIndex)?.toPayload().orEmpty(),
                    "code" to mapErrorCode(event.code),
                    "message" to event.message
                )
            )
            else -> Unit
        }
    }

    private fun mapErrorCode(code: String): String = when (code.lowercase()) {
        "not_found", "network", "decode", "unsupported_format", "permission_denied" -> code.lowercase()
        else -> "unknown"
    }

    private fun updateDots() {
        val dots = dotsView ?: return
        val dotsConfig = config.dots ?: run {
            dots.visibility = View.GONE
            return
        }
        val ctx = dots.context
        dots.visibility = if (config.items.size > 1) View.VISIBLE else View.GONE
        dots.removeAllViews()
        val density = ctx.resources.displayMetrics.density
        dots.orientation = if (config.direction == "vertical") LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
        dots.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            if (config.direction == "vertical") Gravity.END or Gravity.CENTER_VERTICAL else Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
        ).apply {
            if (config.direction == "vertical") {
                rightMargin = (12 * density).toInt()
            } else {
                bottomMargin = (12 * density).toInt()
            }
        }
        val size = (6 * density).toInt().coerceAtLeast(4)
        val margin = (4 * density).toInt()
        for (i in config.items.indices) {
            val dot = View(ctx).apply {
                background = dotDrawable(if (i == currentIndex) dotsConfig.activeColor else dotsConfig.color)
                layoutParams = LinearLayout.LayoutParams(size, size).apply {
                    if (config.direction == "vertical") {
                        topMargin = margin
                        bottomMargin = margin
                    } else {
                        leftMargin = margin
                        rightMargin = margin
                    }
                }
            }
            dots.addView(dot)
        }
        dots.bringToFront()
    }

    private fun dotDrawable(color: Int): GradientDrawable =
        GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color)
        }

    /**
     * Idempotent autoplay scheduler. Repeated calls with no relevant state change
     * preserve the existing timer instead of cancelling it — without this,
     * rect-only prop updates (which fire frequently during layout settle) would
     * reset the countdown to `interval` every time and the timer would never
     * actually fire on the user. Call sites that follow a real page change
     * (onPageSelected, goTo) use `restart()` instead to start a fresh countdown.
     */
    private fun scheduleAutoplay() {
        if (!config.autoplay || config.items.size <= 1) {
            stopAutoplay()
            return
        }
        if (!config.loop && currentIndex >= config.items.lastIndex) {
            stopAutoplay()
            return
        }
        if (autoplayRunnable != null) return
        val task = Runnable { goBy(1, "autoplay") }
        autoplayRunnable = task
        mainHandler.postDelayed(task, config.interval.toLong())
    }

    /** Force-restart the autoplay countdown — call after a real page change. */
    private fun restartAutoplay() {
        stopAutoplay()
        scheduleAutoplay()
    }

    private fun stopAutoplay() {
        autoplayRunnable?.let { mainHandler.removeCallbacks(it) }
        autoplayRunnable = null
    }
}

private data class MediaSwiperConfig(
    val items: List<MediaSwiperItem> = emptyList(),
    val index: Int? = null,
    val initialIndex: Int = 0,
    val loop: Boolean = false,
    val autoplay: Boolean = false,
    val interval: Int = 5000,
    val animation: String = "slide",
    val direction: String = "horizontal",
    val rotate: Int = 0,
    val objectFit: LxMediaObjectFit = LxMediaObjectFit.COVER,
    val controls: Boolean = false,
    val muted: Boolean = true,
    val dots: DotsConfig? = null,
    val swipeEnabled: Boolean = true,
    /** Peek in CSS pixels, applied as padding on the inner RecyclerView so adjacent
     *  pages render in the gutter while paging snaps remain at item boundaries. */
    val peekPrevious: Int = 0,
    val peekNext: Int = 0,
) {
    companion object {
        fun from(props: Map<String, Any?>, previous: MediaSwiperConfig): MediaSwiperConfig {
            return MediaSwiperConfig(
                items = parseItems(props["items"]) ?: previous.items,
                index = (props["index"] as? Number)?.toInt(),
                initialIndex = (props["initialIndex"] as? Number)?.toInt()
                    ?: (props["initial-index"] as? Number)?.toInt()
                    ?: previous.initialIndex,
                loop = props["loop"] as? Boolean ?: previous.loop,
                autoplay = props["autoplay"] as? Boolean ?: previous.autoplay,
                interval = ((props["interval"] as? Number)?.toInt() ?: previous.interval).coerceAtLeast(500),
                animation = (props["animation"] as? String)?.takeIf { it == "none" || it == "slide" } ?: previous.animation,
                direction = (props["direction"] as? String)?.takeIf { it == "vertical" || it == "horizontal" } ?: previous.direction,
                rotate = ((props["contentRotate"] as? Number)?.toInt() ?: previous.rotate).let { if (it in setOf(0, 90, 180, 270)) it else 0 },
                objectFit = (props["objectFit"] as? String)?.let { LxMediaObjectFit.fromString(it) } ?: previous.objectFit,
                controls = props["controls"] as? Boolean ?: previous.controls,
                muted = props["muted"] as? Boolean ?: previous.muted,
                dots = parseDots(props["dots"], previous.dots),
                swipeEnabled = props["swipeEnabled"] as? Boolean ?: previous.swipeEnabled,
                peekPrevious = parsePeekSide(props["peek"], "previous", previous.peekPrevious),
                peekNext = parsePeekSide(props["peek"], "next", previous.peekNext),
            )
        }

        private fun parsePeekSide(value: Any?, side: String, fallback: Int): Int {
            return when (value) {
                is Number -> value.toInt().coerceAtLeast(0)
                is Map<*, *> -> (value[side] as? Number)?.toInt()?.coerceAtLeast(0) ?: fallback
                null -> fallback
                else -> fallback
            }
        }

        private fun parseItems(value: Any?): List<MediaSwiperItem>? {
            val raw = value as? List<*> ?: return null
            return raw.mapIndexedNotNull { index, entry ->
                val map = entry as? Map<*, *> ?: return@mapIndexedNotNull null
                val type = map["type"] as? String ?: return@mapIndexedNotNull null
                val src = (map["src"] as? String)?.takeIf { it.isNotBlank() } ?: return@mapIndexedNotNull null
                val id = map["id"] as? String ?: "$type:$src:$index"
                when (type) {
                    "image" -> MediaSwiperItem(id, MediaSwiperItemType.IMAGE, src)
                    "video" -> MediaSwiperItem(
                        id = id,
                        type = MediaSwiperItemType.VIDEO,
                        src = src,
                        poster = map["poster"] as? String,
                        controls = map["controls"] as? Boolean,
                        muted = map["muted"] as? Boolean
                    )
                    else -> null
                }
            }
        }

        private fun parseDots(value: Any?, previous: DotsConfig?): DotsConfig? {
            return when (value) {
                is Boolean -> if (value) DotsConfig() else null
                is Map<*, *> -> DotsConfig(
                    color = parseColor(value["color"] as? String, 0x66FFFFFF),
                    activeColor = parseColor(value["activeColor"] as? String, Color.WHITE),
                )
                null -> previous
                else -> previous
            }
        }

        private fun parseColor(value: String?, fallback: Int): Int {
            if (value.isNullOrBlank()) return fallback
            val trimmed = value.trim()
            // Handle CSS-style #RRGGBBAA (Android's Color.parseColor expects #AARRGGBB,
            // which doesn't match what JS callers pass via the `dots` prop).
            if (trimmed.length == 9 && trimmed.startsWith("#")) {
                return try {
                    val r = trimmed.substring(1, 3).toInt(16)
                    val g = trimmed.substring(3, 5).toInt(16)
                    val b = trimmed.substring(5, 7).toInt(16)
                    val a = trimmed.substring(7, 9).toInt(16)
                    Color.argb(a, r, g, b)
                } catch (_: Exception) { fallback }
            }
            return try { Color.parseColor(trimmed) } catch (_: Exception) { fallback }
        }
    }
}

private data class DotsConfig(
    val color: Int = 0x66FFFFFF,
    val activeColor: Int = Color.WHITE,
)

private enum class MediaSwiperItemType { IMAGE, VIDEO }

private data class MediaSwiperItem(
    val id: String,
    val type: MediaSwiperItemType,
    val src: String,
    val poster: String? = null,
    val controls: Boolean? = null,
    val muted: Boolean? = null,
) {
    fun toPayload(): Map<String, Any> {
        val out = mutableMapOf<String, Any>(
            "id" to id,
            "type" to if (type == MediaSwiperItemType.VIDEO) "video" else "image",
            "src" to src
        )
        poster?.let { out["poster"] = it }
        controls?.let { out["controls"] = it }
        muted?.let { out["muted"] = it }
        return out
    }
}

private class MediaSwiperAdapter(
    private val componentId: String,
    private val items: List<MediaSwiperItem>,
    private val configProvider: () -> MediaSwiperConfig,
    private val eventSink: (Int, LxMediaEvent) -> Unit,
    private val tapSink: (Int) -> Unit
) : RecyclerView.Adapter<MediaSwiperAdapter.Holder>() {
    private val holders = mutableSetOf<Holder>()
    var lastSettledIndex: Int = 0
    private var currentIndex: Int = 0
    private var pageWidth = 0
    private var pageHeight = 0

    init {
        setHasStableIds(true)
    }

    override fun getItemCount(): Int = items.size

    override fun getItemId(position: Int): Long = items[position].id.hashCode().toLong()

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
        val container = FrameLayout(parent.context).apply {
            layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
            setBackgroundColor(Color.BLACK)
        }
        return Holder(componentId, container, configProvider, eventSink, tapSink).also { holders.add(it) }
    }

    override fun onBindViewHolder(holder: Holder, position: Int) {
        holder.bind(items[position], position, pageWidth, pageHeight, position == currentIndex)
        if (position == currentIndex) holder.onVisible() else holder.onHidden()
    }

    override fun onViewRecycled(holder: Holder) {
        holder.clear()
        holders.remove(holder)
        super.onViewRecycled(holder)
    }

    fun setCurrentIndex(index: Int) {
        currentIndex = index
        // Use our own `boundIndex` instead of RecyclerView's bindingAdapterPosition
        // because the latter can transiently return NO_POSITION mid-scroll, which
        // would skip the onVisible fan-out and leave the newly-active video page
        // black (pendingVideoItem stays unset, player never attached).
        holders.forEach { holder ->
            if (holder.boundPosition == index) holder.onVisible() else holder.onHidden()
        }
    }

    fun setPageSize(width: Int, height: Int) {
        pageWidth = width
        pageHeight = height
        holders.forEach { it.setPageSize(width, height) }
    }

    fun release() {
        holders.forEach { it.clear() }
        holders.clear()
    }

    class Holder(
        private val componentId: String,
        private val container: FrameLayout,
        private val configProvider: () -> MediaSwiperConfig,
        private val eventSink: (Int, LxMediaEvent) -> Unit,
        private val tapSink: (Int) -> Unit
    ) : RecyclerView.ViewHolder(container) {
        private var imageView: ImageView? = null
        private var player: LxMediaPlayer? = null
        private var boundItem: MediaSwiperItem? = null
        private var boundIndex: Int = RecyclerView.NO_POSITION
        private var boundRenderKey: String? = null
        private var pageWidth = 0
        private var pageHeight = 0
        /** Stable position view, unaffected by RecyclerView's mid-scroll NO_POSITION blips. */
        val boundPosition: Int
            get() = boundIndex

        private var configuredVolume: Float = 1.0f
        private var configuredMuted: Boolean = true

        fun bind(item: MediaSwiperItem, index: Int, width: Int, height: Int, isCurrent: Boolean) {
            val config = configProvider()
            val renderKey = renderKey(item, index, config)
            if (boundRenderKey == renderKey) return
            release()
            boundItem = item
            boundIndex = index
            boundRenderKey = renderKey
            pageWidth = width
            pageHeight = height
            when (item.type) {
                MediaSwiperItemType.IMAGE -> bindImage(item, config)
                MediaSwiperItemType.VIDEO -> attachVideoPlayer(item, index, config, isCurrent)
            }
        }

        fun setPageSize(width: Int, height: Int) {
            pageWidth = width
            pageHeight = height
            // player.view uses MATCH_PARENT layoutParams (see attachVideoPlayer),
            // so it sizes itself to the holder's container automatically — no
            // need to call player.setFrame() here. Calling it with explicit
            // pixel dimensions was the source of the 0×0 surface bug.
        }

        fun onVisible() {
            Log.i(SWIPER_TAG, "[$componentId] onVisible idx=$boundIndex hasPlayer=${player != null} " +
                "pageSize=${pageWidth}x${pageHeight}")
            // Restore volume per the user-configured muted state and resume the
            // active video. Hidden videos are paused in onHidden() so they do not
            // continue decoding or emit videoended while off-screen.
            player?.update(LxMediaPlayerConfig(muted = configuredMuted))
            player?.handle(LxMediaCommand.Play)
        }

        fun onHidden() {
            Log.i(SWIPER_TAG, "[$componentId] onHidden idx=$boundIndex hasPlayer=${player != null}")
            // Hidden videos must not keep playing in the background. Mute first
            // to avoid any short audio leak during transition, then pause.
            player?.update(LxMediaPlayerConfig(muted = true))
            player?.handle(LxMediaCommand.Pause)
        }

        fun release() {
            player?.detach()
            player = null
            imageView = null
            container.removeAllViews()
        }

        fun clear() {
            release()
            boundItem = null
            boundIndex = RecyclerView.NO_POSITION
            boundRenderKey = null
        }

        private fun bindImage(item: MediaSwiperItem, config: MediaSwiperConfig) {
            val requestedIndex = boundIndex
            val requestedItem = item
            val view = ImageView(container.context).apply {
                layoutParams = FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                setBackgroundColor(Color.BLACK)
                scaleType = scaleTypeFor(config.objectFit)
                rotation = config.rotate.toFloat()
            }
            imageView = view
            container.addView(view)
            ImageLoader.load(container.context, item.src, view) { errorCode ->
                if (errorCode != null && boundIndex == requestedIndex && boundItem == requestedItem) {
                    Log.w(SWIPER_TAG, "[$componentId] image error idx=$requestedIndex code=$errorCode src=${item.src}")
                    eventSink(requestedIndex, LxMediaEvent.Error(errorCode, "image load failed"))
                }
            }
            view.setOnClickListener {
                if (boundIndex != RecyclerView.NO_POSITION) tapSink(boundIndex)
            }
        }

        private fun attachVideoPlayer(item: MediaSwiperItem, index: Int, config: MediaSwiperConfig, isCurrent: Boolean) {
            val player = LxMediaPlayer(container.context, eventSink = {}, typedEventSink = { event ->
                eventSink(index, event)
            }, componentId = "$componentId-video-$index")
            this.player = player
            container.addView(player.view)
            // Force MATCH_PARENT layoutParams instead of relying on the adapter's
            // `pageWidth`/`pageHeight` — those may still be 0 when ViewPager2
            // prefetches an offscreen holder before our setFrame propagation has
            // run. With 0×0 layoutParams, ExoPlayer's PlayerView renders the
            // video to a zero-sized surface (audio plays, `ended` fires, but the
            // user sees only the shutter overlay → "video shows black"). Using
            // MATCH_PARENT lets the player view size itself to the container,
            // matching how the image branch already works.
            player.view.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // For a single-video swiper, swiper.loop has no page-level meaning
            // (there's nothing else to cycle to) — fold it into video-level loop
            // so the same video replays. With multiple items, video.loop stays
            // false and swiper's autoplay timer drives cross-item transitions.
            val videoLoop = config.loop && config.items.size == 1
            // Mount all videos pre-muted; onVisible() unmutes the active one.
            // Without this, every video starts with audio and you hear all of
            // them mixed together until their first onHidden() arrives.
            configuredMuted = item.muted ?: config.muted
            Log.i(SWIPER_TAG, "[$componentId] attachVideoPlayer idx=$index src=${item.src} " +
                "pageSize=${pageWidth}x${pageHeight} loop=$videoLoop userMuted=$configuredMuted")
            player.update(
                LxMediaPlayerConfig(
                    src = item.src,
                    poster = item.poster,
                    controls = item.controls ?: config.controls,
                    muted = true,
                    loop = videoLoop,
                    objectFit = config.objectFit,
                    rotateDegrees = config.rotate,
                    autoplay = isCurrent
                )
            )
            if (!(item.controls ?: config.controls)) {
                player.view.setOnClickListener {
                    if (boundIndex != RecyclerView.NO_POSITION) tapSink(boundIndex)
                }
            }
        }

        private fun renderKey(item: MediaSwiperItem, index: Int, config: MediaSwiperConfig): String {
            val effectiveControls = item.controls ?: config.controls
            val effectiveMuted = item.muted ?: config.muted
            return listOf(
                index,
                item.id,
                item.type,
                item.src,
                item.poster.orEmpty(),
                effectiveControls,
                effectiveMuted,
                config.objectFit,
                config.rotate,
            ).joinToString("|")
        }

        private fun scaleTypeFor(fit: LxMediaObjectFit): ImageView.ScaleType = when (fit) {
            LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
            LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
        }
    }
}

private object ImageLoader {
    private val executor = Executors.newFixedThreadPool(2)
    private val mainHandler = Handler(Looper.getMainLooper())

    fun load(context: Context, src: String, target: ImageView, done: (String?) -> Unit) {
        // Capture the desired decode size on the calling thread so the worker block
        // doesn't touch the view from a background thread.
        val display = context.resources.displayMetrics
        val initialW = target.width.takeIf { it > 0 } ?: display.widthPixels
        val initialH = target.height.takeIf { it > 0 } ?: display.heightPixels
        executor.execute {
            var errorCode = "unknown"
            val bitmap = try {
                val resolved = resolveUri(context, src)
                when {
                    resolved.startsWith("http://") || resolved.startsWith("https://") -> {
                        val conn = URL(resolved).openConnection() as HttpURLConnection
                        conn.connectTimeout = 10_000
                        conn.readTimeout = 15_000
                        try {
                            if (conn.responseCode !in 200..299) {
                                errorCode = "network"
                                null
                            } else {
                                val decoded = conn.inputStream.use { BitmapFactory.decodeStream(it) }
                                if (decoded == null) errorCode = "decode"
                                decoded
                            }
                        } finally {
                            conn.disconnect()
                        }
                    }
                    else -> {
                        val path = Uri.parse(resolved).path ?: resolved
                        val file = File(path)
                        if (!file.exists()) {
                            errorCode = "not_found"
                            null
                        } else {
                            val decoded = decodeBitmapDownsampled(file.absolutePath, initialW, initialH)
                            if (decoded == null) errorCode = "decode"
                            decoded
                        }
                    }
                }
            } catch (_: Exception) {
                errorCode = "network"
                null
            }
            mainHandler.post {
                if (bitmap != null) target.setImageBitmap(bitmap)
                done(if (bitmap == null) errorCode else null)
            }
        }
    }

    /**
     * Decode a local image file with inSampleSize chosen so the result roughly fits the target
     * ImageView. Camera photos can be 12MP+ which OOMs a naive BitmapFactory.decodeFile and
     * silently returns null, surfacing as a black page.
     */
    private fun decodeBitmapDownsampled(path: String, targetW: Int, targetH: Int): android.graphics.Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        var sample = 1
        while (
            (bounds.outWidth / sample) > targetW * 2 ||
            (bounds.outHeight / sample) > targetH * 2
        ) {
            sample *= 2
        }
        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
        return try {
            BitmapFactory.decodeFile(path, opts)
        } catch (e: OutOfMemoryError) {
            Log.w(SWIPER_TAG, "decode OOM at sample=$sample for $path", e)
            null
        }
    }

    private fun resolveUri(context: Context, src: String): String {
        val raw = src.trim()
        if (raw.startsWith("http://") || raw.startsWith("https://")) return raw
        if (raw.startsWith("lx://", ignoreCase = true)) {
            val appId = LxApp.getCurrentActivity()?.getAppId()
            val resolved = appId?.let { NativeApi.resolveLxUri(it, raw) }
            if (!resolved.isNullOrBlank()) return resolved
        }
        if (raw.startsWith("/")) return raw
        return File(context.filesDir, raw).absolutePath
    }
}
