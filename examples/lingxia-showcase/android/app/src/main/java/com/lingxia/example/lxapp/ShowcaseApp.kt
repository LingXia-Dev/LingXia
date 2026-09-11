package com.lingxia.example.lxapp

import android.app.Application
// import com.lingxia.example.lxapp.mpv.MpvNative

class ShowcaseApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Uncomment with MainActivity.setUrlPlayerEngineFactory to use libmpv.
        // MpvNative.load()
    }
}
