package com.lingxia.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity

/**
 * LxApp initialization Activity
 *
 * This is the recommended way to use LxApp in Android.
 * Simply extend this class for complete automatic setup:
 *
 * ```kotlin
 * class MainActivity : LxAppLaunchActivity() {
 *     override fun onCreate(savedInstanceState: Bundle?) {
 *         LxApp.enableWebViewDebugging() // Optional, for development only
 *         super.onCreate(savedInstanceState)
 *         // Your app logic here - LxApp is fully ready
 *     }
 *
 *     // Install native host addon
 *     override fun installHostAddon() {
 *         nativeInstallHostAddon()
 *     }
 *
 *     private external fun nativeInstallHostAddon()
 * }
 * ```
 *
 * Automatically provides:
 * - Native host addon installation (via installHostAddon())
 * - LxApp SDK initialization
 * - Home LxApp automatic opening
 * - Edge-to-edge transparent system bars
 */
abstract class LxAppLaunchActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "LxAppLaunchActivity"
        private var hostAddonInstalled = false
    }

    /**
     * Override to install native host addon.
     */
    protected open fun installHostAddon() {
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        Log.d(TAG, "Initializing LxApp...")

        // Install native host addon (once only)
        synchronized(Companion::class.java) {
            if (!hostAddonInstalled) {
                val nativeReady = NativeApi.ensureLoaded()
                if (nativeReady) {
                    installHostAddon()
                    hostAddonInstalled = true
                } else {
                    Log.w(TAG, "Native library unavailable; skipping installHostAddon()")
                }
            }
        }

        // Configure transparent system bars for edge-to-edge experience
        LxAppActivity.configureTransparentSystemBars(this)

        // Initialize LingXia SDK (handles init-once internally)
        Lingxia.initialize(this)

        // Auto-open home LxApp
        LxApp.openHomeLxApp()
    }
}
