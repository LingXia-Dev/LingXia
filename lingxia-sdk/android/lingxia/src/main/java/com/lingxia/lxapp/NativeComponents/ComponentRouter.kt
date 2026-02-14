package com.lingxia.lxapp.NativeComponents

import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.TextureView
import com.lingxia.lxapp.NativeComponents.Components.VideoComponent
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.ArrayDeque
import java.util.concurrent.ConcurrentHashMap

/**
 * Global router for dispatching commands from Rust FFI to native components.
 *
 * This is a lightweight registry that only maintains componentId -> manager mappings
 * for command routing. All component state (including callbacks) is managed by
 * NativeComponentManager.
 *
 * Note: This class is called from Rust via JNI. Methods with @JvmStatic are exposed
 * to native code.
 */
object ComponentRouter {
    private const val TAG = "ComponentRouter"
    private val managers = ConcurrentHashMap<String, WeakReference<NativeComponentManager>>()
    private val mainHandler = Handler(Looper.getMainLooper())
    private val streamDecoders = ConcurrentHashMap<String, StreamDecoderSession>()
    private val cachedVideoConfigJson = ConcurrentHashMap<String, String>()
    private val cachedAudioConfigJson = ConcurrentHashMap<String, String>()

    private data class PendingVideoFrame(
        val data: ByteArray,
        val dtsMs: Int,
        val ptsMs: Int,
        val keyframe: Boolean,
    )

    private data class PendingAudioFrame(
        val data: ByteArray,
        val dtsMs: Int,
        val ptsMs: Int,
    )

    private const val MAX_PENDING_VIDEO_FRAMES = 90
    private const val MAX_PENDING_AUDIO_FRAMES = 180
    private val pendingFrameLock = Any()
    private val pendingVideoFrames = ConcurrentHashMap<String, ArrayDeque<PendingVideoFrame>>()
    private val pendingAudioFrames = ConcurrentHashMap<String, ArrayDeque<PendingAudioFrame>>()

    private data class DesiredAudioState(val volume: Float, val muted: Boolean)

    private val desiredAudioStates = ConcurrentHashMap<String, DesiredAudioState>()

    private fun enqueuePendingVideoFrame(componentId: String, frame: PendingVideoFrame) {
        synchronized(pendingFrameLock) {
            val queue = pendingVideoFrames.getOrPut(componentId) { ArrayDeque() }
            if (frame.keyframe) {
                queue.clear()
            }
            queue.addLast(frame)
            while (queue.size > MAX_PENDING_VIDEO_FRAMES) {
                queue.removeFirst()
            }
        }
    }

    private fun enqueuePendingAudioFrame(componentId: String, frame: PendingAudioFrame) {
        synchronized(pendingFrameLock) {
            val queue = pendingAudioFrames.getOrPut(componentId) { ArrayDeque() }
            queue.addLast(frame)
            while (queue.size > MAX_PENDING_AUDIO_FRAMES) {
                queue.removeFirst()
            }
        }
    }

    private fun drainPendingFrames(componentId: String, session: StreamDecoderSession) {
        val videoFrames: List<PendingVideoFrame>
        val audioFrames: List<PendingAudioFrame>
        synchronized(pendingFrameLock) {
            videoFrames = pendingVideoFrames.remove(componentId)?.toList().orEmpty()
            audioFrames = pendingAudioFrames.remove(componentId)?.toList().orEmpty()
        }

        val videoConfig = cachedVideoConfigJson[componentId]
        val audioConfig = cachedAudioConfigJson[componentId]
        if (videoConfig != null) {
            session.configureVideo(videoConfig)
        }
        if (audioConfig != null) {
            session.configureAudio(audioConfig)
        }

        if (videoConfig != null) {
            for (frame in videoFrames) {
                session.pushVideo(frame.data, frame.dtsMs, frame.ptsMs, frame.keyframe)
            }
        } else if (videoFrames.isNotEmpty()) {
            synchronized(pendingFrameLock) {
                val queue = pendingVideoFrames.getOrPut(componentId) { ArrayDeque() }
                for (frame in videoFrames) {
                    queue.addLast(frame)
                }
                while (queue.size > MAX_PENDING_VIDEO_FRAMES) {
                    queue.removeFirst()
                }
            }
        }

        if (audioConfig != null) {
            for (frame in audioFrames) {
                session.pushAudio(frame.data, frame.dtsMs, frame.ptsMs)
            }
        } else if (audioFrames.isNotEmpty()) {
            synchronized(pendingFrameLock) {
                val queue = pendingAudioFrames.getOrPut(componentId) { ArrayDeque() }
                for (frame in audioFrames) {
                    queue.addLast(frame)
                }
                while (queue.size > MAX_PENDING_AUDIO_FRAMES) {
                    queue.removeFirst()
                }
            }
        }
    }

