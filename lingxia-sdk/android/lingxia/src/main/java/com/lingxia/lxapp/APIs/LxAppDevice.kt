package com.lingxia.lxapp.APIs

import android.app.Activity
import android.Manifest
import android.content.Context
import android.util.Log
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import org.json.JSONObject
import com.lingxia.lxapp.LxApp
import androidx.core.content.ContextCompat
import java.io.File

/**
 * Device-related APIs shared by LxApp JNI surface on Android.
 */
internal object LxAppDevice {
    private const val TAG = "LingXia.Device"

    @JvmStatic
    fun getScreenInfo(callbackId: Long) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "getScreenInfo: current activity is null")
            val errorData = JSONObject().apply {
                put("width", 0)
                put("height", 0)
                put("scale", 1.0)
                put("error", "No active activity")
            }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, errorData.toString())
            return
        }
        activity.runOnUiThread {
            getScreenInfo(activity, callbackId)
        }
    }

    @JvmStatic
    fun vibrate(longVibration: Boolean) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "vibrate: current activity is null")
            return
        }
        activity.runOnUiThread {
            try {
                vibrate(activity, longVibration)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to vibrate", e)
            }
        }
    }

    @JvmStatic
    fun makePhoneCall(phoneNumber: String) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "makePhoneCall: current activity is null")
            return
        }
        activity.runOnUiThread {
            try {
                makePhoneCall(activity, phoneNumber)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to make phone call", e)
            }
        }
    }

    @JvmStatic
    fun readExternalStorageText(storageKey: String): String? {
        val file = resolveExternalFile(storageKey) ?: return null
        if (!ensureExternalStorageAccess()) return null

        return try {
            if (!file.exists() || !file.isFile) {
                null
            } else {
                file.readText(Charsets.UTF_8)
            }
        } catch (e: Exception) {
            Log.w(TAG, "readExternalStorageText failed: ${file.absolutePath}", e)
            null
        }
    }

    @JvmStatic
    fun writeExternalStorageText(storageKey: String, value: String): Boolean {
        val file = resolveExternalFile(storageKey) ?: return false
        if (!ensureExternalStorageAccess()) return false

        return try {
            file.parentFile?.mkdirs()
            file.writeText(value, Charsets.UTF_8)
            true
        } catch (e: Exception) {
            Log.w(TAG, "writeExternalStorageText failed: ${file.absolutePath}", e)
            false
        }
    }

    @JvmStatic
    fun readSecureStoreValueBase64(storageKey: String): String? {
        return AndroidSecureStore.readValueBase64(storageKey)
    }

    @JvmStatic
    fun writeSecureStoreValueBase64(storageKey: String, valueBase64: String) {
        AndroidSecureStore.writeValueBase64(storageKey, valueBase64)
    }

    @JvmStatic
    fun deleteSecureStoreValue(storageKey: String) {
        AndroidSecureStore.deleteValue(storageKey)
    }

    fun getScreenInfo(activity: Activity, callbackId: Long) {
        try {
            val displayMetrics = activity.resources.displayMetrics

            val widthDp = kotlin.math.round(displayMetrics.widthPixels / displayMetrics.density).toInt()
            val heightDp = kotlin.math.round(displayMetrics.heightPixels / displayMetrics.density).toInt()
            val scale = kotlin.math.round(displayMetrics.density * 10.0) / 10.0

            val screenInfo = JSONObject().apply {
                put("width", widthDp)
                put("height", heightDp)
                put("scale", scale)
            }

            val success = com.lingxia.lxapp.NativeApi.onCallback(callbackId, true, screenInfo.toString())
            if (!success) {
                Log.e(TAG, "Failed to send screen info callback for ID: $callbackId")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get screen info", e)
            val errorData = JSONObject().apply {
                put("width", 0)
                put("height", 0)
                put("scale", 1.0)
            }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, errorData.toString())
        }
    }

    fun vibrate(activity: Activity, longVibration: Boolean) {
        try {
            // API 31+ uses VibratorManager, but we use reflection to avoid compile-time dependency
            val vibrator: android.os.Vibrator = (if (android.os.Build.VERSION.SDK_INT >= 31) {
                try {
                    val vibratorManager = activity.getSystemService("vibrator_manager")
                    vibratorManager?.javaClass?.getMethod("getDefaultVibrator")?.invoke(vibratorManager) as? android.os.Vibrator
                } catch (e: Exception) {
                    null
                }
            } else null) ?: run {
                @Suppress("DEPRECATION")
                activity.getSystemService(Context.VIBRATOR_SERVICE) as android.os.Vibrator
            }

            if (!vibrator.hasVibrator()) {
                Log.w(TAG, "Device does not support vibration")
                return
            }

            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                val effect = when {
                    android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q && !longVibration -> {
                        android.os.VibrationEffect.createPredefined(android.os.VibrationEffect.EFFECT_TICK)
                    }
                    longVibration -> {
                        android.os.VibrationEffect.createOneShot(400L, android.os.VibrationEffect.DEFAULT_AMPLITUDE)
                    }
                    else -> {
                        val amplitude = if (vibrator.hasAmplitudeControl()) 255 else android.os.VibrationEffect.DEFAULT_AMPLITUDE
                        android.os.VibrationEffect.createOneShot(15L, amplitude)
                    }
                }
                vibrator.vibrate(effect)
            } else {
                @Suppress("DEPRECATION")
                val duration = if (longVibration) 400L else 15L
                vibrator.vibrate(duration)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to vibrate", e)
            throw e
        }
    }

    fun makePhoneCall(activity: Activity, phoneNumber: String) {
        try {
            val intent = Intent(Intent.ACTION_DIAL).apply {
                data = Uri.parse("tel:$phoneNumber")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to make phone call", e)
            throw e
        }
    }

    private fun resolveExternalFile(storageKey: String): File? {
        val key = storageKey.trim()
        if (key.isEmpty() || key.contains('/') || key.contains('\\') || key.contains("..")) {
            Log.w(TAG, "Invalid external storage key: $storageKey")
            return null
        }

        @Suppress("DEPRECATION")
        val root = Environment.getExternalStorageDirectory()
        if (root == null) {
            Log.w(TAG, "External storage directory unavailable")
            return null
        }

        val appId = resolveStorageAppId() ?: return null
        return File(root, ".lingxia/$appId/$key")
    }

    private fun resolveStorageAppId(): String? {
        val packageName = LxApp.applicationContext()?.packageName?.trim().orEmpty()
        if (packageName.isNotEmpty()) {
            return packageName
        }

        Log.w(TAG, "External storage appId unavailable")
        return null
    }

    private fun ensureExternalStorageAccess(): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: fingerprint path should use ANDROID_ID instead.
            Log.w(TAG, "External storage access disabled on API ${Build.VERSION.SDK_INT} (use ANDROID_ID path)")
            return false
        }

        // Android 5.x-6.x install-time permissions.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            return true
        }

        val permissions = buildStoragePermissions()
        val context = LxApp.applicationContext() ?: LxApp.getCurrentActivity()
        if (context == null) {
            Log.w(TAG, "External storage access denied: context unavailable")
            return false
        }
        return hasAllPermissions(context, permissions)
    }

    private fun buildStoragePermissions(): Array<String> {
        return buildList {
            add(Manifest.permission.READ_EXTERNAL_STORAGE)
            if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.Q) {
                add(Manifest.permission.WRITE_EXTERNAL_STORAGE)
            }
        }.toTypedArray()
    }

    private fun hasAllPermissions(context: Context, permissions: Array<String>): Boolean {
        return permissions.all { permission ->
            ContextCompat.checkSelfPermission(context, permission) == android.content.pm.PackageManager.PERMISSION_GRANTED
        }
    }

}
