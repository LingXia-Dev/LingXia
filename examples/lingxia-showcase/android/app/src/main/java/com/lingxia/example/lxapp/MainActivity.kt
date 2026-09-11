package com.lingxia.example.lxapp

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.Lingxia
// import com.lingxia.example.lxapp.mpv.MpvUrlPlayerEngineFactory

class MainActivity : AppCompatActivity() {
    private val TAG = "MainActivity"

    private external fun nativeRegisterHostAddon()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Default URL playback is ExoPlayer. To use host libmpv instead:
        // 1. Set android:name=".ShowcaseApp" on <application> in AndroidManifest.xml
        // 2. Uncomment MpvNative.load() in ShowcaseApp
        // 3. Uncomment the factory line below (must stay before quickStart)
        // Lingxia.setUrlPlayerEngineFactory(MpvUrlPlayerEngineFactory())
        Lingxia.quickStart(this) {
            nativeRegisterHostAddon()
        }

        Log.d(TAG, "Lingxia is ready")
    }
}
