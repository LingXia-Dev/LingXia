package com.lingxia.lxapp.APIs

import android.app.Instrumentation
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.lxapp.getWebTag
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference

/**
 * Real touch gestures for test automation (JNI). Events go through the
 * system input dispatcher into the window when the platform lets the app
 * inject into its own window ("window"), else through the activity's
 * dispatch ("view"); either way every interceptor on the way sees them.
 */
internal object LxAppGesture {
    private class Finger(
        val downTime: Long,
        val window: Boolean,
        var x: Float,
        var y: Float,
    )

    private val fingers = ConcurrentHashMap<Long, Finger>()
    private val nextFinger = AtomicLong(1)
    private val instrumentation = Instrumentation()

    /**
     * Runs one gesture segment on the calling (non-main) thread and returns
     * `{delivered, events, durationMs, finger}` or `{error}`. Points are
     * `[x, y, atMs]`: page CSS pixels of [webtag]'s page, or window points
     * when [webtag] is empty.
     */
    @JvmStatic
    fun perform(appId: String, webtag: String, request: String): String = try {
        performChecked(appId, webtag, JSONObject(request)).toString()
    } catch (e: Throwable) {
        JSONObject().put("error", e.message ?: e.toString()).toString()
    }

    /** The on-screen part of the page (or window) as `{left, top, width, height}` in its own points. */
    @JvmStatic
    fun viewport(appId: String, webtag: String): String = try {
        onMain {
            val activity = presenting(appId)
            val view = if (webtag.isEmpty()) activity.window.decorView else {
                findPage(activity.window.decorView, webtag)
                    ?: throw IllegalStateException("page $webtag is not attached")
            }
            val visible = android.graphics.Rect()
            if (!view.getGlobalVisibleRect(visible)) throw IllegalStateException("page $webtag is not on screen")
            val at = IntArray(2).also { view.getLocationOnScreen(it) }
            val density = activity.resources.displayMetrics.density
            JSONObject()
                .put("left", ((visible.left - at[0]) / density).toDouble())
                .put("top", ((visible.top - at[1]) / density).toDouble())
                .put("width", (visible.width() / density).toDouble())
                .put("height", (visible.height() / density).toDouble())
        }.toString()
    } catch (e: Throwable) {
        JSONObject().put("error", e.message ?: e.toString()).toString()
    }

    /** `{phase, offset}` (offset in page CSS pixels), `{}` without an indicator, or `{error}`. */
    @JvmStatic
    fun pullRefreshState(appId: String, webtag: String): String = try {
        onMain {
            val activity = presenting(appId)
            val helper = activity.pullToRefreshHelper
            if (activity.getCurrentPageWebTag() != webtag) {
                throw IllegalStateException("page $webtag is not the presented page")
            }
            if (helper == null || !helper.isEnabled()) {
                JSONObject()
            } else {
                val density = activity.resources.displayMetrics.density
                JSONObject()
                    .put("phase", helper.indicatorPhase())
                    .put("offset", (helper.indicatorOffsetPx() / density).toDouble())
            }
        }.toString()
    } catch (e: Throwable) {
        JSONObject().put("error", e.message ?: e.toString()).toString()
    }

    private fun performChecked(appId: String, webtag: String, request: JSONObject): JSONObject {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            throw IllegalStateException("gestures cannot run on the main thread")
        }
        val points = request.getJSONArray("points")
        if (points.length() == 0) throw IllegalArgumentException("gesture has no points")
        val end = request.optString("end", "up")
        val continued = request.optLong("finger", 0L)
        // Screen origin and scale of the coordinate space, read once so a
        // pull that moves the page does not move the finger with it.
        val (originX, originY, scale) = onMain { origin(appId, webtag) }

