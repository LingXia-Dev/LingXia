package com.lingxia.example.miniapp

import android.os.Bundle
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.miniapp.MiniApp
import com.lingxia.miniapp.WebView

class MainActivity : AppCompatActivity() {
    private lateinit var webView: WebView

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
        webView = MiniApp.attachMiniApp("demo", "index.html")
        webView.layoutParams = ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        setContentView(webView)
    }

    override fun onResume() {
        super.onResume()
        webView.resume()
    }

    override fun onPause() {
        super.onPause()
        webView.pause()
    }

    override fun onDestroy() {
        super.onDestroy()
        MiniApp.destroy()
    }
}
