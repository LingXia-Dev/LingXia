package com.lingxia.example.miniapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.miniapp.MiniApp
import com.lingxia.miniapp.MiniAppActivity

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize MiniApp
        MiniApp.initialize(this)

        // Configure transparent status bar using shared method
        MiniAppActivity.configureTransparentSystemBars(this)

        // Open Home MiniApp using the new method
        MiniApp.openHomeMiniApp()

        Log.d(TAG, "MainActivity initiated opening of home MiniApp")

        // Finish this activity since we're opening the home app in a new activity
        // The home MiniAppActivity will now function as our main activity
        finish()
    }
}