package com.lingxia.lxapp

import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedDispatcher
import com.lingxia.app.NativeApi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29, 35], manifest = Config.NONE)
class LxAppSystemBackCallbackTest {
    class HostActivity : ComponentActivity() {
        var backgroundCalls = 0
        var allowedNonRoot = false

        override fun moveTaskToBack(nonRoot: Boolean): Boolean {
            backgroundCalls++
            allowedNonRoot = nonRoot
            return true
        }
    }

    @Test
    fun onlyAnExplicitHomeRootResultBackgroundsTheTaskWithoutFinishingTheActivity() {
        Robolectric.buildActivity(HostActivity::class.java).setup().use { controller ->
            val activity = controller.get()
            var result = NativeApi.UI_EVENT_RESULT_HANDLED
            var defaultBackCalls = 0
            val dispatcher = OnBackPressedDispatcher { defaultBackCalls++ }
            dispatcher.addCallback(activity, LxAppSystemBackCallback(activity, { false }, { result }))

            // Page pop, rejected navigation, and unknown results all stay in the app.
            for (value in listOf(NativeApi.UI_EVENT_RESULT_HANDLED, NativeApi.UI_EVENT_RESULT_UNHANDLED, 99)) {
                result = value
                dispatcher.onBackPressed()
                assertEquals(0, activity.backgroundCalls)
            }
            result = NativeApi.UI_EVENT_RESULT_BACKGROUND_HOST
            dispatcher.onBackPressed()
            assertEquals(1, activity.backgroundCalls)
            assertTrue(activity.allowedNonRoot)
            assertFalse(activity.isFinishing)
            assertEquals(0, defaultBackCalls)
        }
    }

    @Test
    fun closingAnOverlayDoesNotAlsoNavigateOrBackgroundTheApp() {
        Robolectric.buildActivity(HostActivity::class.java).setup().use { controller ->
            val activity = controller.get()
            var overlayVisible = true
            var navigationCalls = 0
            val callback = LxAppSystemBackCallback(activity, {
                val wasVisible = overlayVisible
                overlayVisible = false
                wasVisible
            }, {
                navigationCalls++
                NativeApi.UI_EVENT_RESULT_BACKGROUND_HOST
            })
            val dispatcher = OnBackPressedDispatcher()
            dispatcher.addCallback(activity, callback)

            dispatcher.onBackPressed()
            assertFalse(overlayVisible)
            assertEquals(0, navigationCalls)
            assertEquals(0, activity.backgroundCalls)

            dispatcher.onBackPressed()
            assertEquals(1, navigationCalls)
            assertEquals(1, activity.backgroundCalls)
        }
    }
}
