package com.lingxia.lxapp.APIs.media

import android.app.ActivityManager
import android.content.Context
import android.net.Uri
import android.os.Build
import com.lingxia.lxapp.APIs.media.player.PlayerEvent as CorePlayerEvent

/**
 * One item in a playlist. [url] is required; the display-property fields are
 * optional per-item overrides. When `null`, the item inherits whatever
 * element-level value [LxMediaPlayer] currently has — this is the path
 * lx-video takes (its JS-facing `playlist: string[]` API maps to items with
 * all overrides `null`, so element-level `objectFit` / `rotateDegrees` keep
 * applying uniformly across the whole list).
 *
 * Per-item overrides are used by application scenarios that genuinely need
 * different display per item (e.g. MediaPreview, where each captured video
 * carries its own EXIF rotation and objectFit).
 */
internal data class LxMediaPlaylistItem(
    val url: String,
    val objectFit: LxMediaObjectFit? = null,
    val rotateDegrees: Int? = null,
)

/**
 * Owns playlist state for [LxMediaPlayer], translating high-level commands
 * (apply / next / previous / goToIndex) and engine events (Ended / Error /
 * FirstFrameRendered) into surface-level actions: load next source, emit
 * `playlistchange` / `playlistend`, drive the [VideoTransitionOverlay], and
 * prefetch upcoming first frames via [LocalVideoFrameCache].
 *
 * Source of truth for *current playlist index* is this controller's [index]
 * field, kept consistent by routing all advance through [navigateTo]. The
 * underlying [androidx.media3.exoplayer.ExoPlayer] sees a single
 * `setMediaItem` per advance — gapless behavior is achieved through the
 * overlay rather than a multi-item playlist API. This trade-off keeps the
 * single-source code path unchanged and lets the controller be a focused,
 * additive layer.
 *
 * Visual contract — never a black frame visible:
 * - On any transition we show the overlay with the next item's first-frame
 *   bitmap (cached or extracted on the fly via [LocalVideoFrameCache]).
 * - Overlay hides as soon as ExoPlayer renders the new item's first frame.
 * - For non-local items the overlay short-circuits — SurfaceView's natural
 *   last-frame retention covers the (typically <100ms) gap.
 *
 * Async show races are resolved by a [transitionGeneration] counter: a stale
 * load callback that races a new transition or first-frame is suppressed.
 */
internal class LxMediaPlaylistController(private val host: PlaylistHost) {

    /**
     * Minimal interface [LxMediaPlayer] implements so the controller can
     * drive playback without taking a hard dependency on the player class.
     */
    interface PlaylistHost {
        val context: Context
        /** Current `loop` attribute state. */
        val isLoopEnabled: Boolean
        /** Resolve a raw src string (`http(s)://`, `lx://...`, raw path) to a Uri. */
        fun parseUri(src: String): Uri?
        /** Equivalent to the existing [LxMediaPlayer.loadSource]; loads via PlayerCore. */
        fun loadSourceForPlaylist(uri: Uri)
        /** Equivalent to the existing [LxMediaPlayer.play]. */
        fun playFromPlaylist()
        /** Emit a typed media event (playlistchange / playlistend). */
        fun emit(event: LxMediaEvent)
        /**
         * Apply per-item display overrides (objectFit / rotateDegrees) before
         * the next source loads. Item fields that are `null` MUST be left
         * untouched — that is how lx-video keeps element-level display
         * settings effective uniformly across the playlist.
         */
        fun applyItemDisplay(item: LxMediaPlaylistItem)
    }

    private var items: List<LxMediaPlaylistItem> = emptyList()
    private var index: Int = 0
    private var overlay: VideoTransitionOverlay? = null

    /** Monotonic counter; bumped on every transition. Stale async loads check this. */
    private var transitionGeneration: Long = 0L
    /** Set on FirstFrameRendered, reset when we initiate a new transition. */
    private var engineHasRenderedFirstFrame: Boolean = false

    /**
     * Whether the controller should auto-advance on Ended / Error. lx-video
     * needs this (the whole point of a playlist element). MediaPreview drives
     * advance from the ViewPager and disables this — otherwise the controller
     * would queue up the next video's audio while the UI is on a different
     * (image) page or stays put (MANUAL preview mode).
     */
    var autoAdvance: Boolean = true

    val isActive: Boolean get() = items.size > 1
    val currentIndex: Int get() = index
    val currentUrl: String? get() = items.getOrNull(index)?.url

    fun bindOverlay(overlay: VideoTransitionOverlay) { this.overlay = overlay }

    /**
     * Apply or update the playlist. Returns `true` if the item list changed
     * and the caller should expect playback state to be reset (handled here
     * via [PlaylistHost.loadSourceForPlaylist] for the starting item).
     *
     * No-op when the item list is unchanged (structural equality) — this
     * lets the caller invoke us on every `update(config)` without worrying
     * about restart-mid-stream.
     *
     * [startingIndex] lets the caller open straight at a non-zero position
     * (e.g. MediaPreview opening on the second video) without first loading
     * item 0 and then immediately jumping. Defaults to 0 for the lx-video
     * cold-start case where playback always begins at the top of the list.
     * Honored only when the item list actually changes — for repositioning
     * within an unchanged list use [goToIndex] from outside the controller.
     */
    fun apply(newItems: List<LxMediaPlaylistItem>, startingIndex: Int = 0): Boolean {
        if (newItems.isEmpty()) {
            deactivate()
            return false
        }
        if (newItems == items) return false
        // Defensive copy: caller-owned list could be mutated after this returns
        // and silently corrupt our state. Cheap O(N) for the typical playlist size.
        items = ArrayList(newItems)
        index = startingIndex.coerceIn(0, newItems.size - 1)
        transitionGeneration += 1
        engineHasRenderedFirstFrame = false

        val first = newItems[index]
        host.applyItemDisplay(first)

        // Cold-start overlay — shown synchronously if cache hits, async
        // otherwise (gen-checked when the load resolves).
        showOverlayFor(index, transitionGeneration)

        val firstUri = host.parseUri(first.url) ?: return true
        host.loadSourceForPlaylist(firstUri)
        triggerWindowPrefetch()
        return true
    }

