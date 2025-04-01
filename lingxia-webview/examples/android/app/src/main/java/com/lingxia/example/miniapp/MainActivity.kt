package com.lingxia.example.miniapp

import android.os.Bundle
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.miniapp.MiniApp

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize MiniApp
        MiniApp.initialize(this)

        // Configure transparent status bar and navigation bar
        MiniApp.configureTransparentSystemBars(
            activity = this,
            lightStatusBars = true,
            lightNavigationBars = false,
            showStatusBars = true,
            showNavigationBars = false
        )

        // Create WebView
        val webView = MiniApp.attachMiniApp("demo", "index.html")
        webView.layoutParams = ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        setContentView(webView)
    }

    override fun onDestroy() {
        super.onDestroy()
        MiniApp.destroy()
    }
}