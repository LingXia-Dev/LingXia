package com.lingxia.lxapp

import android.app.Activity
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

object PermissionManager {
    private data class PendingRequest(
        val permissions: Array<String>,
        val callback: (Boolean) -> Unit
    )

    private val pendingRequests = ConcurrentHashMap<Int, PendingRequest>()
    private val requestCodeGenerator = AtomicInteger(10_000)
    private val mainHandler = Handler(Looper.getMainLooper())

    fun ensurePermissions(
        activity: Activity,
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

        val requestCode = requestCodeGenerator.incrementAndGet()
        pendingRequests[requestCode] = PendingRequest(missing.toTypedArray(), onResult)

        ActivityCompat.requestPermissions(
            activity,
            missing.toTypedArray(),
            requestCode
        )
    }

    fun handleRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ): Boolean {
        val pending = pendingRequests.remove(requestCode) ?: return false
        val expected = pending.permissions.toSet()

        var granted = true
        for (index in permissions.indices) {
            val permission = permissions[index]
            val status = if (index < grantResults.size) grantResults[index] else PackageManager.PERMISSION_DENIED
            if (permission in expected && status != PackageManager.PERMISSION_GRANTED) {
                granted = false
                break
            }
        }

        mainHandler.post { pending.callback(granted) }
        return true
    }
}