    private fun updateDesiredAudioState(componentId: String, name: String, paramsJson: String) {
        if (name != "setVolume" && name != "setMuted") return
        try {
            val obj = JSONObject(paramsJson)
            val prev = desiredAudioStates[componentId] ?: DesiredAudioState(volume = 1.0f, muted = false)
            val next = when (name) {
                "setVolume" -> {
                    val v = obj.optDouble("volume", prev.volume.toDouble()).toFloat().coerceIn(0f, 1f)
                    prev.copy(volume = v)
                }
                "setMuted" -> {
                    if (obj.has("muted") && !obj.isNull("muted")) {
                        prev.copy(muted = obj.optBoolean("muted", prev.muted))
                    } else {
                        prev
                    }
                }
                else -> prev
            }
            desiredAudioStates[componentId] = next
        } catch (_: Exception) {
        }
    }

    fun register(componentId: String, manager: NativeComponentManager) {
        managers[componentId] = WeakReference(manager)
    }

    fun unregister(componentId: String) {
        // Defensive cleanup for teardown races: ensure decoder/session and pending frames are released
        // even if callers skipped an explicit stop call.
        stopStreamDecoder(componentId)
        managers.remove(componentId)
        cachedVideoConfigJson.remove(componentId)
        cachedAudioConfigJson.remove(componentId)
        desiredAudioStates.remove(componentId)
        synchronized(pendingFrameLock) {
            pendingVideoFrames.remove(componentId)
            pendingAudioFrames.remove(componentId)
        }
    }

    @JvmStatic
    fun hasComponent(componentId: String): Boolean {
        return managers[componentId]?.get() != null
    }

    /**
     * Ask the component's manager to re-measure its DOM rect via evaluateJavascript and update
     * native overlay position (used as a fallback when JS doesn't send component.update).
     */
    internal fun requestRectSync(componentId: String): Boolean {
        val manager = managers[componentId]?.get() ?: return false
        mainHandler.post { manager.requestRectSyncFromNative(componentId) }
        return true
    }

    /**
     * Set callback for a component. Called from Rust FFI.
     * Returns true if component exists and callback was set.
     */
    @JvmStatic
    fun setVideoPlayerCallback(componentId: String, callbackId: Long): Boolean {
        val manager = managers[componentId]?.get() ?: return false
        return manager.setCallback(componentId, callbackId)
    }

    /**
     * Dispatch a command to a component. Called from Rust FFI.
     * Posts to main thread since ExoPlayer requires main thread access.
     */
    @JvmStatic
    fun dispatchVideoCommand(componentId: String, name: String, paramsJson: String) {
        val shouldUseStreamDecoder =
            name == "play" ||
                name == "pause" ||
                name == "stop" ||
                name == "resetStream" ||
                name == "rebindSurface" ||
                name == "setVolume" ||
                name == "setMuted"

        updateDesiredAudioState(componentId, name, paramsJson)

        val session = streamDecoders[componentId]
        if (shouldUseStreamDecoder && session != null && !session.isTextureViewAttached()) {
            mainHandler.post { createStreamDecoder(componentId) }
        }
        if (session != null && name != "enterFullscreen" && name != "exitFullscreen") {
            if (session.handleCommand(name, paramsJson)) {
                return
            }
        }

        mainHandler.post {
            val manager = managers[componentId]?.get() ?: return@post
            val params = parseParams(paramsJson)
            manager.dispatchCommand(componentId, name, params)
        }
    }

