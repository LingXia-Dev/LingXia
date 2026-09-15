package com.lingxia.example.lxapp

import android.app.Application
// import com.lingxia.example.lxapp.mpv.MpvNative

class ShowcaseApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Uncomment with lingxia.packageMpvJni=true and the MainActivity factory.
        // MpvNative.load()
    }
}
