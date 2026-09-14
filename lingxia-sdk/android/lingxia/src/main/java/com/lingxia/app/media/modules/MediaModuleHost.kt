package com.lingxia.app.media.modules

import android.net.Uri
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.ActivityInsets
import com.lingxia.lxapp.APIs.media.MediaPickerFragment

/** SDK module support; keeps JNI and internal UI helpers inside the core AAR. */
object MediaModuleHost {
    fun onCallback(id: Long, success: Boolean, payload: String) = NativeApi.onCallback(id, success, payload)
    fun cacheDirectory(appId: String): String? =
        NativeApi.getLxAppInfo(appId)?.cacheDir?.trim()?.takeIf { it.isNotEmpty() }

    fun contentBottomInset(): Int = ActivityInsets.contentBottomInset()
    fun applyBottomMargin(root: ViewGroup, target: View, extra: Int) =
        ActivityInsets.applyBottomMargin(root, target, extra)

    fun pick(activity: AppCompatActivity, maxCount: Int, mode: String, allowCamera: Boolean,
             onPicked: (List<Uri>, Boolean) -> Unit) =
        MediaPickerFragment.pick(activity, maxCount, mode, allowCamera, onPicked)

    fun w(tag: String, message: String, error: Throwable? = null) { LxLog.w(tag, message, error) }
    fun e(tag: String, message: String, error: Throwable? = null) { LxLog.e(tag, message, error) }
}
