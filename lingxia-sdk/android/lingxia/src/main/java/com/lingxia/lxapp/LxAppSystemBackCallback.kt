package com.lingxia.lxapp

import android.app.Activity
import androidx.activity.OnBackPressedCallback
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi

/** One dispatch path for both the navigation key and Android's back gesture. */
internal class LxAppSystemBackCallback(
    private val activity: Activity,
    private val closeOverlay: () -> Boolean,
    private val dispatchBack: () -> Int,
) : OnBackPressedCallback(true) {
    override fun handleOnBackPressed() {
        try {
            if (closeOverlay()) return
            if (dispatchBack() == NativeApi.UI_EVENT_RESULT_BACKGROUND_HOST) {
                // The launcher/bootstrap Activity remains beneath LxAppActivity.
                // Finishing only this Activity would reveal that empty bootstrap.
                // Background the whole task, preserving the runtime for a warm return.
                if (!activity.moveTaskToBack(true)) {
                    LxLog.e("LxAppActivity", "System Back could not background the host task")
                }
            }
        } catch (error: Exception) {
            LxLog.e("LxAppActivity", "Error handling system Back: ${error.message}")
        }
    }
}
