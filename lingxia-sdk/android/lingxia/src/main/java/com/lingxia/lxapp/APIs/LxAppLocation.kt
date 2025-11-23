package com.lingxia.lxapp.APIs

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Bundle
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.core.content.ContextCompat
import org.json.JSONObject
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.PermissionManager
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Android implementation of location-related APIs exposed to the native layer.
 */
object LxAppLocation {
    private const val TAG = "LingXia.Location"
    private const val LOCATION_TIMEOUT_MS = 10_000L
    private const val STALE_LOCATION_THRESHOLD_MS = 2 * 60 * 1000L
    private val LOCATION_PERMISSIONS = arrayOf(
        Manifest.permission.ACCESS_FINE_LOCATION,
        Manifest.permission.ACCESS_COARSE_LOCATION,
    )

    @JvmStatic
    fun isLocationEnabled(): Boolean {
        val context = LxApp.getCurrentActivity() ?: LxApp.applicationContext()
        if (context == null) {
            Log.w(TAG, "isLocationEnabled: no context available")
            return false
        }
        return isLocationEnabled(context)
    }

    @JvmStatic
    fun requestLocation(callbackId: Long, isHighAccuracy: Boolean, includeAltitude: Boolean, expireTimeMs: Int) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "requestLocation: current activity is null")
            val payload = JSONObject().apply { put("error", "No active activity") }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
            return
        }
        activity.runOnUiThread {
            requestSingleLocationWithConfig(activity, callbackId, isHighAccuracy, includeAltitude, expireTimeMs)
        }
    }

    fun isLocationEnabled(context: Context): Boolean {
        val locationManager = context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager
        if (locationManager == null) {
            Log.w(TAG, "LocationManager is not available")
            return false
        }

        return locationManager.isProviderEnabled(LocationManager.GPS_PROVIDER) ||
            locationManager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)
    }

    fun requestSingleLocation(activity: Activity, callbackId: Long) {
        requestSingleLocationWithConfig(activity, callbackId, false, false, 10000)
    }

    fun requestSingleLocationWithConfig(
        activity: Activity,
        callbackId: Long,
        isHighAccuracy: Boolean,
        includeAltitude: Boolean,
        expireTimeMs: Int
    ) {
        val locationManager = activity.getSystemService(Context.LOCATION_SERVICE) as? LocationManager
        if (locationManager == null) {
            sendFailure(callbackId, "location_services_unavailable", "Location service unavailable")
            return
        }

        val hasFinePermission = ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.ACCESS_FINE_LOCATION,
        ) == PackageManager.PERMISSION_GRANTED

        val hasCoarsePermission = ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.ACCESS_COARSE_LOCATION,
        ) == PackageManager.PERMISSION_GRANTED

        if (!hasFinePermission && !hasCoarsePermission) {
            // On the first request, prompt the system permission dialog and
            // only continue with the location request after the user decides.
            PermissionManager.ensurePermissions(activity, LOCATION_PERMISSIONS) { granted ->
                if (granted) {
                    requestSingleLocationWithConfig(activity, callbackId, isHighAccuracy, includeAltitude, expireTimeMs)
                } else {
                    sendFailure(callbackId, "location_permission_denied", "Location permission not granted")
                }
            }
            return
        }

        // Log configuration parameters
        Log.d(TAG, "Location request config - high_accuracy: $isHighAccuracy, include_altitude: $includeAltitude, expire_time: ${expireTimeMs}ms")

        // Choose providers based on accuracy requirements
        val providers = buildList {
            if (isHighAccuracy && locationManager.isProviderEnabled(LocationManager.GPS_PROVIDER)) {
                // High accuracy mode: prefer GPS
                add(LocationManager.GPS_PROVIDER)
                if (locationManager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)) {
                    add(LocationManager.NETWORK_PROVIDER)
                }
            } else {
                // Normal mode: prefer network, fallback to GPS
                if (locationManager.isProviderEnabled(LocationManager.NETWORK_PROVIDER)) {
                    add(LocationManager.NETWORK_PROVIDER)
                }
                if (locationManager.isProviderEnabled(LocationManager.GPS_PROVIDER)) {
                    add(LocationManager.GPS_PROVIDER)
                }
            }
        }

        if (providers.isEmpty()) {
            Log.w(TAG, "No enabled location provider")
            sendFailure(callbackId, "location_unavailable", "No location provider enabled")
            return
        }

        val bestLastLocation = providers
            .mapNotNull { provider ->
                try {
                    locationManager.getLastKnownLocation(provider)
                } catch (securityException: SecurityException) {
                    Log.e(TAG, "Failed to access last known location for $provider", securityException)
                    null
                }
            }
            .filterNot { isLocationStale(it) }
            .maxByOrNull { location -> location.time }

        if (bestLastLocation != null) {
            deliverSuccess(callbackId, bestLastLocation, includeAltitude)
            return
        }

        val handled = AtomicBoolean(false)
        val mainHandler = Handler(Looper.getMainLooper())

        lateinit var timeoutRunnable: Runnable

        val listener = object : LocationListener {
            override fun onLocationChanged(location: Location) {
                if (handled.compareAndSet(false, true)) {
                    mainHandler.removeCallbacks(timeoutRunnable)
                    locationManager.removeUpdates(this)
                    deliverSuccess(callbackId, location, includeAltitude)
                }
            }

            @Suppress("DEPRECATION")
            override fun onStatusChanged(provider: String?, status: Int, extras: Bundle?) {}

            override fun onProviderEnabled(provider: String) {}

            override fun onProviderDisabled(provider: String) {}
        }

        timeoutRunnable = Runnable {
            if (handled.compareAndSet(false, true)) {
                locationManager.removeUpdates(listener)
                sendFailure(callbackId, "location_timeout", "Location request timed out")
            }
        }

        // Use custom timeout if provided, otherwise use default
        val timeoutMs = if (expireTimeMs > 0) expireTimeMs.toLong() else LOCATION_TIMEOUT_MS
        mainHandler.postDelayed(timeoutRunnable, timeoutMs)

        activity.runOnUiThread {
            var started = false
            providers.forEach { provider ->
                try {
                    // Set update parameters based on accuracy requirements
                    val minTimeMs = if (isHighAccuracy) 1000L else 5000L  // High accuracy: 1s, Normal: 5s
                    val minDistanceM = if (isHighAccuracy) 0f else 10f    // High accuracy: 0m, Normal: 10m

                    locationManager.requestLocationUpdates(
                        provider,
                        minTimeMs,
                        minDistanceM,
                        listener,
                        Looper.getMainLooper(),
                    )
                    started = true
                    Log.d(TAG, "Started location updates for $provider (minTime: ${minTimeMs}ms, minDistance: ${minDistanceM}m)")
                } catch (securityException: SecurityException) {
                    Log.e(TAG, "Security exception when requesting updates from $provider", securityException)
                } catch (illegalArgumentException: IllegalArgumentException) {
                    Log.w(TAG, "Provider $provider is unavailable", illegalArgumentException)
                }
            }

            if (!started && handled.compareAndSet(false, true)) {
                locationManager.removeUpdates(listener)
                mainHandler.removeCallbacks(timeoutRunnable)
                sendFailure(callbackId, "location_unavailable", "Unable to request location updates")
            }
        }
    }

    private fun isLocationStale(location: Location): Boolean {
        val ageMs = System.currentTimeMillis() - location.time
        return ageMs > STALE_LOCATION_THRESHOLD_MS
    }

    private fun deliverSuccess(callbackId: Long, location: Location, includeAltitude: Boolean = true) {
        val horizontalAccuracy = if (location.hasAccuracy()) location.accuracy.toDouble() else 0.0
        val verticalAccuracy = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && location.hasVerticalAccuracy()) {
            location.verticalAccuracyMeters.toDouble()
        } else {
            0.0
        }

        val payload = JSONObject().apply {
            put("latitude", location.latitude)
            put("longitude", location.longitude)
            put("speed", if (location.hasSpeed()) location.speed.toDouble() else 0.0)
            put("accuracy", horizontalAccuracy)
            // Include altitude only if requested and available
            if (includeAltitude && location.hasAltitude()) {
                put("altitude", location.altitude)
            } else {
                put("altitude", 0.0)
            }
            put("vertical_accuracy", verticalAccuracy)
            put("horizontal_accuracy", horizontalAccuracy)
        }

        val success = com.lingxia.lxapp.NativeApi.onCallback(callbackId, true, payload.toString())
        if (!success) {
            Log.w(TAG, "Location callback $callbackId was not handled by native layer")
        }
    }

    private fun sendFailure(callbackId: Long, code: String, message: String) {
        val payload = JSONObject().apply {
            put("code", code)
            put("error", message)
        }

        val success = com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
        if (!success) {
            Log.w(TAG, "Failed to deliver location error for callback $callbackId")
        }
    }
}