    /** Switch out of playlist mode. Safe to call when not active. */
    fun deactivate() {
        if (items.isEmpty()) return
        items = emptyList()
        index = 0
        overlay?.hide()
    }

    fun next() {
        if (!isActive) return
        val target = if (host.isLoopEnabled) (index + 1) % items.size
        else (index + 1).coerceAtMost(items.size - 1)
        if (target == index) return
        navigateTo(target, "manual")
    }

    fun previous() {
        if (!isActive) return
        val n = items.size
        val target = if (host.isLoopEnabled) ((index - 1) % n + n) % n
        else (index - 1).coerceAtLeast(0)
        if (target == index) return
        navigateTo(target, "manual")
    }

    fun goToIndex(target: Int) {
        if (!isActive) return
        if (target !in items.indices) return
        if (target == index) return
        navigateTo(target, "manual")
    }

    /**
     * Engine-event hook called from [LxMediaPlayer.handleCoreEvent] AFTER the
     * normal event has been dispatched to JS. Drives auto-advance, error
     * recovery, and overlay hide-on-first-frame.
     */
    fun onCoreEvent(event: CorePlayerEvent) {
        if (!isActive) {
            // Even out of playlist mode, hide overlay on first frame so a
            // straggler `apply → deactivate` doesn't leave it visible.
            if (event is CorePlayerEvent.FirstFrameRendered ||
                event is CorePlayerEvent.Playing
            ) {
                overlay?.hide()
            }
            return
        }
        when (event) {
            is CorePlayerEvent.Ended -> if (autoAdvance) advanceOnTerminal("ended")
            is CorePlayerEvent.Error -> if (autoAdvance) advanceOnTerminal("error")
            is CorePlayerEvent.FirstFrameRendered,
            is CorePlayerEvent.Playing -> {
                engineHasRenderedFirstFrame = true
                overlay?.hide()
            }
            else -> Unit
        }
    }

    /** Called from [LxMediaPlayer.unmount]. */
    fun release() {
        items = emptyList()
        index = 0
        overlay = null
    }

    // ---------- internal ----------

    private fun advanceOnTerminal(reason: String) {
        val isLast = index >= items.size - 1
        if (isLast && !host.isLoopEnabled) {
            host.emit(LxMediaEvent.PlaylistEnd(index, items[index].url))
            return
        }
        val target = if (isLast) 0 else index + 1
        navigateTo(target, reason)
    }

    private fun navigateTo(target: Int, reason: String) {
        index = target
        transitionGeneration += 1
        engineHasRenderedFirstFrame = false

        val item = items[target]
        host.emit(LxMediaEvent.PlaylistChange(target, item.url, reason))

        // Apply per-item display props BEFORE loading the source so the new
        // frame, when it arrives, is laid out correctly from the first paint.
        host.applyItemDisplay(item)

        showOverlayFor(target, transitionGeneration)

        host.parseUri(item.url)?.let { uri ->
            host.loadSourceForPlaylist(uri)
            host.playFromPlaylist()
        }
        triggerWindowPrefetch()
    }

    /**
     * Show the overlay with the bitmap for [targetIndex]. Sync if cache hit,
     * async otherwise. The async path is gen- and first-frame-guarded so a
     * stale load can't paint over live video on a subsequent transition.
     *
     * For non-local URIs this is a no-op — SurfaceView retention is the
     * fallback (the previous item's last frame stays on screen for the brief
     * codec re-init gap).
     */
    private fun showOverlayFor(targetIndex: Int, gen: Long) {
        val overlay = overlay ?: return
        val item = items.getOrNull(targetIndex) ?: return
        val uri = host.parseUri(item.url) ?: return
        if (!isLocalUri(uri)) return

        LocalVideoFrameCache.peek(host.context, uri)?.let { cached ->
            if (gen == transitionGeneration) overlay.show(cached)
            return
        }
        LocalVideoFrameCache.load(host.context, uri) { bitmap ->
            if (bitmap == null) return@load
            // Suppress if we've moved on or the engine has already rendered.
            if (gen != transitionGeneration) return@load
            if (engineHasRenderedFirstFrame) return@load
            overlay.show(bitmap)
        }
    }

    /**
     * Prefetch first-frame bitmaps for the items most likely to be jumped to
     * next. Window size: 1 on constrained devices, 2 otherwise — small enough
     * that the cache (capped at ~6-8MB) doesn't churn, large enough to cover
     * the common next/prev pattern.
     */
    private fun triggerWindowPrefetch() {
        val ctx = host.context
        val windowSize = if (isConstrainedDevice(ctx)) 1 else 2
        val n = items.size
        for (offset in 1..windowSize) {
            val nextIdx = if (host.isLoopEnabled) (index + offset) % n else index + offset
            if (nextIdx !in items.indices) break
            val uri = host.parseUri(items[nextIdx].url) ?: continue
            if (isLocalUri(uri)) LocalVideoFrameCache.prefetch(ctx, uri)
        }
    }

    private fun isConstrainedDevice(context: Context): Boolean {
        if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.LOLLIPOP_MR1) return true
        val mgr = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        if (mgr?.isLowRamDevice == true) return true
        return Runtime.getRuntime().maxMemory() <= 256L * 1024L * 1024L
    }
}
