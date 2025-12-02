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
 *     // Register custom JS extensions
 *     override fun registerExtensions() {
 *         registerNativeExtensions()
 *     }
 *
 *     private external fun registerNativeExtensions()
 * }
 * ```
 *
 * Automatically provides:
 * - User extension registration (via registerExtensions())
 * - LxApp SDK initialization
 * - Home LxApp automatic opening
 * - Edge-to-edge transparent system bars
 */
abstract class LxAppLaunchActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "LxAppLaunchActivity"
        private var extensionsRegistered = false
    }

    /**
     * Override to register custom native extensions.
     */
    protected open fun registerExtensions() {
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        Log.d(TAG, "Initializing LxApp...")

        // Register user extensions (once only)
        synchronized(Companion::class.java) {
            if (!extensionsRegistered) {
                val nativeReady = NativeApi.ensureLoaded()
                if (nativeReady) {
                    registerExtensions()
                    extensionsRegistered = true
                } else {
                    Log.w(TAG, "Native library unavailable; skipping registerExtensions()")
                }
            }
        }

        // Configure transparent system bars for edge-to-edge experience
        LxAppActivity.configureTransparentSystemBars(this)

        // Initialize LxApp SDK (handles init-once internally)
        LxApp.initialize(this)

        // Auto-open home LxApp
        LxApp.openHomeLxApp()
    }
}
