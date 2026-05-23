package com.lingxia.app

import androidx.appcompat.app.AppCompatActivity
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Top-level entry point for the LingXia SDK.
 */
object Lingxia {
    private val hostAddonInstalled = AtomicBoolean(false)

    /**
     * Product-app entry point. Initializes the runtime and opens the configured home LxApp.
     */
    @JvmStatic
    fun quickStart(activity: AppCompatActivity) {
        quickStart(activity, null)
    }

    /**
     * Product-app entry point with an app-owned native addon registrar.
     *
     * The SDK loads liblingxia before invoking [registerHostAddon], so host apps do not need to
     * call System.loadLibrary themselves.
     */
    @JvmStatic
    fun quickStart(activity: AppCompatActivity, registerHostAddon: (() -> Unit)?) {
        if (!NativeApi.ensureLoaded()) {
            throw IllegalStateException("Failed to load native library 'lingxia'")
        }
        if (registerHostAddon != null && hostAddonInstalled.compareAndSet(false, true)) {
            try {
                registerHostAddon()
            } catch (error: Throwable) {
                hostAddonInstalled.set(false)
                throw error
            }
        }
        LxApp.initializeRuntime(activity)
        LxApp.openHomeLxApp()
    }
}
