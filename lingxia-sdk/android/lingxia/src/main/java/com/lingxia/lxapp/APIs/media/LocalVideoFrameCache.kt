package com.lingxia.lxapp.APIs.media

import android.app.ActivityManager
import android.content.ComponentCallbacks2
import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import java.io.File
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.util.concurrent.TimeUnit
import kotlin.math.max

/**
 * Process-level cache for first-frame bitmaps extracted from local video files.
 *
 * Used by [MediaPreviewFragment] (during swipe prefetch) and [LxMediaPlayer]
 * (during playlist transitions) to bridge visual gaps when loading a new video
 * item. Decoding is independent of any [androidx.media3.exoplayer.ExoPlayer]
 * instance — uses [MediaMetadataRetriever], a system demuxer that costs ~10-50ms
 * per local file and holds no codec/buffer resources between calls.
 *
 * Memory & threading:
 * - Heap-sized LRU. Constrained devices (low-RAM or `memoryClass <= 256MB`)
 *   get a smaller budget and a single decode thread.
 * - Bitmaps are downscaled to screen size before caching — keeps a typical
 *   1080p frame at ~8MB instead of the source 4K's ~33MB.
 * - Registers [ComponentCallbacks2] once per process: evicts on critical pressure,
 *   trims to half on moderate.
 * - Decode thread runs at one notch below normal priority; never blocks UI.
 * - Same-URI concurrent loads are coalesced: single decode, callbacks fan out.
 * - On `OutOfMemoryError` during decode, evicts the cache and returns null.
 *
 * Cache invalidation:
 * - Keys for `file://` URIs include file size + last-modified time, so a mutated
 *   file at the same path produces a fresh decode.
 * - `content://` URIs use the URI string directly (provider-managed identity).
 *
 * Lifecycle:
 * - Singleton (`object`). Lazy-init on first call. Uses `applicationContext`,
 *   never the calling Activity, so no leaks. Deliberately no `release()` — the
 *   process-level lifetime is correct.
 */
internal object LocalVideoFrameCache {

    // Cache budget bounds. Mirrors preview's previous numbers (production-validated).
    private const val MIN_CACHE_BYTES = 1 * 1024 * 1024
    private const val MAX_CACHE_BYTES = 8 * 1024 * 1024
    private const val CONSTRAINED_MIN_CACHE_BYTES = 512 * 1024
    private const val CONSTRAINED_MAX_CACHE_BYTES = 6 * 1024 * 1024
    private const val CACHE_DIVISOR = 192L
    private const val CONSTRAINED_CACHE_DIVISOR = 384L
    private const val CONSTRAINED_HEAP_THRESHOLD_BYTES = 256L * 1024L * 1024L

    // Downscale targets. Frames larger than this on the long edge get scaled.
    private const val DOWNSCALE_MIN_EDGE_PX = 720
    private const val DOWNSCALE_MAX_EDGE_PX = 1080

    private val mainHandler = Handler(Looper.getMainLooper())

    @Volatile private var initialized = false
    private val initLock = Any()

    private lateinit var cache: LruCache<String, Bitmap>
    private lateinit var executor: ExecutorService

    // Coalesces in-flight decode requests for the same URI.
    private val inFlight = HashMap<String, MutableList<CallbackHandle>>()
    private val inFlightLock = Any()

    private class CallbackHandle(
        private val callback: (Bitmap?) -> Unit,
    ) : Future<Unit> {
        @Volatile private var cancelled = false

        override fun cancel(mayInterruptIfRunning: Boolean): Boolean {
            if (cancelled) return false
            cancelled = true
            return true
        }

        override fun isCancelled(): Boolean = cancelled

        override fun isDone(): Boolean = cancelled

        override fun get() = Unit

        override fun get(timeout: Long, unit: TimeUnit) = Unit

        fun dispatch(bitmap: Bitmap?) {
            if (!cancelled) callback(bitmap)
        }
    }

