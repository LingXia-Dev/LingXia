package com.lingxia.lxapp.SameLevel

import android.os.Handler
import android.os.Looper
import android.util.Log
import com.lingxia.lxapp.NativeApi
import java.lang.ref.WeakReference
import java.util.concurrent.ConcurrentHashMap
import org.json.JSONObject

object VideoPlayerRegistry {
    private const val TAG = "VideoPlayerRegistry"
    private val managers = ConcurrentHashMap<String, WeakReference<SameLevelComponentManager>>()
    private val callbackIds = ConcurrentHashMap<String, Long>()
    private val mainHandler = Handler(Looper.getMainLooper())
    
    // Queue commands that arrive before the component is mounted
    private data class PendingCommand(val name: String, val paramsJson: String)
    private val pendingCommands = ConcurrentHashMap<String, MutableList<PendingCommand>>()

    fun registerComponent(componentId: String, manager: SameLevelComponentManager) {
        managers[componentId] = WeakReference(manager)
        
        // Dispatch any queued commands now that the manager is available
        val queued = pendingCommands.remove(componentId)?.toList().orEmpty()
        queued.forEach { cmd ->
            Log.d(TAG, "Dispatching queued command ${cmd.name} for component $componentId")
            dispatchCommandInternal(componentId, manager, cmd.name, cmd.paramsJson)
        }
    }

    fun unregisterComponent(componentId: String) {
        managers.remove(componentId)
        pendingCommands.remove(componentId)
        // Note: We don't remove callbackIds here because unregisterComponent is called when
        // the component unmounts from the view hierarchy (WebView side), but the Rust side
        // player controller might still exist and want to receive events or commands until
        // it is explicitly destroyed via unregisterVideoPlayer.
        // However, if the component is gone, commands will fail anyway.
    }

    fun registerCallback(componentId: String, callbackId: Long) {
        callbackIds[componentId] = callbackId
    }

    fun unregisterCallback(componentId: String) {
        callbackIds.remove(componentId)
        pendingCommands.remove(componentId)
    }

    fun dispatchCommand(componentId: String, name: String, paramsJson: String) {
        // Post to main thread since ExoPlayer requires main thread access
        mainHandler.post {
            dispatchCommandOnMainThread(componentId, name, paramsJson)
        }
    }
    
    private fun dispatchCommandOnMainThread(componentId: String, name: String, paramsJson: String) {
        val managerRef = managers[componentId]
        val manager = managerRef?.get()
        
        if (manager == null) {
            // Queue the command for when the component is mounted
            Log.d(TAG, "Queueing command $name for component $componentId (not yet mounted)")
            pendingCommands.compute(componentId) { _, existing ->
                val queue = existing ?: mutableListOf()
                queue.add(PendingCommand(name, paramsJson))
                queue
            }
            return
        }
        
        dispatchCommandInternal(componentId, manager, name, paramsJson)
    }
    
    private fun dispatchCommandInternal(
        componentId: String,
        manager: SameLevelComponentManager,
        name: String,
        paramsJson: String
    ) {
        val params = parseParams(paramsJson)
        
        val commandMap = mutableMapOf<String, Any?>()
        commandMap["id"] = componentId
        commandMap["name"] = name
        commandMap["params"] = params
        commandMap["action"] = "component.command"
        
        manager.handle(commandMap)
    }
    
    fun emitEventIfNeeded(componentId: String, payload: Map<String, Any>) {
        val callbackId = callbackIds[componentId] ?: return
        
        val enriched = payload.toMutableMap()
        enriched["componentId"] = componentId
        
        val jsonString = JSONObject(enriched as Map<*, *>).toString()
        NativeApi.onCallback(callbackId, true, jsonString)
    }

    private fun parseParams(json: String): Map<String, Any?> {
        if (json.isEmpty() || json == "{}") return emptyMap()
        try {
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
            return map
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse params JSON", e)
            return emptyMap()
        }
    }
}
