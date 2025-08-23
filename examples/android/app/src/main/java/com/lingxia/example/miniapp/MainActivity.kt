package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.LxApp

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        LxApp.initialize(this)
        LxApp.enableWebViewDebugging()
        LxApp.openHomeLxApp()
        Log.d(TAG, "MainActivity initiated opening of home LxApp")
    }
}
