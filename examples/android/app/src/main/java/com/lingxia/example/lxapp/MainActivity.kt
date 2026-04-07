package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import com.lingxia.lxapp.LxAppLaunchActivity
import com.lingxia.lxapp.LxApp

class MainActivity : LxAppLaunchActivity() {
    private val TAG = "MainActivity"

    /**
     * Install native host addon.
     * Called once before LxApp initialization.
     */
    override fun installHostAddon() {
        nativeInstallHostAddon()
    }

    private external fun nativeInstallHostAddon()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Enable WebView debugging BEFORE calling super.onCreate()
        LxApp.enableWebViewDebugging()

        super.onCreate(savedInstanceState)

        Log.d(TAG, "LxApp is ready")
    }
}
