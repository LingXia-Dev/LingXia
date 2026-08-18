package com.lingxia.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import java.util.concurrent.atomic.AtomicReference

/**
 * MediaProjection coordinator. A token is consumed on the next session start
 * and is never reused. [MediaProjection.Callback.onStop] clears it so a
 * later start must collect a fresh consent result.
 *
 * Full-resolution RGBA is not copied into Rust; the encoder stays on this
 * side and posts opaque encoded packets through JNI.
 */
object MediaProjectionCapture {
    private val pendingResult = AtomicReference<Pair<Int, Intent>?>(null)
    private val activeProjection = AtomicReference<MediaProjection?>(null)

    @JvmStatic
    fun createIntent(context: Context): Intent {
        val manager = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        return manager.createScreenCaptureIntent()
    }

    /** Store a fresh activity result. Replaces any unused previous result. */
    @JvmStatic
    fun offerResult(resultCode: Int, data: Intent?) {
        if (resultCode != Activity.RESULT_OK || data == null) {
            pendingResult.set(null)
            return
        }
        pendingResult.set(resultCode to Intent(data))
    }

    /**
     * Consume the pending result for one provider session. Returns null when
     * the host must prompt again — the previous token is never recycled.
     */
    @JvmStatic
    fun takeFreshProjection(context: Context): MediaProjection? {
        val pending = pendingResult.getAndSet(null) ?: return null
        val manager = context.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        val projection = manager.getMediaProjection(pending.first, pending.second) ?: return null
        projection.registerCallback(
            object : MediaProjection.Callback() {
                override fun onStop() {
                    activeProjection.compareAndSet(projection, null)
                    nativeOnProjectionStopped()
                }
            },
            null,
        )
        activeProjection.set(projection)
        return projection
    }

    @JvmStatic
    fun stopActive() {
        activeProjection.getAndSet(null)?.stop()
    }

    @JvmStatic
    fun foregroundServiceTypes(visual: Boolean, systemAudio: Boolean, microphone: Boolean): Int {
        var types = 0
        if (visual || systemAudio) {
            if (Build.VERSION.SDK_INT >= 29) {
                types = types or android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
            }
        }
        if (microphone) {
            if (Build.VERSION.SDK_INT >= 29) {
                types = types or android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
            }
        }
        return types
    }

    @JvmStatic
    private external fun nativeOnProjectionStopped()
}
