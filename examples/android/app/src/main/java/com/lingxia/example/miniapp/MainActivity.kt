package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize LxApp
        LxApp.initialize(this)

        // Enable WebView debugging
        LxApp.enableWebViewDebugging()

        // Configure transparent status bar using shared method
        LxAppActivity.configureTransparentSystemBars(this)

        // Open Home LxApp using the new method
        LxApp.openHomeLxApp()

        Log.d(TAG, "MainActivity initiated opening of home LxApp")

        // Finish this activity since we're opening the home app in a new activity
        // The home LxAppActivity will now function as our main activity
        finish()
    }
}
