package com.lingxia.lxapp.SameLevel

import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.TextureView
import com.lingxia.lxapp.SameLevel.Components.VideoComponent
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Global router for dispatching commands from Rust FFI to native components.
 *
 * This is a lightweight registry that only maintains componentId -> manager mappings
 * for command routing. All component state (including callbacks) is managed by
 * SameLevelComponentManager.
 *
 * Note: This class is called from Rust via JNI. Methods with @JvmStatic are exposed
 * to native code.
 */
object ComponentRouter {
    private const val TAG = "ComponentRouter"
    private val managers = ConcurrentHashMap<String, WeakReference<SameLevelComponentManager>>()
    private val mainHandler = Handler(Looper.getMainLooper())
    private val streamDecoders = ConcurrentHashMap<String, StreamDecoderSession>()
    private val streamDecoderLock = Any()

    private data class DesiredAudioState(val volume: Float, val muted: Boolean)

    private val desiredAudioStates = ConcurrentHashMap<String, DesiredAudioState>()

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

    private fun ensureStreamDecoderExists(componentId: String, reason: String): StreamDecoderSession? {
        streamDecoders[componentId]?.let { return it }
        synchronized(streamDecoderLock) {
            streamDecoders[componentId]?.let { return it }
            val ok = createStreamDecoder(componentId)
            if (!ok) {
                Log.w(TAG, "ensureStreamDecoderExists($reason): create failed: $componentId")
                return null
            }
            return streamDecoders[componentId]
        }
    }

    fun register(componentId: String, manager: SameLevelComponentManager) {
        managers[componentId] = WeakReference(manager)
    }

    fun unregister(componentId: String) {
        managers.remove(componentId)
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

        var session = streamDecoders[componentId]
        if (shouldUseStreamDecoder) {
            if (session == null) {
                session = ensureStreamDecoderExists(componentId, "dispatchVideoCommand:$name")
            } else if (!session.isTextureViewAttached()) {
                mainHandler.post { createStreamDecoder(componentId) }
            }
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
            streamDecoders[componentId] = session
            return true
        }

        val latch = CountDownLatch(1)
        var textureView: TextureView? = null
        var component: VideoComponent? = null
        mainHandler.post {
            component = manager.getVideoComponent(componentId)
            if (component == null) {
                Log.e(TAG, "createStreamDecoder: video component not found: $componentId")
                latch.countDown()
                return@post
            }
            textureView = component?.acquireStreamTextureView()
            latch.countDown()
        }

        val ready = try {
            latch.await(5000, TimeUnit.MILLISECONDS)
        } catch (e: InterruptedException) {
            Log.e(TAG, "createStreamDecoder: interrupted while waiting: $componentId", e)
            mainHandler.post { component?.releaseStreamTextureView() }
            return false
        }

        if (!ready) {
            Log.e(TAG, "createStreamDecoder: timeout waiting for TextureView: $componentId")
            mainHandler.post { component?.releaseStreamTextureView() }
            return false
        }

        if (textureView == null) {
            Log.e(TAG, "createStreamDecoder: TextureView not available: $componentId")
            mainHandler.post { component?.releaseStreamTextureView() }
            return false
        }

        val existing = streamDecoders[componentId]
        if (existing != null && existing.usesTextureView(textureView!!)) {
            existing.rebindSurface()
            return true
        }

        streamDecoders.remove(componentId)?.let { oldDecoder ->
            oldDecoder.release()
        }

        val session = StreamDecoderSession(
            componentId = componentId,
            textureView = textureView!!,
            eventEmitter = { event, detail -> emitStreamEvent(componentId, event, detail) }
        )
        desiredAudioStates[componentId]?.let { session.applyDesiredAudioState(it.volume, it.muted) }
        streamDecoders[componentId] = session
        return true
    }

    @JvmStatic
    fun configureStreamVideo(componentId: String, configJson: String): Boolean {
        createStreamDecoder(componentId)
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "configureStreamVideo: decoder not found: $componentId")
            return false
        }
        return decoder.configureVideo(configJson)
    }

    @JvmStatic
    fun configureStreamAudio(componentId: String, configJson: String): Boolean {
        createStreamDecoder(componentId)
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "configureStreamAudio: decoder not found: $componentId")
            return false
        }
        return decoder.configureAudio(configJson)
    }

    @JvmStatic
    fun pushStreamVideo(
        componentId: String,
        data: ByteArray,
        dtsMs: Int,
        ptsMs: Int,
        keyframe: Boolean
    ): Boolean {
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "pushStreamVideo: decoder not found: $componentId")
            return false
        }
        return decoder.pushVideo(data, dtsMs, ptsMs, keyframe)
    }

    @JvmStatic
    fun pushStreamAudio(
        componentId: String,
        data: ByteArray,
        dtsMs: Int,
        ptsMs: Int
    ): Boolean {
        val decoder = streamDecoders[componentId]
        if (decoder == null) {
            Log.w(TAG, "pushStreamAudio: decoder not found: $componentId")
            return false
        }
        return decoder.pushAudio(data, dtsMs, ptsMs)
    }

    @JvmStatic
    fun stopStreamDecoder(componentId: String): Boolean {
        val decoder = streamDecoders.remove(componentId) ?: return false
        decoder.release()
        mainHandler.post {
            managers[componentId]?.get()?.getVideoComponent(componentId)?.releaseStreamTextureView()
        }
        return true
    }

    private fun emitStreamEvent(componentId: String, event: String, detail: Map<String, Any?>) {
        mainHandler.post {
            val manager = managers[componentId]?.get() ?: return@post
            manager.emitComponentEvent(componentId, event, detail)
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
