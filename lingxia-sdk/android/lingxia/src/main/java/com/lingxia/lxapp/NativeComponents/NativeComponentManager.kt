package com.lingxia.lxapp.NativeComponents

import android.graphics.Outline
import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.lingxia.lxapp.NativeApi
import com.lingxia.webview.LingXiaWebView
import com.lingxia.lxapp.NativeComponents.Components.VideoComponent
import android.os.Handler
import android.os.Looper
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Manages the lifecycle of native components rendered in overlay.
 *
 * For Android's overlay-based rendering, this manager tracks component
 * positions in WebView content coordinates and updates screen positions when
 * the WebView scrolls, eliminating JS polling latency.
 */
class NativeComponentManager(
    hostView: ViewGroup,
    private val defaultPageId: String,
    private val eventSink: (Map<String, Any>) -> Unit,
    webView: LingXiaWebView? = null
) {
    private companion object {
        private const val INACTIVE_PAGE_STOP_DELAY_MS = 60_000L
        private const val MAX_PENDING_NATIVE_EVENTS_PER_COMPONENT = 8
    }

    private val logTag = "NativeComponentManager"
    private val hostViewRef = WeakReference(hostView)
    private val webViewRef = webView?.let { WeakReference(it) }
    private val density = hostView.context.resources.displayMetrics.density
    private val mainHandler = Handler(Looper.getMainLooper())

    private val components = mutableMapOf<String, LxNativeComponent>()
    private val componentPage = mutableMapOf<String, String>()
    private val componentType = mutableMapOf<String, String>()
    private val componentPageFuncBindings = mutableMapOf<String, Map<String, String>>()
    private val componentDataset = mutableMapOf<String, Map<String, Any?>>()
    private val componentAdjustPosition = mutableMapOf<String, Boolean>()
    private val readyComponentIds = mutableSetOf<String>()
    private val pendingEventsByComponent = mutableMapOf<String, MutableList<Map<String, Any>>>()
    // Rust callback IDs for VideoContext event forwarding
    private val componentCallbacks = mutableMapOf<String, Long>()
    // Content position (document coordinates = viewport + scroll at mount time)
    // This is the stable reference position in the WebView content
    private val componentContentRects = mutableMapOf<String, RectF>()
    // Pre-allocated screen rects to avoid GC during scroll
    private val componentScreenRects = mutableMapOf<String, RectF>()
    private val pageComponents = mutableMapOf<String, MutableSet<String>>()
    // Monotonic generation per component id. Used to drop stale async events from old instances.
    private val componentEpochs = mutableMapOf<String, Long>()
    private val factories = mutableMapOf<String, LxNativeComponentFactory>()

    private val webOverlayCoverageRestore: MutableMap<String, Int> = mutableMapOf()

    // When DOM is transitioning (e.g. switching live <-> playback), measureById can temporarily
    // return 0-size. Keep retrying a few times so the native overlay catches the final layout.
    private val rectSyncRetries = mutableMapOf<String, Int>()
    private val rectSyncRetryRunnables = mutableMapOf<String, Runnable>()
    private val focusVisibilityRunnables = mutableMapOf<String, MutableList<Runnable>>()
    private val pageInactiveStopRunnables = mutableMapOf<String, Runnable>()
    private val componentPlaybackIntent = mutableMapOf<String, Boolean>()
    private val componentsPendingAutoResume = mutableSetOf<String>()

    fun register(type: String, factory: LxNativeComponentFactory) {
        factories[type] = factory
    }

    fun handle(message: Map<String, Any?>) {
        when (message["action"] as? String) {
            "component.mount" -> handleMount(message)
            "component.update" -> handleUpdate(message)
            "component.unmount" -> handleUnmount(message)
            "component.ready" -> handleReady(message)
            "component.focus" -> handleFocus(message)
            "component.blur" -> handleBlur(message)
            "component.command" -> handleCommand(message)
            "component.coverage" -> handleCoverage(message)
            "page.lifecycle" -> handlePageLifecycle(message)
        }
    }

    private fun handleReady(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        readyComponentIds.add(id)
        val pending = pendingEventsByComponent.remove(id) ?: return
        pending.forEach { eventSink(it) }
    }

    private fun handleCoverage(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val covered = params["covered"] as? Boolean ?: return
        setWebOverlayCoverage(componentId = id, covered = covered)
    }

    private fun setWebOverlayCoverage(componentId: String, covered: Boolean) {
        val component = components[componentId] as? VideoComponent ?: return
        val view = component.view

        if (covered) {
            if (view.visibility == View.INVISIBLE && webOverlayCoverageRestore.containsKey(componentId)) {
                return
            }
            webOverlayCoverageRestore.putIfAbsent(componentId, view.visibility)
            view.visibility = View.INVISIBLE
            return
        }

        val restore = webOverlayCoverageRestore.remove(componentId) ?: return
        view.visibility = restore
    }

    private fun handleMount(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val type = params["type"] as? String ?: return
        val rectDict = params["rect"] as? Map<*, *> ?: return
        if (components.containsKey(id)) return

        val factory = factories[type] ?: return
        val host = hostViewRef.get() ?: return

        val pageId = resolvePageId(params)
        val props = params["props"].asStringMap()
        val zIndex = (params["zIndex"] as? Number)?.toFloat() ?: 0f
        val cornerRadius = (params["cornerRadius"] as? Number)?.toFloat() ?: 0f
        // JS sends document coordinates (content position = viewport + scroll)
        // Store directly as content rect for scroll tracking
        val contentRect = pixelAligned(rectFrom(rectDict))
        
        // Calculate initial screen position
        val screenRect = contentRectToScreenRect(contentRect)

        val epoch = (componentEpochs[id] ?: 0L) + 1L
        componentEpochs[id] = epoch
        val component = factory.make(id, props) { event ->
            // Guard against stale events from a previously unmounted instance that shared the same id.
            if (componentEpochs[id] != epoch) return@make
            if (!components.containsKey(id)) return@make
            dispatchComponentEvent(id, event)
        }

        components[id] = component
        componentPage[id] = pageId
        componentType[id] = type
        parsePageFuncBindings(props)?.let { componentPageFuncBindings[id] = it }
        parseDataset(props)?.let { componentDataset[id] = it }
        componentAdjustPosition[id] = parseAdjustPosition(props)
        if (type == "input.native") {
            Log.d(
                logTag,
                "mount input id=$id type=${props["type"]} password=${props["password"]} adjustPosition=${componentAdjustPosition[id]}"
            )
        }
        componentContentRects[id] = contentRect
        componentScreenRects[id] = screenRect
        pageComponents.getOrPut(pageId) { mutableSetOf() }.add(id)
        ComponentRouter.register(id, this)

        component.mount(host)
        component.setFrame(screenRect)
        component.update(props)
        applyCornerRadius(component.view, cornerRadius)
        component.view.translationZ = zIndex
    }

    private fun handleUpdate(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val component = components[id] ?: return

        (params["rect"] as? Map<*, *>)?.let { rectDict ->
            // JS sends document coordinates (content position)
            val newContentRect = pixelAligned(rectFrom(rectDict))
            // During DOM transitions (e.g. switching live <-> playback), React can transiently
            // report 0-size rects. Applying them would make the native overlay disappear and
            // it may never recover if no further update is emitted.
            if (newContentRect.width() <= 1f || newContentRect.height() <= 1f) {
                scheduleRectSyncRetry(id, "ignored 0-size update rect: $newContentRect")
                return@let
            }
            
            if (shouldUpdateFrame(componentContentRects[id], newContentRect)) {
                componentContentRects[id] = newContentRect
                val screenRect = componentScreenRects.getOrPut(id) { RectF() }
                updateScreenRect(screenRect, newContentRect)
                component.setFrame(screenRect)
            }
        }

        params["props"].asStringMap().takeIf { it.isNotEmpty() }?.let { props ->
            if ("pageFuncBindings" in props) {
                val parsed = parsePageFuncBindings(props)
                if (parsed.isNullOrEmpty()) {
                    componentPageFuncBindings.remove(id)
                } else {
                    componentPageFuncBindings[id] = parsed
                }
            }
            if ("dataset" in props) {
                val parsed = parseDataset(props)
                if (parsed.isNullOrEmpty()) {
                    componentDataset.remove(id)
                } else {
                    componentDataset[id] = parsed
                }
            }
            if ("adjustPosition" in props || "adjust-position" in props) {
                componentAdjustPosition[id] = parseAdjustPosition(props)
            }
            if (componentType[id] == "input.native" && ("type" in props || "password" in props)) {
                Log.d(
                    logTag,
                    "update input id=$id type=${props["type"]} password=${props["password"]} adjustPosition=${componentAdjustPosition[id]}"
                )
            }
            component.update(props)
        }
        (params["zIndex"] as? Number)?.let { component.view.translationZ = it.toFloat() }
        (params["cornerRadius"] as? Number)?.toFloat()?.let { radius ->
            applyCornerRadius(component.view, radius)
            component.update(mapOf("cornerRadius" to radius))
        }
    }

    private fun handleUnmount(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val pageId = resolvePageId(params)
        unmountComponent(id, pageId)
    }

    private fun handleFocus(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        components[id]?.focus()
    }

    private fun handleBlur(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        focusVisibilityRunnables.remove(id)?.forEach { mainHandler.removeCallbacks(it) }
        components[id]?.blur()
    }

    private fun handleCommand(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val name = params["name"] as? String ?: return
        when (name) {
            "play" -> componentPlaybackIntent[id] = true
            "pause", "stop" -> componentPlaybackIntent[id] = false
        }
        components[id]?.handleCommand(name, params["params"].asStringMap().ifEmpty { null })
    }

    private fun handlePageLifecycle(params: Map<String, Any?>) {
        when (params["state"] as? String) {
            "inactive" -> pausePage(resolvePageId(params))
            "active" -> resumePage(resolvePageId(params))
            "destroyed" -> unmountPage(resolvePageId(params))
        }
    }

    fun teardownAll() {
        pageInactiveStopRunnables.values.forEach { mainHandler.removeCallbacks(it) }
        pageInactiveStopRunnables.clear()
        focusVisibilityRunnables.values.flatten().forEach { mainHandler.removeCallbacks(it) }
        focusVisibilityRunnables.clear()
        componentsPendingAutoResume.clear()
        componentPlaybackIntent.clear()
        val allIds = components.keys.toList()
        allIds.forEach { id ->
            unmountComponent(id, componentPage[id])
        }
        pageComponents.clear()
    }
    
    /**
     * Called before each frame draw to sync component positions with WebView scroll.
     * Uses pre-allocated RectF objects to avoid GC pressure during scroll.
     * 
     * @param scrollX Current horizontal scroll position in pixels
     * @param scrollY Current vertical scroll position in pixels
     */
    fun onWebViewScroll(scrollX: Int, scrollY: Int) {
        val scrollXPx = scrollX.toFloat()
        val scrollYPx = scrollY.toFloat()
        
        components.forEach { (id, component) ->
            val contentRect = componentContentRects[id] ?: return@forEach
            val screenRect = componentScreenRects.getOrPut(id) { RectF() }
            screenRect.set(
                contentRect.left - scrollXPx,
                contentRect.top - scrollYPx,
                contentRect.right - scrollXPx,
                contentRect.bottom - scrollYPx
            )
            component.setFrame(screenRect)
        }
    }

    /**
     * Update a component's content rect from native code (document coordinates in physical pixels).
     * Useful as a fallback when the WebView layout shifts without a scroll event and JS doesn't
     * send a component.update (e.g. DOM reflow triggered by state updates).
     */
    internal fun updateContentRectFromNative(componentId: String, contentRectPx: RectF): Boolean {
        val component = components[componentId] ?: return false
        val aligned = pixelAligned(contentRectPx)
        if (aligned.width() <= 1f || aligned.height() <= 1f) {
            scheduleRectSyncRetry(componentId, "ignored 0-size native rect: $aligned")
            return false
        }
        componentContentRects[componentId] = aligned
        val screenRect = componentScreenRects.getOrPut(componentId) { RectF() }
        updateScreenRect(screenRect, aligned)
        component.setFrame(screenRect)
        return true
    }

    internal fun requestRectSyncFromNative(componentId: String) {
        val webView = webViewRef?.get() ?: return

        val escapedId = componentId
            .replace("\\", "\\\\")
            .replace("'", "\\'")

        val script = """
            (function(){
              try {
                return window.LingXiaBridge.dom.measureById('$escapedId');
              } catch (e) { return null; }
            })()
        """.trimIndent()

        webView.evaluateJavascript(script) { value ->
            try {
                if (value == null || value == "null" || value == "\"null\"") return@evaluateJavascript
                val v = value.trim()
                if (!v.startsWith("[") || !v.endsWith("]")) return@evaluateJavascript
                val parts = v.substring(1, v.length - 1).split(',')
                if (parts.size < 4) return@evaluateJavascript
                val xCss = parts[0].trim().toDouble()
                val yCss = parts[1].trim().toDouble()
                val wCss = parts[2].trim().toDouble()
                val hCss = parts[3].trim().toDouble()
                if (wCss <= 0.0 || hCss <= 0.0) {
                    scheduleRectSyncRetry(componentId, "zero-size rect from JS: $v")
                    return@evaluateJavascript
                }

                val rectPx = RectF(
                    (xCss * density).toFloat(),
                    (yCss * density).toFloat(),
                    ((xCss + wCss) * density).toFloat(),
                    ((yCss + hCss) * density).toFloat()
                )
                rectSyncRetries.remove(componentId)
                rectSyncRetryRunnables.remove(componentId)?.let { mainHandler.removeCallbacks(it) }
                updateContentRectFromNative(componentId, rectPx)
            } catch (e: Exception) {
                scheduleRectSyncRetry(componentId, "parse error: ${e.message}")
            }
        }
    }

    private fun scheduleRectSyncRetry(componentId: String, reason: String) {
        val attempts = (rectSyncRetries[componentId] ?: 0) + 1
        rectSyncRetries[componentId] = attempts
        if (attempts > 10) {
            clearRectSyncRetry(componentId)
            Log.w(logTag, "requestRectSyncFromNative retry exhausted for $componentId ($reason)")
            return
        }

        rectSyncRetryRunnables[componentId]?.let { mainHandler.removeCallbacks(it) }
        val task = Runnable { requestRectSyncFromNative(componentId) }
        rectSyncRetryRunnables[componentId] = task
        mainHandler.postDelayed(task, 120L)
    }

    private fun clearRectSyncRetry(componentId: String) {
        rectSyncRetries.remove(componentId)
        rectSyncRetryRunnables.remove(componentId)?.let { mainHandler.removeCallbacks(it) }
    }

    /**
     * Set Rust callback ID for a component (used by VideoContext).
     * Returns true once stored (component may not exist yet).
     */
    fun setCallback(componentId: String, callbackId: Long): Boolean {
        componentCallbacks[componentId] = callbackId
        return true
    }

    /**
     * Dispatch a command to a component from Rust FFI.
     * Returns true if component exists and command was dispatched.
     */
    fun dispatchCommand(componentId: String, name: String, params: Map<String, Any?>?): Boolean {
        val component = components[componentId] ?: return false
        component.handleCommand(name, params)
        return true
    }

    internal fun getVideoComponent(componentId: String): VideoComponent? {
        return components[componentId] as? VideoComponent
    }

    internal fun deliverStreamDecoderEvent(
        componentId: String,
        event: String,
        detail: Map<String, Any?> = emptyMap()
    ) {
        val videoComponent = components[componentId] as? VideoComponent ?: return
        videoComponent.handleStreamDecoderEvent(event, detail)
    }

    internal fun emitComponentEvent(
        componentId: String,
        event: String,
        detail: Map<String, Any?> = emptyMap()
    ) {
        dispatchComponentEvent(componentId, mapOf("event" to event, "detail" to detail))
    }

    private fun dispatchComponentEvent(componentId: String, event: Map<String, Any>) {
        val payload = event.toMutableMap()
        payload["action"] = "component.event"
        payload["id"] = componentId
        payload["componentId"] = componentId
        componentPage[componentId]?.let { payload["pageId"] = it }
        val eventName = payload["event"] as? String
        if (eventName == "focus") {
            scheduleEnsureFocusedComponentVisible(componentId)
        }
        updatePlaybackIntent(componentId, eventName)
        emitEventToView(componentId, payload)
        dispatchPageFunc(componentId, payload)
        
        // Also forward to Rust callback if registered (for VideoContext).
        // Keep this list small to avoid high-frequency callbacks (e.g. timeupdate).
        val shouldForwardToCallback = when (eventName) {
            "waiting",
            "playrequest",
            "playing",
            "pause",
            "stop",
            "ended",
            "error",
            "seeked",
            "seeking" -> true
            else -> false
        }
        if (shouldForwardToCallback) {
            componentCallbacks[componentId]?.let { callbackId ->
                payload["componentId"] = componentId
                NativeApi.onCallback(callbackId, true, JSONObject(payload as Map<*, *>).toString())
            }
        }
    }

    private fun emitEventToView(componentId: String, payload: MutableMap<String, Any>) {
        if (readyComponentIds.contains(componentId)) {
            eventSink(payload)
            return
        }
        val queue = pendingEventsByComponent.getOrPut(componentId) { mutableListOf() }
        queue.add(payload.toMap())
        if (queue.size > MAX_PENDING_NATIVE_EVENTS_PER_COMPONENT) {
            queue.subList(0, queue.size - MAX_PENDING_NATIVE_EVENTS_PER_COMPONENT).clear()
        }
    }

    private fun parsePageFuncBindings(props: Map<String, Any?>): Map<String, String>? {
        val parsed = mutableMapOf<String, String>()
        val rawObject = props["pageFuncBindings"] as? Map<*, *>
        if (rawObject != null) {
            rawObject.forEach { (k, v) ->
                val key = (k as? String)?.trim()?.lowercase().orEmpty()
                val value = (v as? String)?.trim().orEmpty()
                if (key.isNotEmpty() && value.isNotEmpty()) {
                    parsed[key] = value
                }
            }
        }
        val rawJson = props["pageFuncBindingsJson"] as? String
        if (!rawJson.isNullOrBlank()) {
            try {
                val obj = JSONObject(rawJson)
                obj.keys().forEach { key ->
                    val normalized = key.trim().lowercase()
                    val value = obj.optString(key, "").trim()
                    if (normalized.isNotEmpty() && value.isNotEmpty()) {
                        parsed[normalized] = value
                    }
                }
            } catch (_: Exception) {}
        }
        return parsed.takeIf { it.isNotEmpty() }
    }

    private fun parsePageId(pageId: String): Pair<String, String>? {
        val separator = pageId.indexOf(':')
        if (separator <= 0 || separator >= pageId.length - 1) return null
        val appId = pageId.substring(0, separator)
        val path = pageId.substring(separator + 1)
        if (appId.isEmpty() || path.isEmpty()) return null
        return appId to path
    }

    private fun parseDataset(props: Map<String, Any?>): Map<String, Any?>? {
        val parsed = mutableMapOf<String, Any?>()
        val rawObject = props["dataset"] as? Map<*, *>
        if (rawObject != null) {
            rawObject.forEach { (k, v) ->
                val key = (k as? String)?.trim().orEmpty()
                if (key.isNotEmpty()) {
                    parsed[key] = v
                }
            }
        }
        val rawJson = props["datasetJson"] as? String
        if (!rawJson.isNullOrBlank()) {
            try {
                val obj = JSONObject(rawJson)
                obj.keys().forEach { key ->
                    val normalized = key.trim()
                    if (normalized.isNotEmpty()) {
                        parsed[normalized] = jsonToAny(obj.opt(key))
                    }
                }
            } catch (_: Exception) {}
        }
        return parsed.takeIf { it.isNotEmpty() }
    }

    private fun parseAdjustPosition(props: Map<String, Any?>): Boolean {
        if (props.containsKey("adjustPosition")) {
            return readBooleanProp(props["adjustPosition"], true)
        }
        if (props.containsKey("adjust-position")) {
            return readBooleanProp(props["adjust-position"], true)
        }
        return true
    }

    private fun readBooleanProp(raw: Any?, default: Boolean): Boolean {
        if (raw == null) return default
        return when (raw) {
            is Boolean -> raw
            is Number -> raw.toInt() != 0
            is String -> raw.equals("true", ignoreCase = true) || raw == "1"
            else -> default
        }
    }

    private fun scheduleEnsureFocusedComponentVisible(componentId: String) {
        if (componentAdjustPosition[componentId] == false) {
            return
        }
        // Cancel ALL pending scroll-into-view runnables, not just those for this component.
        // If the user moves focus from component A to B before A's runnables fire, A's
        // runnables would otherwise scroll the WebView unexpectedly and scramble positions.
        focusVisibilityRunnables.keys.toList().forEach { id ->
            focusVisibilityRunnables.remove(id)?.forEach { mainHandler.removeCallbacks(it) }
        }
        // JS already handles keyboard avoidance for input/textarea via ensureVisibleForKeyboard.
        // Adding a native scroll on top causes double-scrolling and position chaos.
        val type = componentType[componentId]
        if (type == "input.native" || type == "textarea.native") return

        val tasks = mutableListOf<Runnable>()
        val delays = longArrayOf(120L, 260L)
        delays.forEach { delay ->
            val task = Runnable {
                ensureComponentVisibleForIme(componentId)
            }
            tasks.add(task)
            mainHandler.postDelayed(task, delay)
        }
        focusVisibilityRunnables[componentId] = tasks
    }

    private fun ensureComponentVisibleForIme(componentId: String) {
        if (componentAdjustPosition[componentId] == false) return
        val webView = webViewRef?.get() ?: return
        val host = hostViewRef.get() ?: return
        val contentRect = componentContentRects[componentId] ?: return

        val rootInsets = ViewCompat.getRootWindowInsets(host)
        val imeBottom = rootInsets?.getInsets(WindowInsetsCompat.Type.ime())?.bottom ?: 0
        val navBottom = rootInsets?.getInsets(WindowInsetsCompat.Type.navigationBars())?.bottom ?: 0
        val keyboardHeight = (imeBottom - navBottom).coerceAtLeast(0)
        val viewportHeight = webView.height.toFloat()
        if (viewportHeight <= 0f) return

        val scrollY = webView.scrollY.toFloat()
        val screenTop = contentRect.top - scrollY
        val screenBottom = contentRect.bottom - scrollY
        val topSafe = 12f * density
        val bottomSafe = 24f * density
        val visibleBottom = (viewportHeight - keyboardHeight - bottomSafe).coerceAtLeast(topSafe)

        var delta = 0f
        if (screenBottom > visibleBottom) {
            delta = screenBottom - visibleBottom
        } else if (screenTop < topSafe) {
            delta = screenTop - topSafe
        }
        if (abs(delta) < 1f) return

        val targetY = (webView.scrollY + delta.roundToInt()).coerceAtLeast(0)
        if (targetY == webView.scrollY) return
        webView.scrollTo(webView.scrollX, targetY)
        onWebViewScroll(webView.scrollX, targetY)
    }

    private fun jsonToAny(value: Any?): Any? {
        return when (value) {
            null, JSONObject.NULL -> null
            is JSONObject -> {
                val out = mutableMapOf<String, Any?>()
                value.keys().forEach { key ->
                    out[key] = jsonToAny(value.opt(key))
                }
                out
            }
            is JSONArray -> {
                val out = mutableListOf<Any?>()
                for (i in 0 until value.length()) {
                    out.add(jsonToAny(value.opt(i)))
                }
                out
            }
            else -> value
        }
    }

    private fun buildPageEvent(componentId: String, eventName: String, payload: Map<String, Any>): Map<String, Any?> {
        val detail = payload["detail"] ?: emptyMap<String, Any?>()
        val dataset = componentDataset[componentId] ?: emptyMap<String, Any?>()
        val target = mapOf(
            "id" to componentId,
            "dataset" to dataset
        )
        return mapOf(
            "type" to eventName,
            "detail" to detail,
            "id" to componentId,
            "dataset" to dataset,
            "target" to target,
            "currentTarget" to target,
            "timeStamp" to System.currentTimeMillis()
        )
    }

    private fun dispatchPageFunc(componentId: String, payload: MutableMap<String, Any>) {
        val eventName = (payload["event"] as? String)?.trim()?.lowercase() ?: return
        val bindings = componentPageFuncBindings[componentId]
        if (bindings.isNullOrEmpty()) {
            return
        }

        val pageId = componentPage[componentId]
        if (pageId.isNullOrEmpty()) {
            return
        }
        val route = parsePageId(pageId)
        if (route == null) {
            return
        }

        val pageEventJson = try {
            val pageEvent = buildPageEvent(componentId, eventName, payload)
            JSONObject(pageEvent as Map<*, *>).toString()
        } catch (e: Exception) {
            Log.w(logTag, "nativecomponent payload encode failed componentId=$componentId event=$eventName", e)
            return
        }
        val bindingsJson = try {
            JSONObject(bindings as Map<*, *>).toString()
        } catch (e: Exception) {
            Log.w(logTag, "nativecomponent bindings encode failed componentId=$componentId event=$eventName", e)
            return
        }
        NativeApi.dispatchNativeComponentEvent(
            route.first,
            route.second,
            componentId,
            eventName,
            pageEventJson,
            bindingsJson
        )
    }

    private fun updatePlaybackIntent(componentId: String, eventName: String?) {
        when (eventName) {
            "play", "playrequest", "playing" -> componentPlaybackIntent[componentId] = true
            "pause", "stop", "ended", "error" -> componentPlaybackIntent[componentId] = false
        }
    }

    private fun resolvePageId(dict: Map<String, Any?>): String {
        val pageId = dict["pageId"] as? String
        return if (!pageId.isNullOrEmpty()) pageId else defaultPageId
    }

    private fun Any?.asStringMap(): Map<String, Any?> {
        val raw = this as? Map<*, *> ?: return emptyMap()
        val filtered = mutableMapOf<String, Any?>()
        raw.forEach { (k, v) ->
            if (k is String) {
                filtered[k] = v
            }
        }
        return filtered
    }

    private fun rectFrom(dict: Map<*, *>): RectF {
        val x = ((dict["x"] as? Number)?.toFloat() ?: 0f) * density
        val y = ((dict["y"] as? Number)?.toFloat() ?: 0f) * density
        val w = ((dict["width"] as? Number)?.toFloat() ?: 0f) * density
        val h = ((dict["height"] as? Number)?.toFloat() ?: 0f) * density
        return RectF(x, y, x + w, y + h)
    }

    private fun pixelAligned(rect: RectF): RectF = RectF(
        rect.left.roundToInt().toFloat(),
        rect.top.roundToInt().toFloat(),
        rect.right.roundToInt().toFloat(),
        rect.bottom.roundToInt().toFloat()
    )

    private fun shouldUpdateFrame(old: RectF?, new: RectF): Boolean {
        if (old == null) return true
        return abs(old.left - new.left) > 0.5f || abs(old.top - new.top) > 0.5f ||
               abs(old.right - new.right) > 0.5f || abs(old.bottom - new.bottom) > 0.5f
    }

    private fun contentRectToScreenRect(contentRect: RectF): RectF {
        val webView = webViewRef?.get()
        val scrollX = (webView?.scrollX ?: 0).toFloat()
        val scrollY = (webView?.scrollY ?: 0).toFloat()
        return RectF(
            contentRect.left - scrollX,
            contentRect.top - scrollY,
            contentRect.right - scrollX,
            contentRect.bottom - scrollY
        )
    }

    private fun updateScreenRect(screenRect: RectF, contentRect: RectF) {
        val webView = webViewRef?.get()
        val scrollX = (webView?.scrollX ?: 0).toFloat()
        val scrollY = (webView?.scrollY ?: 0).toFloat()
        screenRect.set(
            contentRect.left - scrollX,
            contentRect.top - scrollY,
            contentRect.right - scrollX,
            contentRect.bottom - scrollY
        )
    }

    private fun applyCornerRadius(view: View, radius: Float) {
        if (radius > 0) {
            view.clipToOutline = true
            view.outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(v: View, outline: Outline) {
                    outline.setRoundRect(0, 0, v.width, v.height, radius * density)
                }
            }
        } else {
            view.clipToOutline = false
            view.outlineProvider = null
        }
    }

    private fun unmountPage(pageId: String) {
        cancelInactivePageStop(pageId)
        pageComponents.remove(pageId)?.forEach { unmountComponent(it, pageId) }
    }

    private fun pausePage(pageId: String) {
        pageComponents[pageId]?.forEach { id ->
            components[id]?.apply {
                if (componentPlaybackIntent[id] == true) {
                    componentsPendingAutoResume.add(id)
                } else {
                    componentsPendingAutoResume.remove(id)
                }
                blur()
                handleCommand("pause", null)
                view.visibility = View.GONE
            }
        }
        scheduleInactivePageStop(pageId)
    }

    private fun resumePage(pageId: String) {
        cancelInactivePageStop(pageId)
        val host = hostViewRef.get() ?: return
        pageComponents[pageId]?.forEach { id ->
            components[id]?.apply {
                if (view.parent != host) {
                    (view.parent as? ViewGroup)?.removeView(view)
                    host.addView(view)
                }
                view.visibility = View.VISIBLE
                if (componentsPendingAutoResume.remove(id)) {
                    handleCommand("play", null)
                }
            }
        }
    }

    private fun scheduleInactivePageStop(pageId: String) {
        cancelInactivePageStop(pageId)
        val task = Runnable {
            val ids = pageComponents[pageId]?.toList().orEmpty()
            ids.forEach { id ->
                components[id]?.handleCommand("stop", null)
            }
            pageInactiveStopRunnables.remove(pageId)
        }
        pageInactiveStopRunnables[pageId] = task
        mainHandler.postDelayed(task, INACTIVE_PAGE_STOP_DELAY_MS)
    }

    private fun cancelInactivePageStop(pageId: String) {
        pageInactiveStopRunnables.remove(pageId)?.let { mainHandler.removeCallbacks(it) }
    }

    private fun unmountComponent(id: String, pageId: String?) {
        webOverlayCoverageRestore.remove(id)
        focusVisibilityRunnables.remove(id)?.forEach { mainHandler.removeCallbacks(it) }
        componentsPendingAutoResume.remove(id)
        componentPlaybackIntent.remove(id)
        readyComponentIds.remove(id)
        pendingEventsByComponent.remove(id)
        // Unregister first to block any queued JNI command from being routed back into a component
        // that is in the middle of teardown.
        ComponentRouter.unregister(id)
        val callbackId = componentCallbacks.remove(id)
        clearRectSyncRetry(id)
        components.remove(id)?.let { component ->
            try {
                component.unmount()
            } catch (e: Exception) {
                Log.w(logTag, "component unmount failed: $id", e)
            }
        }
        componentContentRects.remove(id)
        componentScreenRects.remove(id)
        componentPage.remove(id)
        componentType.remove(id)
        componentPageFuncBindings.remove(id)
        componentDataset.remove(id)
        componentAdjustPosition.remove(id)
        pageId?.let { pid ->
            pageComponents[pid]?.let { ids ->
                ids.remove(id)
                if (ids.isEmpty()) {
                    pageComponents.remove(pid)
                    cancelInactivePageStop(pid)
                }
            }
        }
        callbackId?.let {
            val payload = JSONObject()
            payload.put("action", "component.event")
            payload.put("id", id)
            payload.put("componentId", id)
            payload.put("event", "unmount")
            payload.put("detail", JSONObject())
            NativeApi.onCallback(it, true, payload.toString())
        }
    }
}
