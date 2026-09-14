package com.lingxia.lxapp.APIs.media

import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.media.modules.ScannerModule

class ScannerModuleImpl : ScannerModule {
    override fun scan(activity: AppCompatActivity, scanTypes: IntArray, onlyFromCamera: Boolean, callbackId: Long) {
        ScanCodeFragment.start(activity, scanTypes, onlyFromCamera, callbackId)
    }
}
