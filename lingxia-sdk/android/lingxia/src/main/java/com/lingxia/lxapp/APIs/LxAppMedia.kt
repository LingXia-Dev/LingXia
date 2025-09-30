package com.lingxia.lxapp.APIs

import android.util.Log
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.media.MediaPreviewActivity
import com.lingxia.lxapp.media.PreviewMediaPayload

internal object LxAppMedia {
    private const val TAG = "LingXia.LxAppMedia"

    @JvmStatic
    fun previewMedia(items: Array<PreviewMediaPayload>) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "previewMedia: current activity is null")
            return
        }
        if (items.isEmpty()) {
            Log.w(TAG, "previewMedia: invalid media payload")
            return
        }
        activity.runOnUiThread {
            MediaPreviewActivity.launch(activity, items)
        }
    }
}