    /**
     * Dispatch a command specifically to the stream decoder session.
     *
     * Unlike [dispatchVideoCommand], this never forwards to the component manager/web path, to avoid
     * feedback loops when the Player's FEED engine controls the decoder pipeline.
     */
    internal fun dispatchStreamDecoderCommand(componentId: String, name: String, paramsJson: String) {
        updateDesiredAudioState(componentId, name, paramsJson)

        val session = streamDecoders[componentId]
        if (session == null) {
            createStreamDecoder(componentId)
            mainHandler.post {
                streamDecoders[componentId]?.handleCommand(name, paramsJson)
            }
            return
        }

        if (!session.isTextureViewAttached()) {
            mainHandler.post { createStreamDecoder(componentId) }
        }
        session.handleCommand(name, paramsJson)
    }

    @JvmStatic
    fun createStreamDecoder(componentId: String): Boolean {
        val manager = managers[componentId]?.get()
        if (manager == null) {
            Log.e(TAG, "createStreamDecoder: component not found: $componentId")
            return false
        }

        streamDecoders[componentId]?.let { existing ->
            if (existing.isTextureViewAttached()) {
                existing.rebindSurface()
                return true
            }
        }

        val isMainThread = Looper.myLooper() == Looper.getMainLooper()
        if (isMainThread) {
            val component = manager.getVideoComponent(componentId)
            if (component == null) {
                Log.e(TAG, "createStreamDecoder: video component not found: $componentId")
                return false
            }
            val textureView = component.acquireStreamTextureView()
            if (textureView == null) {
                Log.e(TAG, "createStreamDecoder: TextureView not available: $componentId")
                component.releaseStreamTextureView()
                return false
            }

            val existing = streamDecoders[componentId]
            if (existing != null && existing.usesTextureView(textureView)) {
                existing.rebindSurface()
                return true
            }

            streamDecoders.remove(componentId)?.let { oldDecoder ->
                oldDecoder.release()
            }

            val session = StreamDecoderSession(
                componentId = componentId,
                textureView = textureView,
                eventEmitter = { event, detail -> emitStreamEvent(componentId, event, detail) }
            )
            desiredAudioStates[componentId]?.let { session.applyDesiredAudioState(it.volume, it.muted) }
            cachedVideoConfigJson[componentId]?.let { session.configureVideo(it) }
            cachedAudioConfigJson[componentId]?.let { session.configureAudio(it) }
            streamDecoders[componentId] = session
            drainPendingFrames(componentId, session)
            return true
        }

        // Avoid blocking a non-main thread while waiting for UI resources. Stream frames/configs
        // can arrive on Rust/JNI threads during stream switching; blocking here can cascade into
        // ANRs if the main thread is busy. Schedule creation on main thread and return.
        mainHandler.post { createStreamDecoder(componentId) }
        return true
    }

