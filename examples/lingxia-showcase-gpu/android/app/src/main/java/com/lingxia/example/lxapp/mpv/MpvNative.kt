package com.lingxia.example.lxapp.mpv

import android.util.Log

/**
 * Loads libmpv before any [MpvPlayerEngine] is constructed.
 * [com.lingxia.app.Lingxia.setUrlPlayerEngineFactory] must not load libraries
 * on the same line as the setter.
 */
object MpvNative {
    private const val TAG = "LingXia.MpvNative"

    @Volatile
    var available: Boolean = false
        private set

    fun load() {
        if (available) return
        try {
            System.loadLibrary("mpv")
            System.loadLibrary("player")
            available = true
            Log.i(TAG, "libmpv loaded")
        } catch (t: Throwable) {
            available = false
            Log.e(TAG, "libmpv load failed; URL playback will use ExoPlayer", t)
        }
    }
}