        var finger = if (continued != 0L) {
            fingers.remove(continued) ?: throw IllegalStateException("finger $continued is not held")
        } else null
        val startedAt = SystemClock.uptimeMillis()
        var events = 0
        var window = finger?.window ?: true
        for (i in 0 until points.length()) {
            val point: JSONArray = points.getJSONArray(i)
            val x = originX + point.getDouble(0).toFloat() * scale
            val y = originY + point.getDouble(1).toFloat() * scale
            val due = startedAt + point.getDouble(2).toLong()
            val wait = due - SystemClock.uptimeMillis()
            if (wait > 0) SystemClock.sleep(wait)
            if (finger == null) {
                val downTime = SystemClock.uptimeMillis()
                window = inject(MotionEvent.ACTION_DOWN, downTime, x, y, preferWindow = true)
                finger = Finger(downTime, window, x, y)
            } else {
                inject(MotionEvent.ACTION_MOVE, finger.downTime, x, y, preferWindow = finger.window)
                finger.x = x
                finger.y = y
            }
            events++
        }
        val held = finger!!
        val id = when (end) {
            "hold" -> nextFinger.getAndIncrement().also { fingers[it] = held }
            "cancel" -> { inject(MotionEvent.ACTION_CANCEL, held.downTime, held.x, held.y, held.window); events++; 0L }
            else -> { inject(MotionEvent.ACTION_UP, held.downTime, held.x, held.y, held.window); events++; 0L }
        }
        val result = JSONObject()
            .put("delivered", if (window) "window" else "view")
            .put("events", events)
            .put("durationMs", (SystemClock.uptimeMillis() - startedAt).toDouble())
        if (id != 0L) result.put("finger", id)
        return result
    }

    /** True when the event went through the input dispatcher. */
    private fun inject(action: Int, downTime: Long, x: Float, y: Float, preferWindow: Boolean): Boolean {
        val event = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0)
        event.source = InputDevice.SOURCE_TOUCHSCREEN
        try {
            if (preferWindow) {
                try {
                    instrumentation.sendPointerSync(event)
                    return true
                } catch (_: SecurityException) {
                    // Injection refused: dispatch through the activity instead.
                }
            }
            onMain {
                val activity = LxApp.getCurrentActivity()
                    ?: throw IllegalStateException("no activity presents an lxapp")
                val decor = activity.window.decorView
                val at = IntArray(2).also { decor.getLocationOnScreen(it) }
                event.offsetLocation(-at[0].toFloat(), -at[1].toFloat())
                activity.dispatchTouchEvent(event)
            }
            return false
        } finally {
            event.recycle()
        }
    }

    private fun presenting(appId: String): LxAppActivity {
        val activity = LxApp.getCurrentActivity()
            ?: throw IllegalStateException("no activity presents an lxapp")
        if (appId.isNotEmpty() && activity.appId != appId) throw IllegalStateException("$appId is not the presented lxapp")
        return activity
    }

    private fun origin(appId: String, webtag: String): Triple<Float, Float, Float> {
        val activity = presenting(appId)
        val density = activity.resources.displayMetrics.density
        val view = if (webtag.isEmpty()) {
            activity.window.decorView
        } else {
            findPage(activity.window.decorView, webtag)
                ?: throw IllegalStateException("page $webtag is not attached")
        }
        if (!view.isShown) throw IllegalStateException("page $webtag is not shown")
        val at = IntArray(2).also { view.getLocationOnScreen(it) }
        return Triple(at[0].toFloat(), at[1].toFloat(), density)
    }

    private fun findPage(root: View, webtag: String): View? {
        if (root.getWebTag() == webtag) return root
        if (root is ViewGroup) {
            for (i in 0 until root.childCount) {
                findPage(root.getChildAt(i), webtag)?.let { return it }
            }
        }
        return null
    }

    private fun <T> onMain(block: () -> T): T {
        if (Looper.myLooper() == Looper.getMainLooper()) return block()
        val result = AtomicReference<Result<T>>()
        val latch = CountDownLatch(1)
        Handler(Looper.getMainLooper()).post {
            result.set(runCatching(block))
            latch.countDown()
        }
        if (!latch.await(2, TimeUnit.SECONDS)) throw IllegalStateException("main thread did not answer")
        return result.get().getOrThrow()
    }
}
