package com.lingxia.app.media.modules

import androidx.appcompat.app.AppCompatActivity
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import java.util.ServiceLoader

/** SPI implemented by the optional camera AAR. */
interface CameraModule {
    fun capture(activity: AppCompatActivity, mode: String, maxDuration: Int, callbackId: Long, cameraFacing: Int)
}

/** SPI implemented by the optional scanner AAR. */
interface ScannerModule {
    fun scan(activity: AppCompatActivity, scanTypes: IntArray, onlyFromCamera: Boolean, callbackId: Long)
}

/** Module discovery is lazy: playback-only hosts never initialize camera code. */
object MediaModules {
    internal fun <T> discover(type: Class<T>): T? =
        ServiceLoader.load(type, type.classLoader).singleOrNull()

    private val camera by lazy { discover(CameraModule::class.java) }
    private val scanner by lazy { discover(ScannerModule::class.java) }

    val hasCamera: Boolean get() = camera != null
    val hasScanner: Boolean get() = scanner != null

    fun capture(activity: AppCompatActivity, mode: String, maxDuration: Int, callbackId: Long, cameraFacing: Int) {
        val provider = camera ?: return unavailable("camera", callbackId)
        provider.capture(activity, mode, maxDuration, callbackId, cameraFacing)
    }

    fun scan(activity: AppCompatActivity, scanTypes: IntArray, onlyFromCamera: Boolean, callbackId: Long) {
        val provider = scanner ?: return unavailable("scanner", callbackId)
        provider.scan(activity, scanTypes, onlyFromCamera, callbackId)
    }

    private fun unavailable(module: String, callbackId: Long) {
        LxLog.w("LingXia.MediaModules", "Optional $module module is not installed")
        NativeApi.onCallback(callbackId, false, "MODULE_NOT_INSTALLED:$module")
    }
}