    /** Synchronous cache hit; null if absent. Cheap, safe on main thread. */
    fun peek(context: Context, uri: Uri): Bitmap? {
        if (!initialized) return null
        val key = buildKey(context, uri)
        return synchronized(cache) { cache.get(key) }
    }

    /**
     * Asynchronously extract the first frame for [uri]. Caller is expected to pass
     * a *local* URI (`file://`, `content://`, or a raw filesystem path). Remote URIs
     * aren't supported by [MediaMetadataRetriever] and will return null.
     *
     * [onResult] is always invoked on the main thread.
     * Cache hits dispatch via the main handler (single post).
     *
     * @return a callback handle that may be cancelled, or null if served from cache.
     */
    fun load(context: Context, uri: Uri, onResult: (Bitmap?) -> Unit): Future<*>? {
        ensureInit(context)
        val key = buildKey(context, uri)

        synchronized(cache) { cache.get(key) }?.let { cached ->
            mainHandler.post { onResult(cached) }
            return null
        }

        val handle = CallbackHandle(onResult)
        synchronized(inFlightLock) {
            inFlight[key]?.let { pending ->
                pending.add(handle)
                return handle
            }
            inFlight[key] = mutableListOf(handle)
        }

        val appContext = context.applicationContext
        val targetEdge = resolveDownscaleEdge(appContext)
        executor.execute {
            val bitmap = extractFirstFrame(appContext, uri, targetEdge)
            if (bitmap != null) {
                synchronized(cache) { cache.put(key, bitmap) }
            }
            val callbacks: List<CallbackHandle>
            synchronized(inFlightLock) {
                callbacks = inFlight.remove(key).orEmpty()
            }
            mainHandler.post { callbacks.forEach { it.dispatch(bitmap) } }
        }
        return handle
    }

    /**
     * Fire-and-forget prefetch. Skips if already cached, in-flight, or for a
     * non-local URI. Intended for proactive warming of upcoming playlist items.
     */
    fun prefetch(context: Context, uri: Uri) {
        if (!isExtractable(uri)) return
        if (peek(context, uri) != null) return
        val key = buildKey(context, uri)
        synchronized(inFlightLock) { if (inFlight.containsKey(key)) return }
        load(context, uri) { /* result is in cache */ }
    }

    /** True for URIs MediaMetadataRetriever can decode (file/content/raw path). */
    private fun isExtractable(uri: Uri): Boolean {
        val scheme = uri.scheme
        return scheme.isNullOrEmpty() ||
            scheme.equals("file", ignoreCase = true) ||
            scheme.equals("content", ignoreCase = true)
    }

    /** Manual eviction. Rarely needed — TrimMemory does it automatically. */
    fun evictAll() {
        if (!initialized) return
        synchronized(cache) { cache.evictAll() }
    }

    // ---------- internal ----------

    private fun ensureInit(context: Context) {
        if (initialized) return
        synchronized(initLock) {
            if (initialized) return
            val appContext = context.applicationContext
            val constrained = isConstrainedDevice(appContext)
            cache = object : LruCache<String, Bitmap>(resolveCacheSizeBytes(constrained)) {
                override fun sizeOf(key: String, value: Bitmap): Int =
                    value.byteCount.coerceAtLeast(1)
            }
            executor = Executors.newFixedThreadPool(if (constrained) 1 else 2) { runnable ->
                Thread(runnable, "LingXiaVideoFrame").apply {
                    isDaemon = true
                    priority = Thread.NORM_PRIORITY - 1
                }
            }
            registerMemoryCallbacks(appContext)
            initialized = true
        }
    }

    /**
     * Build a cache key. For file URIs, includes size + mtime so a mutated file
     * at the same path produces a fresh decode. For content URIs, includes the
     * package name to namespace across apps sharing the cache (defensive).
     */
    private fun buildKey(context: Context, uri: Uri): String {
        val scheme = uri.scheme?.lowercase()
        return if (scheme.isNullOrEmpty() || scheme == "file") {
            val path = uri.path.orEmpty()
            val file = File(path)
            if (path.isNotEmpty() && file.exists()) {
                "video:file:$path:${file.length()}:${file.lastModified()}"
            } else {
                "video:file:$path"
            }
        } else {
            "video:${context.applicationContext.packageName}|$uri"
        }
    }

