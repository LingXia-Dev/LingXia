package com.lingxia.lxapp.APIs

import android.app.Activity
import android.content.Context
import android.util.Log
import android.content.Intent
import android.net.Uri
import org.json.JSONObject

/**
 * Device-related APIs shared by LxApp JNI surface on Android.
 */
object LxAppDevice {
    private const val TAG = "LingXia.Device"

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
            val vibrator = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) {
                val vibratorManager = activity.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as android.os.VibratorManager
                vibratorManager.defaultVibrator
            } else {
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
}
