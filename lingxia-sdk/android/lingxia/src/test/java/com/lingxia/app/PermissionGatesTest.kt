package com.lingxia.app

import android.app.Activity
import android.os.Looper
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.LooperMode

@RunWith(RobolectricTestRunner::class)
@LooperMode(LooperMode.Mode.PAUSED)
class PermissionGatesTest {
    private val camera = arrayOf("android.permission.CAMERA")
    private val controller = Robolectric.buildActivity(Activity::class.java).setup()
    private val activity: Activity = controller.get()
    private val events = mutableListOf<String>()

    /** Records every prompt and leaves it undecided for the test to resolve. */
    private inner class HeldGate : PermissionGate {
        val held = mutableListOf<PermissionPrompt>()
        override fun beforePrompt(prompt: PermissionPrompt) {
            events += "before:${prompt.feature}"
            held += prompt
        }
        override fun afterPrompt(prompt: PermissionPrompt, outcome: PermissionOutcome) {
            events += "after:${prompt.feature}:$outcome"
        }
    }

    @After
    fun tearDown() {
        // Releases any prompt a test left undecided in the shared queue.
        if (!activity.isDestroyed) controller.pause().stop().destroy()
        idle()
        Lingxia.setPermissionGate(null)
    }

    private fun request(feature: PermissionFeature, result: Boolean? = null) {
        PermissionGates.run(activity, feature, camera, declined = { events += "declined:$feature" }) { finish ->
            events += "prompt:$feature"
            if (result != null) finish(result)
        }
    }

    private fun idle() = shadowOf(Looper.getMainLooper()).idle()

    @Test
    fun withoutAGatePromptsSynchronously() {
        request(PermissionFeature.CAMERA, result = true)
        assertEquals(listOf("prompt:CAMERA"), events)
    }

    @Test
    fun denySkipsThePromptAndAfterPrompt() {
        Lingxia.setPermissionGate { prompt ->
            events += "before:${prompt.feature}"
            prompt.deny()
            prompt.proceed()
        }
        request(PermissionFeature.CAMERA)
        idle()
        assertEquals(listOf("before:CAMERA", "declined:CAMERA"), events)
    }

    @Test
    fun proceedPromptsOnceAndReportsTheOutcomeOnce() {
        val gate = HeldGate()
        Lingxia.setPermissionGate(gate)
        PermissionGates.run(activity, PermissionFeature.SCAN_CODE, camera, declined = { events += "declined" }) { finish ->
            events += "prompt"
            finish(true)
            finish(false)
        }
        idle()
        gate.held.single().proceed()
        gate.held.single().proceed()
        idle()
        assertEquals(listOf("before:SCAN_CODE", "prompt", "after:SCAN_CODE:GRANTED"), events)
    }

    @Test
    fun promptsAreGatedOneAtATime() {
        val gate = HeldGate()
        Lingxia.setPermissionGate(gate)
        request(PermissionFeature.CAMERA)
        request(PermissionFeature.LOCATION)
        idle()
        assertEquals(listOf("before:CAMERA"), events)
        gate.held[0].deny()
        idle()
        assertEquals(listOf("before:CAMERA", "declined:CAMERA", "before:LOCATION"), events)
    }

    @Test
    fun aPermissionAskedBeforeWithoutRationaleIsBlocked() {
        val gate = HeldGate()
        Lingxia.setPermissionGate(gate)
        request(PermissionFeature.CAMERA, result = false)
        idle()
        gate.held[0].proceed()
        idle()
        // First dismissal/denial without rationale is not yet permanent.
        assertEquals("after:CAMERA:DENIED", events.last())
        request(PermissionFeature.CAMERA)
        idle()
        assertEquals(true, gate.held[1].blocked)
        assertEquals(false, gate.held[0].blocked)
    }

    @Test
    fun destroyingTheActivityDeniesAnUndecidedPrompt() {
        val gate = HeldGate()
        Lingxia.setPermissionGate(gate)
        request(PermissionFeature.CAMERA)
        idle()
        controller.pause().stop().destroy()
        idle()
        gate.held.single().proceed()
        idle()
        assertEquals(listOf("before:CAMERA", "declined:CAMERA"), events)
    }
}
