package com.lingxia.lxapp.APIs.media

import android.app.Activity
import android.os.Looper
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsControllerCompat
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.LooperMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31, 35])
@LooperMode(LooperMode.Mode.PAUSED)
class ImmersiveWindowUiTest {
    @Test
    fun previewRestoresBothIconAppearancesAndSwipeBehavior() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup()
        try {
            val window = activity.get().window
            val controller = WindowCompat.getInsetsController(window, window.decorView)
            for ((status, navigation) in listOf(true to true, true to false, false to true, false to false)) {
                controller.isAppearanceLightStatusBars = status
                controller.isAppearanceLightNavigationBars = navigation
                controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_DEFAULT
                val snapshot = ImmersiveWindowUi.capture(window)
                ImmersiveWindowUi.apply(window, keepScreenOn = false)
                shadowOf(Looper.getMainLooper()).idle()
                ImmersiveWindowUi.restore(window, snapshot)
                assertEquals(status, controller.isAppearanceLightStatusBars)
                assertEquals(navigation, controller.isAppearanceLightNavigationBars)
                assertEquals(WindowInsetsControllerCompat.BEHAVIOR_DEFAULT, controller.systemBarsBehavior)
            }
        } finally {
            activity.pause().stop().destroy()
        }
    }
}
