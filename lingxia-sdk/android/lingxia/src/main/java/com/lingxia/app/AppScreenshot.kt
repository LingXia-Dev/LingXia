package com.lingxia.app

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Base64
import android.view.PixelCopy
import com.lingxia.app.Lingxia
import com.lingxia.lxapp.LxApp
import com.lingxia.app.NativeApi
import java.io.ByteArrayOutputStream
import android.view.View
import android.view.ViewGroup
import com.lingxia.view.GpuView
import org.json.JSONArray
import org.json.JSONObject

/**
 * Window-level screenshot for the host app.
 *
 * Captures the current foreground Activity's decor view, including any
 * host-drawn navigation bars, native overlays, and the contents of any
 * WebViews composited inside the window. This is the complement of the
 * per-WebView screenshot exposed by `WebViewController.take_screenshot`.
 *
 * Lives in `com.lingxia.app` (not `com.lingxia.lxapp.APIs`) because the
 * capture is at the host-app scope: an lxapp is one tenant inside the
 * window we're snapshotting, not the unit of capture itself.
 *
 * On API 26+ uses `PixelCopy.request(Window, Rect, Bitmap, Listener, Handler)`,
 * which honors hardware-accelerated layers (e.g. <video>, WebGL).
 * On older devices falls back to `View.draw(Canvas)` — hardware layers
 * may render blank under that path.
 */
object AppScreenshot {
    private const val TAG = "LingXia.AppScreenshot"

    @JvmStatic
    fun captureWindow(callbackId: Long) {
        val mainHandler = Handler(Looper.getMainLooper())
        mainHandler.post {
            try {
                val activity = Lingxia.getLastResumedActivity()
                if (activity == null) {
                    deliverError(callbackId, "no foreground activity")
                    return@post
                }
                val window = activity.window
                val decorView = window?.decorView
                if (decorView == null) {
                    deliverError(callbackId, "activity has no decor view")
                    return@post
                }
                val width = decorView.width
                val height = decorView.height
                if (width <= 0 || height <= 0) {
                    deliverError(callbackId, "decor view has zero size (${width}x${height})")
                    return@post
                }
                val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
                val surfaces = gpuSurfaces(decorView)

                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    val location = IntArray(2)
                    decorView.getLocationInWindow(location)
                    val srcRect = Rect(
                        location[0],
                        location[1],
                        location[0] + width,
                        location[1] + height,
                    )
                    PixelCopy.request(
                        window,
                        srcRect,
                        bitmap,
                        { result ->
                            try {
                                if (result == PixelCopy.SUCCESS) {
                                    deliverBitmap(callbackId, bitmap, surfaces)
                                } else {
                                    bitmap.recycle()
                                    deliverError(callbackId, "PixelCopy failed: result=$result")
                                }
                            } catch (e: Throwable) {
                                LxLog.e(TAG, "Failed to forward PixelCopy result", e)
                            }
                        },
                        mainHandler,
                    )
                } else {
                    val canvas = Canvas(bitmap)
                    decorView.draw(canvas)
                    deliverBitmap(callbackId, bitmap, surfaces)
                }
            } catch (e: Throwable) {
                deliverError(callbackId, "captureWindow threw: ${e.message ?: e.toString()}")
            }
        }
    }

    /**
     * GPU pages on screen, in the capture's pixels: PixelCopy of a window leaves their
     * SurfaceViews out, so native draws each page's newest frame in beneath the capture.
     */
    private fun gpuSurfaces(decorView: View): JSONArray {
        val surfaces = JSONArray()
        val origin = IntArray(2).also { decorView.getLocationInWindow(it) }
        fun visit(view: View) {
            if (view is GpuView) {
                val presenter = view.screenshotPresenter()
                if (presenter != 0L && view.width > 0 && view.height > 0) {
                    val at = IntArray(2).also { view.getLocationInWindow(it) }
                    surfaces.put(JSONObject()
                        .put("handle", presenter)
                        .put("x", at[0] - origin[0]).put("y", at[1] - origin[1])
                        .put("width", view.width).put("height", view.height))
                }
                return
            }
            if (view is ViewGroup) for (i in 0 until view.childCount) visit(view.getChildAt(i))
        }
        visit(decorView)
        return surfaces
    }

    private fun deliverBitmap(callbackId: Long, bitmap: Bitmap, surfaces: JSONArray) {
        try {
            val stream = ByteArrayOutputStream(
                Math.max(64 * 1024, bitmap.width * bitmap.height / 4),
            )
            val ok = bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream)
            bitmap.recycle()
            if (!ok) {
                deliverError(callbackId, "Bitmap.compress(PNG) returned false")
                return
            }
            val b64 = Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP)
            deliverEnvelope(callbackId, JSONObject().put("ok", true).put("data", b64).put("surfaces", surfaces))
        } catch (e: Throwable) {
            deliverError(callbackId, "PNG encode failed: ${e.message ?: e.toString()}")
        }
    }

    /// Always send `success=true` with a JSON envelope so failure strings
    /// survive `lingxia/src/ffi/android.rs::on_callback`'s `success=false`
    /// branch, which discards the message and reports business code 1000.
    private fun deliverEnvelope(callbackId: Long, envelope: JSONObject) {
        try {
            NativeApi.onCallback(callbackId, true, envelope.toString())
        } catch (callbackError: Throwable) {
            LxLog.e(TAG, "Failed to deliver screenshot envelope", callbackError)
        }
    }

    private fun deliverError(callbackId: Long, reason: String) {
        deliverEnvelope(callbackId, JSONObject().put("ok", false).put("error", reason))
    }
}
