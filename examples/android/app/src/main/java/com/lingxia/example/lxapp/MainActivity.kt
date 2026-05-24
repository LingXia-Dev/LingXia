package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.Lingxia

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    private external fun nativeRegisterHostAddon()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Lingxia.enableWebViewDebugging()
        Lingxia.quickStart(this) {
            nativeRegisterHostAddon()
        }

        Log.d(TAG, "Lingxia is ready")
    }
}