    private fun extractFirstFrame(context: Context, uri: Uri, targetEdge: Int): Bitmap? {
        val retriever = MediaMetadataRetriever()
        return try {
            when {
                uri.scheme.isNullOrEmpty() || uri.scheme.equals("file", ignoreCase = true) -> {
                    val path = uri.path ?: return null
                    retriever.setDataSource(path)
                }
                uri.scheme.equals("content", ignoreCase = true) -> {
                    retriever.setDataSource(context, uri)
                }
                else -> return null
            }
            // Try OPTION_CLOSEST_SYNC first (fastest sync frame near 0); fall back
            // to OPTION_PREVIOUS_SYNC, then a non-keyframe pass as last resort.
            val frame = retriever.getFrameAtTime(0L, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
                ?: retriever.getFrameAtTime(0L, MediaMetadataRetriever.OPTION_PREVIOUS_SYNC)
                ?: retriever.getFrameAtTime(-1L)
                ?: return null
            downscaleIfNeeded(frame, targetEdge)
        } catch (oom: OutOfMemoryError) {
            // Bitmap allocation failed: drop everything to free heap, return null
            // so the caller falls back to ExoPlayer's own (slower) cold path.
            evictAll()
            null
        } catch (e: Exception) {
            null
        } finally {
            try { retriever.release() } catch (_: Exception) {}
        }
    }

    private fun downscaleIfNeeded(bitmap: Bitmap, targetEdge: Int): Bitmap {
        val longest = max(bitmap.width, bitmap.height)
        if (longest <= targetEdge) return bitmap
        val scale = targetEdge.toFloat() / longest.toFloat()
        val scaledWidth = max(1, (bitmap.width * scale).toInt())
        val scaledHeight = max(1, (bitmap.height * scale).toInt())
        return try {
            Bitmap.createScaledBitmap(bitmap, scaledWidth, scaledHeight, true).also { scaled ->
                if (scaled !== bitmap) bitmap.recycle()
            }
        } catch (_: Throwable) {
            bitmap
        }
    }

    private fun resolveDownscaleEdge(context: Context): Int {
        val metrics = context.resources?.displayMetrics
        val screenLong = max(metrics?.widthPixels ?: 1080, metrics?.heightPixels ?: 1920)
        return screenLong.coerceIn(DOWNSCALE_MIN_EDGE_PX, DOWNSCALE_MAX_EDGE_PX)
    }

    private fun isConstrainedDevice(context: Context): Boolean {
        if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.LOLLIPOP_MR1) return true
        val mgr = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        if (mgr?.isLowRamDevice == true) return true
        val maxHeap = Runtime.getRuntime().maxMemory()
        return maxHeap in 1 until CONSTRAINED_HEAP_THRESHOLD_BYTES
    }

    private fun resolveCacheSizeBytes(constrained: Boolean): Int {
        val minB = if (constrained) CONSTRAINED_MIN_CACHE_BYTES else MIN_CACHE_BYTES
        val maxB = if (constrained) CONSTRAINED_MAX_CACHE_BYTES else MAX_CACHE_BYTES
        val divisor = if (constrained) CONSTRAINED_CACHE_DIVISOR else CACHE_DIVISOR
        val heap = Runtime.getRuntime().maxMemory().coerceAtLeast(minB.toLong())
        return (heap / divisor).toInt().coerceIn(minB, maxB)
    }

    private fun registerMemoryCallbacks(appContext: Context) {
        appContext.registerComponentCallbacks(object : ComponentCallbacks2 {
            override fun onTrimMemory(level: Int) {
                synchronized(cache) {
                    when {
                        level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ->
                            cache.evictAll()
                        level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE ->
                            cache.trimToSize(cache.maxSize() / 2)
                    }
                }
            }
            override fun onLowMemory() {
                synchronized(cache) { cache.evictAll() }
            }
            override fun onConfigurationChanged(newConfig: Configuration) {}
        })
    }
}