    @JvmStatic
    fun configureStreamVideo(componentId: String, configJson: String): Boolean {
        cachedVideoConfigJson[componentId] = configJson
        val ok = createStreamDecoder(componentId)
        if (!ok) {
            Log.w(TAG, "configureStreamVideo: create failed: $componentId")
            return false
        }
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "configureStreamVideo: decoder not found: $componentId")
            // Decoder creation is async on the main thread; accept config and apply once ready.
            return true
        }
        val configured = decoder.configureVideo(configJson)
        drainPendingFrames(componentId, decoder)
        return configured
    }

    @JvmStatic
    fun configureStreamAudio(componentId: String, configJson: String): Boolean {
        cachedAudioConfigJson[componentId] = configJson
        val ok = createStreamDecoder(componentId)
        if (!ok) {
            Log.w(TAG, "configureStreamAudio: create failed: $componentId")
            return false
        }
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "configureStreamAudio: decoder not found: $componentId")
            // Decoder creation is async on the main thread; accept config and apply once ready.
            return true
        }
        val configured = decoder.configureAudio(configJson)
        drainPendingFrames(componentId, decoder)
        return configured
    }

    @JvmStatic
    fun pushStreamVideo(
        componentId: String,
        data: ByteArray,
        dtsMs: Int,
        ptsMs: Int,
        keyframe: Boolean
    ): Boolean {
        val decoder = streamDecoders[componentId] ?: run {
            val ok = createStreamDecoder(componentId)
            if (!ok) {
                Log.w(TAG, "pushStreamVideo: create failed: $componentId")
                return false
            }
            enqueuePendingVideoFrame(componentId, PendingVideoFrame(data, dtsMs, ptsMs, keyframe))
            return true
        }

        val videoConfigJson = cachedVideoConfigJson[componentId]
        if (videoConfigJson == null) {
            enqueuePendingVideoFrame(componentId, PendingVideoFrame(data, dtsMs, ptsMs, keyframe))
            return true
        }
        decoder.configureVideo(videoConfigJson)
        cachedAudioConfigJson[componentId]?.let { decoder.configureAudio(it) }

        return decoder.pushVideo(data, dtsMs, ptsMs, keyframe)
    }

    @JvmStatic
    fun pushStreamAudio(
        componentId: String,
        data: ByteArray,
        dtsMs: Int,
        ptsMs: Int
    ): Boolean {
        val decoder = streamDecoders[componentId] ?: run {
            val ok = createStreamDecoder(componentId)
            if (!ok) {
                Log.w(TAG, "pushStreamAudio: create failed: $componentId")
                return false
            }
            enqueuePendingAudioFrame(componentId, PendingAudioFrame(data, dtsMs, ptsMs))
            return true
        }

        val audioConfigJson = cachedAudioConfigJson[componentId]
        if (audioConfigJson == null) {
            enqueuePendingAudioFrame(componentId, PendingAudioFrame(data, dtsMs, ptsMs))
            return true
        }
        decoder.configureAudio(audioConfigJson)
        cachedVideoConfigJson[componentId]?.let { decoder.configureVideo(it) }

        return decoder.pushAudio(data, dtsMs, ptsMs)
    }

    @JvmStatic
    fun stopStreamDecoder(componentId: String): Boolean {
        val decoder = streamDecoders.remove(componentId)
        if (decoder != null) {
            decoder.release()
            mainHandler.post {
                managers[componentId]?.get()?.getVideoComponent(componentId)?.releaseStreamTextureView()
            }
        }
        synchronized(pendingFrameLock) {
            pendingVideoFrames.remove(componentId)
            pendingAudioFrames.remove(componentId)
        }
        return true
    }

    internal fun streamPlaybackPositionSeconds(componentId: String): Double? {
        return streamDecoders[componentId]?.playbackPositionSeconds()
    }

    private fun emitStreamEvent(componentId: String, event: String, detail: Map<String, Any?>) {
        mainHandler.post {
            val manager = managers[componentId]?.get() ?: return@post
            manager.deliverStreamDecoderEvent(componentId, event, detail)
        }
    }

    private fun parseParams(json: String): Map<String, Any?>? {
        if (json.isEmpty() || json == "{}") return null
        return try {
            val jsonObj = JSONObject(json)
            val map = mutableMapOf<String, Any?>()
            val keys = jsonObj.keys()
            while (keys.hasNext()) {
                val key = keys.next()
                val value = jsonObj.get(key)
                if (value != JSONObject.NULL) {
                    map[key] = value
                }
            }
            map.ifEmpty { null }
        } catch (e: Exception) {
            null
        }
    }
}
