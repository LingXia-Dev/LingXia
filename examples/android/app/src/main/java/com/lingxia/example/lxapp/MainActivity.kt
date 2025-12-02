package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import com.lingxia.lxapp.LxAppLaunchActivity
import com.lingxia.lxapp.LxApp

class MainActivity : LxAppLaunchActivity() {
    private val TAG = "MainActivity"

    /**
     * Register custom native extensions.
     * Called once before LxApp initialization.
     */
    override fun registerExtensions() {
        registerNativeExtensions()
    }

    private external fun registerNativeExtensions()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Enable WebView debugging BEFORE calling super.onCreate()
        LxApp.enableWebViewDebugging()

        super.onCreate(savedInstanceState)

        Log.d(TAG, "LxApp is ready")
    }
}
