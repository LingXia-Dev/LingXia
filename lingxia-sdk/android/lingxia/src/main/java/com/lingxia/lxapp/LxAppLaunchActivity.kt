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
 * }
 * ```
 *
 * Automatically provides:
 * - LxApp SDK initialization
 * - Home LxApp automatic opening
 * - Edge-to-edge transparent system bars
 */
abstract class LxAppLaunchActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "LxAppLaunchActivity"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        Log.d(TAG, "Initializing LxApp...")

        // Configure transparent system bars for edge-to-edge experience
        LxAppActivity.configureTransparentSystemBars(this)

        // Initialize LxApp SDK
        LxApp.initialize(this)

        // Auto-open home LxApp
        LxApp.openHomeLxApp()
    }
}
