package com.lingxia.lxapp.APIs

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebViewClient
import android.widget.FrameLayout
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.lxapp.NativeApi
import com.lingxia.lxapp.NativeComponents.NativeBridge
import com.lingxia.lxapp.APIs.media.ImmersiveWindowUi
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import kotlin.math.roundToInt

internal enum class SurfacePosition(val value: Int) {
    CENTER(0),
    BOTTOM(1),
    LEFT(2),
    RIGHT(3),
    TOP(4);

    companion object {
        fun fromInt(value: Int): SurfacePosition = when (value) {
            BOTTOM.value -> BOTTOM
            LEFT.value -> LEFT
            RIGHT.value -> RIGHT
            TOP.value -> TOP
            else -> CENTER
        }
    }
}

internal object LxAppSurface {
    private const val TAG = "LingXia.Surface"
    private const val CONTENT_PAGE = 0
    private const val CONTENT_URL = 1
    private const val KIND_POPUP = 1
    private const val MOUNT_RETRY_COUNT = 40
    private const val MOUNT_RETRY_DELAY_MS = 50L

    // Entry is mutable so window snapshot can be re-captured if show() is
    // called after a hide() that already restored the window (defensive: the
    // first snapshot is taken at open time and we hold onto it across the
    // surface's lifetime, but we lazily re-capture if it's ever been nulled).
    private class Entry(
        val id: String,
        val appId: String,
        val pageInstanceId: String?,
        val overlay: FrameLayout,
        val webView: android.webkit.WebView,
        /**
         * True when the surface was opened with widthRatio≈1 AND heightRatio≈1.
         * In that case we apply ImmersiveWindowUi so the surface visually
         * extends behind the status bar / navigation bar / display cutout —
         * matching what `lx.previewMedia` does. Sub-full-screen surfaces keep
         * the system bars visible.
         */
        val immersive: Boolean,
        var windowSnapshot: ImmersiveWindowUi.Snapshot? = null,
    )

    private data class Request(
        val id: String,
        val appId: String,
        val path: String,
        val sessionId: Long,
        val pageInstanceId: String,
        val content: Int,
        val kind: Int,
        val width: Double,
        val height: Double,
        val widthRatio: Double,
        val heightRatio: Double,
        val position: SurfacePosition
    )

    private enum class PendingVisibility {
        SHOW,
        HIDE
    }

    private val entries = LinkedHashMap<String, Entry>()
    private val pendingRequests = LinkedHashMap<String, Request>()
    private val pendingVisibility = LinkedHashMap<String, PendingVisibility>()

    @JvmStatic
    fun present(
        id: String,
        appId: String,
        path: String,
        sessionId: Long,
        pageInstanceId: String,
        content: Int,
        kind: Int,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int
    ): Boolean {
        if (id.isBlank() || appId.isBlank() || sessionId <= 0L) return false
        if (kind != KIND_POPUP) return false
        if (content == CONTENT_PAGE && pageInstanceId.isBlank()) return false
        if (content == CONTENT_URL && !isHttpUrl(path)) return false
        if (content != CONTENT_PAGE && content != CONTENT_URL) return false
        val activity = LxApp.getCurrentActivity() ?: return false
        if (activity.getAppId() != appId) {
            Log.w(TAG, "present: active appId=${activity.getAppId()} does not match $appId")
            return false
        }

        val request = Request(
            id = id,
            appId = appId,
            path = path,
            sessionId = sessionId,
            pageInstanceId = pageInstanceId,
            content = content,
            kind = kind,
            width = width,
            height = height,
            widthRatio = widthRatio,
            heightRatio = heightRatio,
            position = SurfacePosition.fromInt(position)
        )
        pendingRequests[request.id] = request
        activity.runOnUiThread {
            if (request.content == CONTENT_URL) {
                mount(activity, request, createExternalWebView(activity, request.path))
            } else {
                mountWhenReady(activity, request, 0)
            }
        }
        return true
    }

