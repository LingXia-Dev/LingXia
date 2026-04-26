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

    private data class Entry(
        val id: String,
        val appId: String,
        val pageInstanceId: String?,
        val overlay: FrameLayout,
        val webView: android.webkit.WebView
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

    private val entries = LinkedHashMap<String, Entry>()
    private val pendingRequests = LinkedHashMap<String, Request>()

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
        activity.runOnUiThread {
            if (request.content == CONTENT_URL) {
                mount(activity, request, createExternalWebView(activity, request.path))
            } else {
                pendingRequests[request.id] = request
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

        val rootView = activity.findViewById<ViewGroup>(android.R.id.content)
        if (rootView == null) {
            NativeApi.disposePageInstance(request.pageInstanceId, "failed")
            NativeApi.onSurfaceClosed(request.appId, request.id, "failed")
            return
        }
        (webView.parent as? ViewGroup)?.removeView(webView)

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            isClickable = true
            isFocusable = false
            setBackgroundColor(Color.parseColor("#80000000"))
        }
        overlay.setOnClickListener {
            close(request.id, request.appId, "user")
        }

        val surface = FrameLayout(activity).apply {
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(activity, 12).toFloat()
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
        webView.visibility = View.VISIBLE
        if (webView is com.lingxia.lxapp.WebView) {
            webView.resume()
            NativeApi.notifyPageInstanceMounted(request.pageInstanceId)
            NativeApi.notifyPageInstanceVisible(request.pageInstanceId)
        }

        entries[request.id] = Entry(
            request.id,
            request.appId,
            request.pageInstanceId.takeIf { request.content == CONTENT_PAGE },
            overlay,
            webView
        )
        Log.d(TAG, "presented id=${request.id} appId=${request.appId} path=${request.path}")
    }

    private fun closeEntry(id: String, appId: String, reason: String, notifyNative: Boolean = true): Boolean {
        val entry = entries[id] ?: return false
        if (entry.appId != appId) return false
        entries.remove(id)

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
        NativeApi.disposePageInstance(request.pageInstanceId, reason)
        NativeApi.onSurfaceClosed(appId, id, reason)
        return true
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
        return FrameLayout.LayoutParams(width, height).apply {
            gravity = when (request.position) {
                SurfacePosition.BOTTOM -> Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                SurfacePosition.LEFT -> Gravity.START or Gravity.CENTER_VERTICAL
                SurfacePosition.RIGHT -> Gravity.END or Gravity.CENTER_VERTICAL
                SurfacePosition.TOP -> Gravity.TOP or Gravity.CENTER_HORIZONTAL
                SurfacePosition.CENTER -> Gravity.CENTER
            }
            val margin = dp(activity, 16)
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
        "user", "programmatic", "owner_closed", "app_closed", "failed" -> reason.trim()
        else -> "unknown"
    }

    private fun isHttpUrl(value: String): Boolean =
        value.startsWith("https://", ignoreCase = true) ||
            value.startsWith("http://", ignoreCase = true)
}
