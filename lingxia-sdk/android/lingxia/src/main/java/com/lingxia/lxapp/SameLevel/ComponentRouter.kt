package com.lingxia.lxapp.SameLevel

import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.ConcurrentHashMap

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
    private val managers = ConcurrentHashMap<String, WeakReference<SameLevelComponentManager>>()
    private val mainHandler = Handler(Looper.getMainLooper())

    fun register(componentId: String, manager: SameLevelComponentManager) {
        managers[componentId] = WeakReference(manager)
    }

    fun unregister(componentId: String) {
        managers.remove(componentId)
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
        mainHandler.post {
            val manager = managers[componentId]?.get() ?: return@post
            val params = parseParams(paramsJson)
            manager.dispatchCommand(componentId, name, params)
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
