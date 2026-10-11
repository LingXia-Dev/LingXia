package com.lingxia.lxapp

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.LooperMode
import org.robolectric.util.ReflectionHelpers
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [21, 22, 31, 35])
@LooperMode(LooperMode.Mode.PAUSED)
class SplashOverlayTest {
    @Before
    fun freshProcess() {
        for (field in listOf("shownThisProcess", "homeReadySeen", "launchResolved",
            "launchFaceMarked", "placeholderActive")) {
            ReflectionHelpers.setStaticField(SplashOverlay::class.java, field, false)
        }
        ReflectionHelpers.setStaticField(SplashOverlay::class.java, "launchBackground", null)
        ReflectionHelpers.setStaticField(SplashOverlay::class.java, "launchCover", null)
        ReflectionHelpers.setStaticField(SplashOverlay::class.java, "firstDrawHolds", 0)
        LxApp.homeAppId = "home"
    }

    @Test
    fun unconfiguredLaunchHoldsBothActivitiesUntilHomeIsReadyWithoutAnExtraDelay() {
        val bootstrap = Robolectric.buildActivity(Activity::class.java).setup()
        val home = Robolectric.buildActivity(Activity::class.java).setup()
        try {
            home.get().setContentView(FrameLayout(home.get()))
            assertNull(SplashOverlay.backgroundColor(bootstrap.get()))
            assertFalse(SplashOverlay.attachBootstrap(bootstrap.get()))
            SplashOverlay.attachIfNeeded(home.get(), "home")
            assertTrue(content(bootstrap.get()).viewTreeObserver.dispatchOnPreDraw())
            assertTrue(content(home.get()).viewTreeObserver.dispatchOnPreDraw())
            assertFalse(SplashOverlay.coverOnScreen())
            assertTrue(SplashOverlay.launchActive())
            SplashOverlay.markLaunchFace(bootstrap.get())
            assertFalse(ReflectionHelpers.getStaticField<Boolean>(
                SplashOverlay::class.java, "launchFaceMarked"))

            SplashOverlay.notifyHomeReady()
            assertFalse(SplashOverlay.launchActive())
            // No clock advance: unconfigured launches do not inherit the 600ms hold.
            assertFalse(content(bootstrap.get()).viewTreeObserver.dispatchOnPreDraw())
            assertFalse(content(home.get()).viewTreeObserver.dispatchOnPreDraw())
        } finally {
            home.pause().stop().destroy()
            bootstrap.pause().stop().destroy()
        }
    }

    @Test
    fun aStalledLaunchReleasesAtTheDeadlineEvenWithoutAnotherDraw() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup()
        try {
            SplashOverlay.attachIfNeeded(activity.get(), "home")
            assertTrue(content(activity.get()).viewTreeObserver.dispatchOnPreDraw())
            shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(6_000))
            assertFalse(SplashOverlay.launchActive())
            assertFalse(ReflectionHelpers.getStaticField<Boolean>(
                SplashOverlay::class.java, "placeholderActive"))
            assertFalse(content(activity.get()).viewTreeObserver.dispatchOnPreDraw())
        } finally {
            activity.pause().stop().destroy()
        }
    }

    @Test
    fun anUnconfiguredLaunchPreservesTheHostWindowBackground() {
        for (color in listOf(Color.WHITE, Color.BLACK)) {
            val activity = Robolectric.buildActivity(Activity::class.java).setup()
            try {
                val background = ColorDrawable(color)
                activity.get().window.setBackgroundDrawable(background)
                SplashOverlay.attachBootstrap(activity.get())
                assertSame(background, activity.get().window.decorView.background)
                assertTrue(content(activity.get()).viewTreeObserver.dispatchOnPreDraw())
            } finally {
                activity.pause().stop().destroy()
            }
        }
    }

    @Test
    fun detachingTheContentReleasesItsLaunchHold() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().visible()
        try {
            SplashOverlay.attachIfNeeded(activity.get(), "home")
            val content = content(activity.get())
            assertTrue(SplashOverlay.launchActive())
            (content.parent as ViewGroup).removeView(content)
            assertFalse(SplashOverlay.launchActive())
            assertFalse(ReflectionHelpers.getStaticField<Boolean>(
                SplashOverlay::class.java, "placeholderActive"))
            shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(6_000))
            assertEquals(0, ReflectionHelpers.getStaticField<Int>(
                SplashOverlay::class.java, "firstDrawHolds"))
        } finally {
            activity.pause().stop().destroy()
        }
    }

    @Test
    fun warmAndNonHomeActivitiesAreNotHeld() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup()
        try {
            SplashOverlay.attachIfNeeded(activity.get(), "guest")
            assertFalse(content(activity.get()).viewTreeObserver.dispatchOnPreDraw())
            SplashOverlay.notifyHomeReady()
            SplashOverlay.attachBootstrap(activity.get())
            SplashOverlay.attachIfNeeded(activity.get(), "home")
            assertFalse(content(activity.get()).viewTreeObserver.dispatchOnPreDraw())
        } finally {
            activity.pause().stop().destroy()
        }
    }

    private fun content(activity: Activity): View = activity.findViewById(android.R.id.content)
}