    @JvmStatic
    fun close(id: String, appId: String, reason: String): Boolean {
        if (id.isBlank() || appId.isBlank()) return false
        val activity = LxApp.getCurrentActivity() ?: return false
        activity.runOnUiThread {
            val normalizedReason = normalizeReason(reason)
            if (!closeEntry(id, appId, normalizedReason)) {
                if (!closePendingRequest(id, appId, normalizedReason)) {
                    NativeApi.onSurfaceClosed(appId, id, normalizedReason)
                }
            }
        }
        return true
    }

    /**
     * Toggle the surface to visible without tearing it down. Leaves the WebView
     * attached so the page state survives — a subsequent hide() then show()
     * round-trip restores the same scroll position, form input, and JS state.
     *
     * Also routes a page.lifecycle/state:active message through NativeBridge so
     * any native overlay components (video player, media swiper, ...) come back
     * online: their views are re-shown, blur is cleared, and components that
     * were playing before the matching hide() auto-resume. WebView.resume()
     * doesn't do this on its own (only pause() emits the inactive event).
     */
    @JvmStatic
    fun show(id: String, appId: String): Boolean {
        if (id.isBlank() || appId.isBlank()) return false
        val entry = entries[id]
        if (entry == null) {
            return setPendingVisibility(id, appId, PendingVisibility.SHOW)
        }
        if (entry.appId != appId) return false
        val activity = LxApp.getCurrentActivity() ?: return false
        activity.runOnUiThread {
            applyEntryVisibility(activity, entry, true)
        }
        return true
    }

    /**
     * Toggle the surface to hidden without tearing it down. The overlay is
     * collapsed visually but the WebView and page instance stay alive, so a
     * subsequent show() restores the same state instead of remounting.
     */
    @JvmStatic
    fun hide(id: String, appId: String): Boolean {
        if (id.isBlank() || appId.isBlank()) return false
        val entry = entries[id]
        if (entry == null) {
            return setPendingVisibility(id, appId, PendingVisibility.HIDE)
        }
        if (entry.appId != appId) return false
        val activity = LxApp.getCurrentActivity() ?: return false
        activity.runOnUiThread {
            applyEntryVisibility(activity, entry, false)
        }
        return true
    }

    private fun mountWhenReady(activity: Activity, request: Request, attempt: Int) {
        if (!pendingRequests.containsKey(request.id)) {
            return
        }
        val webView = NativeApi.findWebViewByPageInstanceId(request.pageInstanceId)
        if (webView == null) {
            if (attempt < MOUNT_RETRY_COUNT) {
                activity.window?.decorView?.postDelayed({ mountWhenReady(activity, request, attempt + 1) }, MOUNT_RETRY_DELAY_MS)
            } else {
                pendingRequests.remove(request.id)
                pendingVisibility.remove(request.id)
                Log.e(TAG, "present failed: WebView not ready for pageInstanceId=${request.pageInstanceId}")
                NativeApi.disposePageInstance(request.pageInstanceId, "failed")
                NativeApi.onSurfaceClosed(request.appId, request.id, "failed")
            }
            return
        }
        mount(activity, request, webView)
    }

    private fun mount(activity: Activity, request: Request, webView: android.webkit.WebView) {
        closeEntry(request.id, request.appId, "programmatic", notifyNative = false)
        pendingRequests.remove(request.id)
        val requestedVisibility = pendingVisibility.remove(request.id)

        val rootView = activity.findViewById<ViewGroup>(android.R.id.content)
        if (rootView == null) {
            if (request.content == CONTENT_PAGE) {
                NativeApi.disposePageInstance(request.pageInstanceId, "failed")
            } else {
                webView.stopLoading()
                webView.destroy()
            }
            NativeApi.onSurfaceClosed(request.appId, request.id, "failed")
            return
        }
        (webView.parent as? ViewGroup)?.removeView(webView)

        val immersive = isFullScreenRequest(request)
        val cornerRadiusPx = if (immersive) 0f else dp(activity, 12).toFloat()

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            isClickable = true
            isFocusable = false
            // No backdrop for full-screen surfaces — they cover the whole window
            // including system bar areas, so a translucent backdrop would only
            // be visible behind the rounded edges and corners which don't exist
            // when immersive.
            setBackgroundColor(if (immersive) Color.TRANSPARENT else Color.parseColor("#80000000"))
        }
        overlay.setOnClickListener {
            close(request.id, request.appId, "user")
        }

