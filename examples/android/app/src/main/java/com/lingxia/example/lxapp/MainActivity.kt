package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.Lingxia
import com.lingxia.lxapp.LxApp

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    private external fun nativeInstallHostAddon()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        LxApp.enableWebViewDebugging()
        Lingxia.quickStart(this) {
            nativeInstallHostAddon()
        }

        Log.d(TAG, "LxApp is ready")
    }
}
