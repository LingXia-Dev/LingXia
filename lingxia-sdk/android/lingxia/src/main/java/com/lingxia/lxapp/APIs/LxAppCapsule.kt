package com.lingxia.lxapp.APIs

import android.util.Log
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.app.NativeApi

internal object LxAppCapsule {
    private const val TAG = "LingXia.Capsule"

    @JvmStatic
    fun getCapsuleRect(callbackId: Long, appId: String) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            LxLog.w(TAG, "Current activity not available")
            NativeApi.onCallback(callbackId, false, "2001") // System error
            return
        }

        activity.runOnUiThread {
            try {
                Log.i(TAG, "Running getCapsuleRect on UI thread")

                val jsonString = activity.getCapsuleRectJSON(appId)
                if (jsonString.isEmpty() || jsonString == "{}") {
                    LxLog.e(TAG, "Invalid capsule rect payload")
                    NativeApi.onCallback(callbackId, false, "2001")
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