        val surface = FrameLayout(activity).apply {
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = cornerRadiusPx
            }
            clipToOutline = Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP
            elevation = dp(activity, 12).toFloat()
            isClickable = true
        }

        applySafeLayout(activity, rootView, overlay, surface, request)
        ViewCompat.setOnApplyWindowInsetsListener(overlay) { _, _ ->
            applySafeLayout(activity, rootView, overlay, surface, request)
            WindowInsetsCompat.CONSUMED
        }
        surface.addView(webView, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        ))
        overlay.addView(surface)
        rootView.addView(overlay)
        ViewCompat.requestApplyInsets(overlay)

        if (webView is com.lingxia.lxapp.WebView) {
            NativeBridge.attachIfNeeded(webView)
        }
        val initiallyVisible = requestedVisibility != PendingVisibility.HIDE
        webView.visibility = if (initiallyVisible) View.VISIBLE else View.GONE
        if (webView is com.lingxia.lxapp.WebView) {
            NativeApi.notifyPageInstanceMounted(request.pageInstanceId)
            if (initiallyVisible) {
                webView.resume()
                NativeApi.notifyPageInstanceVisible(request.pageInstanceId)
            }
        }

        val entry = Entry(
            id = request.id,
            appId = request.appId,
            pageInstanceId = request.pageInstanceId.takeIf { request.content == CONTENT_PAGE },
            overlay = overlay,
            webView = webView,
            immersive = immersive,
        )
        entries[request.id] = entry

        if (requestedVisibility == PendingVisibility.HIDE) {
            applyEntryVisibility(activity, entry, false)
        }

        // Switch the host window to immersive (status / nav / cutout hidden,
        // decor extends edge-to-edge) so the surface visually covers the
        // status bar — same approach as MediaPreviewFragment. Snapshot the
        // prior state so we can restore on hide / close.
        if (immersive && requestedVisibility != PendingVisibility.HIDE) {
            entry.windowSnapshot = ImmersiveWindowUi.capture(activity.window)
            ImmersiveWindowUi.apply(activity.window, keepScreenOn = false)
        }

        Log.d(TAG, "presented id=${request.id} appId=${request.appId} path=${request.path} immersive=$immersive")
    }

    /**
     * A surface counts as "full-screen" when both ratios are essentially 1.0.
     * We accept a small epsilon because JS sometimes sends 99/100 or 100/100
     * percent strings that round-trip with float noise.
     */
    private fun isFullScreenRequest(request: Request): Boolean {
        val w = request.widthRatio
        val h = request.heightRatio
        return w.isFinite() && h.isFinite() && w >= 0.999 && h >= 0.999
    }

    private fun closeEntry(id: String, appId: String, reason: String, notifyNative: Boolean = true): Boolean {
        val entry = entries[id] ?: return false
        if (entry.appId != appId) return false
        entries.remove(id)

        // Hand system bars back to whatever the host page had before the
        // surface opened. Safe even if hide() already restored — restore is
        // idempotent against the same snapshot.
        if (entry.immersive) {
            val activity = LxApp.getCurrentActivity()
            val window = activity?.window
            val snapshot = entry.windowSnapshot
            if (window != null && snapshot != null) {
                ImmersiveWindowUi.restore(window, snapshot)
            }
            entry.windowSnapshot = null
        }

        (entry.webView.parent as? ViewGroup)?.removeView(entry.webView)
        (entry.overlay.parent as? ViewGroup)?.removeView(entry.overlay)
        if (entry.webView is com.lingxia.lxapp.WebView) {
            entry.webView.pause()
        } else {
            entry.webView.stopLoading()
            entry.webView.destroy()
        }

        if (notifyNative) {
            entry.pageInstanceId?.let { pageInstanceId ->
                NativeApi.notifyPageInstanceHidden(pageInstanceId, reason)
            }
            NativeApi.onSurfaceClosed(appId, id, reason)
        }
        return true
    }

    private fun closePendingRequest(id: String, appId: String, reason: String): Boolean {
        val request = pendingRequests[id] ?: return false
        if (request.appId != appId) return false
        pendingRequests.remove(id)
        pendingVisibility.remove(id)
        if (request.content == CONTENT_PAGE) {
            NativeApi.disposePageInstance(request.pageInstanceId, reason)
        }
        NativeApi.onSurfaceClosed(appId, id, reason)
        return true
    }

    private fun setPendingVisibility(id: String, appId: String, visibility: PendingVisibility): Boolean {
        val request = pendingRequests[id] ?: return false
        if (request.appId != appId) return false
        pendingVisibility[id] = visibility
        return true
    }

    private fun applyEntryVisibility(
        activity: Activity,
        entry: Entry,
        visible: Boolean,
        notifyLifecycle: Boolean = true
    ) {
        val target = if (visible) View.VISIBLE else View.GONE
        // Defense in depth: the Rust JS-side closure already short-circuits
        // on no-op transitions, so under normal flow this guard is unreachable.
        // Skip the immersive flip + lifecycle re-notify if a future call path
        // ever forwards a redundant show/hide.
        if (entry.overlay.visibility == target) return
        entry.overlay.visibility = target
        entry.webView.visibility = target
        if (entry.immersive) {
            if (visible) {
                if (entry.windowSnapshot == null) {
                    entry.windowSnapshot = ImmersiveWindowUi.capture(activity.window)
                }
                ImmersiveWindowUi.apply(activity.window, keepScreenOn = false)
            } else {
                entry.windowSnapshot?.let { ImmersiveWindowUi.restore(activity.window, it) }
            }
        }
        if (notifyLifecycle && entry.webView is com.lingxia.lxapp.WebView) {
            if (visible) {
                entry.webView.resume()
                NativeBridge.notifyPageActive(entry.webView)
                entry.pageInstanceId?.let { NativeApi.notifyPageInstanceVisible(it) }
            } else {
                entry.webView.pause()
                entry.pageInstanceId?.let { NativeApi.notifyPageInstanceHidden(it, "hidden") }
            }
        }
    }

    @Suppress("DEPRECATION")
    private fun createExternalWebView(activity: Activity, url: String): android.webkit.WebView {
        return android.webkit.WebView(activity).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.databaseEnabled = true
            settings.allowFileAccess = false
            settings.allowContentAccess = false
            webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(
                    view: android.webkit.WebView?,
                    request: WebResourceRequest?
                ): Boolean {
                    val next = request?.url ?: return true
                    return !isSameOrigin(Uri.parse(url), next)
                }

                @Deprecated("Deprecated in Android")
                override fun shouldOverrideUrlLoading(view: android.webkit.WebView?, nextUrl: String?): Boolean {
                    val next = nextUrl?.let { Uri.parse(it) } ?: return true
                    return !isSameOrigin(Uri.parse(url), next)
                }
            }
            webChromeClient = WebChromeClient()
            loadUrl(url)
        }
    }

    private fun isSameOrigin(initial: Uri, next: Uri): Boolean {
        val initialScheme = initial.scheme?.lowercase()
        val nextScheme = next.scheme?.lowercase()
        if (initialScheme != nextScheme) return false
        if (!initial.host.equals(next.host, ignoreCase = true)) return false
        return effectivePort(initial) == effectivePort(next)
    }

    private fun effectivePort(uri: Uri): Int {
        if (uri.port > 0) return uri.port
        return when (uri.scheme?.lowercase()) {
            "http" -> 80
            "https" -> 443
            else -> -1
        }
    }

    private fun layoutParams(
        activity: Activity,
        request: Request,
        rootWidth: Int,
        rootHeight: Int,
        bottomInset: Int
    ): FrameLayout.LayoutParams {
        val defaultWidthRatio = 0.90
        val defaultHeightRatio = 0.55
        val width = resolveSize(activity, request.width, request.widthRatio, rootWidth, defaultWidthRatio)
        val height = resolveSize(activity, request.height, request.heightRatio, rootHeight, defaultHeightRatio)
        // Full-screen surfaces should have zero margins so they actually reach
        // edge to edge (and into the status / nav / cutout area once
        // ImmersiveWindowUi expands the decor). Sub-full surfaces keep the
        // 16dp inset that gives the card a visible gap from screen edges.
        val isFull = isFullScreenRequest(request)
        return FrameLayout.LayoutParams(width, height).apply {
            gravity = when (request.position) {
                SurfacePosition.BOTTOM -> Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                SurfacePosition.LEFT -> Gravity.START or Gravity.CENTER_VERTICAL
                SurfacePosition.RIGHT -> Gravity.END or Gravity.CENTER_VERTICAL
                SurfacePosition.TOP -> Gravity.TOP or Gravity.CENTER_HORIZONTAL
                SurfacePosition.CENTER -> Gravity.CENTER
            }
            val margin = if (isFull) 0 else dp(activity, 16)
            if (request.position != SurfacePosition.BOTTOM) topMargin = margin
            bottomMargin = if (request.position == SurfacePosition.BOTTOM) bottomInset
                else if (request.position != SurfacePosition.TOP) margin
                else 0
            if (request.position != SurfacePosition.RIGHT) leftMargin = margin
            if (request.position != SurfacePosition.LEFT) rightMargin = margin
        }
    }

    private fun applySafeLayout(
        activity: Activity,
        rootView: ViewGroup,
        overlay: FrameLayout,
        surface: FrameLayout,
        request: Request
    ) {
        val bottomInset = resolveSurfaceBottomInset(activity, rootView, request)
        if (overlay.paddingBottom != 0) overlay.setPadding(0, 0, 0, 0)

        val rootWidth = overlay.width
            .takeIf { it > 0 }
            ?: rootView.width.takeIf { it > 0 }
            ?: activity.resources.displayMetrics.widthPixels
        val rootHeight = overlay.height
            .takeIf { it > 0 }
            ?: rootView.height.takeIf { it > 0 }
            ?: activity.resources.displayMetrics.heightPixels
        val contentHeight = (rootHeight - bottomInset).coerceAtLeast(dp(activity, 160))
        surface.layoutParams = layoutParams(activity, request, rootWidth, contentHeight, bottomInset)
    }

    private fun resolveSurfaceBottomInset(activity: Activity, rootView: ViewGroup, request: Request): Int {
        val contentInset = (activity as? LxAppActivity)?.getContentBottomInset()
            ?: ViewCompat.getRootWindowInsets(rootView)
                ?.getInsets(WindowInsetsCompat.Type.navigationBars())
                ?.bottom
            ?: 0
        return if (request.position == SurfacePosition.BOTTOM) {
            (contentInset - rootView.paddingBottom).coerceAtLeast(0)
        } else {
            0
        }
    }

    private fun resolveSize(activity: Activity, absoluteDp: Double, ratio: Double, basePx: Int, defaultRatio: Double): Int {
        val raw = when {
            absoluteDp.isFinite() && absoluteDp > 0.0 -> dp(activity, absoluteDp).toDouble()
            ratio.isFinite() && ratio > 0.0 -> basePx * ratio.coerceAtMost(1.0)
            else -> basePx * defaultRatio
        }
        val upper = basePx.coerceAtLeast(1)
        val lower = dp(activity, 160).coerceAtMost(upper)
        return raw.roundToInt().coerceIn(lower, upper)
    }

    private fun dp(activity: Activity, value: Int): Int = dp(activity, value.toDouble())

    private fun dp(activity: Activity, value: Double): Int = (value * activity.resources.displayMetrics.density).roundToInt()

    private fun normalizeReason(reason: String): String = when (reason.trim()) {
        "user", "programmatic", "owner_closed", "app_closed", "reclaimed", "failed" -> reason.trim()
        else -> "unknown"
    }

    private fun isHttpUrl(value: String): Boolean =
        value.startsWith("https://", ignoreCase = true) ||
            value.startsWith("http://", ignoreCase = true)
}
