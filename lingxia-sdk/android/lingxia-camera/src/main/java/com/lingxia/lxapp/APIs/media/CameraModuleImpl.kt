package com.lingxia.lxapp.APIs.media

import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.media.modules.CameraModule

class CameraModuleImpl : CameraModule {
    override fun capture(activity: AppCompatActivity, mode: String, maxDuration: Int, callbackId: Long, cameraFacing: Int) {
        MediaCaptureFragment.start(activity, mode, maxDuration, callbackId, cameraFacing)
    }
}
