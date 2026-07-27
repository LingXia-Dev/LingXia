package com.lingxia.app

import android.content.pm.PackageManager
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PermissionManagerTest {
    private val camera = "android.permission.CAMERA"
    private val microphone = "android.permission.RECORD_AUDIO"
    private val expected = setOf(camera, microphone)

    @Test
    fun emptyPermissionResultIsDenied() {
        assertFalse(allExpectedPermissionsGranted(expected, emptyArray(), intArrayOf()))
    }

    @Test
    fun missingExpectedPermissionIsDenied() {
        assertFalse(
            allExpectedPermissionsGranted(
                expected,
                arrayOf(camera),
                intArrayOf(PackageManager.PERMISSION_GRANTED)
            )
        )
    }

    @Test
    fun missingGrantResultIsDenied() {
        assertFalse(
            allExpectedPermissionsGranted(
                expected,
                arrayOf(camera, microphone),
                intArrayOf(PackageManager.PERMISSION_GRANTED)
            )
        )
    }

    @Test
    fun everyExpectedPermissionMustBeGranted() {
        assertFalse(
            allExpectedPermissionsGranted(
                expected,
                arrayOf(camera, microphone),
                intArrayOf(
                    PackageManager.PERMISSION_GRANTED,
                    PackageManager.PERMISSION_DENIED
                )
            )
        )
        assertTrue(
            allExpectedPermissionsGranted(
                expected,
                arrayOf(camera, microphone),
                intArrayOf(
                    PackageManager.PERMISSION_GRANTED,
                    PackageManager.PERMISSION_GRANTED
                )
            )
        )
    }
}
