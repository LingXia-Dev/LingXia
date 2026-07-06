package com.lingxia.lxapp.APIs

import android.util.Log
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.app.NativeApi

internal object LxAppCapsule {
    private const val TAG = "LingXia.Capsule"

    @JvmStatic
    fun getCapsuleRect(callbackId: Long) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            LxLog.e(TAG, "Current activity not available")
            NativeApi.onCallback(callbackId, false, "2001") // System error
            return
        }

        activity.runOnUiThread {
            try {
                Log.i(TAG, "Running getCapsuleRect on UI thread")

                val jsonString = activity.getCapsuleRectJSON()
                if (jsonString.isEmpty() || jsonString == "{}") {
                    LxLog.w(TAG, "Capsule rect not available")
                    NativeApi.onCallback(callbackId, false, "2001") // Not found
                    return@runOnUiThread
                }

                Log.i(TAG, "Capsule rect (dp): $jsonString")
                NativeApi.onCallback(callbackId, true, jsonString)
            } catch (e: Exception) {
                LxLog.e(TAG, "getCapsuleRect error", e)
                NativeApi.onCallback(callbackId, false, "2001") // System error
            }
        }
    }

}
