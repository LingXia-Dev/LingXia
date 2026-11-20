package com.lingxia.app

import android.app.Activity
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

internal fun allExpectedPermissionsGranted(
    expected: Set<String>,
    permissions: Array<out String>,
    grantResults: IntArray
): Boolean {
    if (expected.isEmpty()) return true

    val resultsByPermission = HashMap<String, Int>(permissions.size)
    for (index in permissions.indices) {
        resultsByPermission[permissions[index]] = grantResults.getOrElse(index) {
            PackageManager.PERMISSION_DENIED
        }
    }
    return expected.all { permission ->
        resultsByPermission[permission] == PackageManager.PERMISSION_GRANTED
    }
}

internal object PermissionManager {
    private data class PendingRequest(
        val permissions: Array<String>,
        val callback: (Boolean) -> Unit,
        val finish: (Boolean) -> Unit,
    )

    private val pendingRequests = ConcurrentHashMap<Int, PendingRequest>()
    private val requestCodeGenerator = AtomicInteger(10_000)
    private val mainHandler = Handler(Looper.getMainLooper())

    fun ensurePermissions(
        activity: Activity,
        feature: PermissionFeature,
        permissions: Array<String>,
        onResult: (Boolean) -> Unit
    ) {
        val missing = permissions.filter {
            ContextCompat.checkSelfPermission(activity, it) != PackageManager.PERMISSION_GRANTED
        }

        if (missing.isEmpty()) {
            onResult(true)
            return
        }

        val requested = missing.toTypedArray()
        PermissionGates.run(activity, feature, requested, declined = { onResult(false) }) { finish ->
            val requestCode = requestCodeGenerator.incrementAndGet()
            pendingRequests[requestCode] = PendingRequest(requested, onResult, finish)
            ActivityCompat.requestPermissions(activity, requested, requestCode)
        }
    }

    /**
     * A web page's request ([permissions] from its geolocation or capture prompt), gated as the
     * lx feature it amounts to: a capture with a camera is [PermissionFeature.CAMERA] with its
     * microphone in the same prompt, as lx video capture asks; microphone alone is
     * [PermissionFeature.MICROPHONE]; location is [PermissionFeature.LOCATION]. A permission
     * the host app does not declare is denied without a prompt, which the system would refuse.
     */
    fun ensureWebPermissions(
        activity: Activity,
        permissions: Array<String>,
        onResult: (Boolean) -> Unit
    ) {
        val location = setOf(
            android.Manifest.permission.ACCESS_FINE_LOCATION,
            android.Manifest.permission.ACCESS_COARSE_LOCATION
        )
        val declared = declaredPermissions(activity)
        val feature = when {
            android.Manifest.permission.CAMERA in permissions -> PermissionFeature.CAMERA
            android.Manifest.permission.RECORD_AUDIO in permissions -> PermissionFeature.MICROPHONE
            permissions.isNotEmpty() && permissions.all { it in location } -> PermissionFeature.LOCATION
            else -> null
        }
        // Coarse location alone is enough for a page; any other permission must be declared.
        val requested = if (feature == PermissionFeature.LOCATION) {
            permissions.filter { it in declared }
        } else {
            permissions.toList().takeIf { list -> list.all { it in declared } }.orEmpty()
        }
        if (feature == null || requested.isEmpty()) {
            android.util.Log.i(
                "LingXia.Permission",
                "web permission request ${permissions.joinToString()} denied: " +
                    if (feature == null) "no lx feature covers it" else "the app does not declare it"
            )
            onResult(false)
            return
        }
        ensurePermissions(activity, feature, requested.toTypedArray(), onResult)
    }

    private fun declaredPermissions(activity: Activity): Set<String> = try {
        val info = activity.packageManager.getPackageInfo(activity.packageName, PackageManager.GET_PERMISSIONS)
        info.requestedPermissions?.toSet().orEmpty()
    } catch (_: PackageManager.NameNotFoundException) {
        emptySet()
    }

    fun handleRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ): Boolean {
        val pending = pendingRequests.remove(requestCode) ?: return false
        val expected = pending.permissions.toSet()

        val granted = allExpectedPermissionsGranted(expected, permissions, grantResults)

        pending.finish(granted)
        mainHandler.post { pending.callback(granted) }
        return true
    }
}
