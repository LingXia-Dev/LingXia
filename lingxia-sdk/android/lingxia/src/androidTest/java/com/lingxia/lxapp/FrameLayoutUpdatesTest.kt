package com.lingxia.lxapp

import android.content.Context
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FrameLayoutUpdatesTest {
    private class CountingView(context: Context) : View(context) {
        var requests = 0
        override fun requestLayout() {
            requests++
            super.requestLayout()
        }
    }

    private fun onMain(block: () -> Unit) =
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)

    @Test fun repeatedIdenticalInsetsDoNotRequestLayout() = onMain {
        val view = CountingView(ApplicationProvider.getApplicationContext())
        val original = FrameLayout.LayoutParams(-1, -1, Gravity.CENTER)
        view.layoutParams = original
        view.requests = 0
        repeat(100) {
            assertFalse(view.updateFrameLayoutParamsIfChanged(FrameLayout.LayoutParams(original)))
        }
        assertSame(original, view.layoutParams)
        assertEquals(0, view.requests)
    }

    @Test fun geometryChangesStillRequestLayout() = onMain {
        val view = CountingView(ApplicationProvider.getApplicationContext())
        view.layoutParams = FrameLayout.LayoutParams(320, 240, Gravity.BOTTOM)
        val mutations: List<(FrameLayout.LayoutParams) -> Unit> = listOf(
            { it.width = 360 }, { it.height = 280 }, { it.gravity = Gravity.TOP },
            { it.leftMargin = 1 }, { it.topMargin = 2 },
            { it.rightMargin = 3 }, { it.bottomMargin = 4 },
            { it.marginStart = 5 }, { it.marginEnd = 6 }
        )
        for (mutate in mutations) {
            val next = FrameLayout.LayoutParams(view.layoutParams as FrameLayout.LayoutParams)
            mutate(next)
            view.requests = 0
            assertTrue(view.updateFrameLayoutParamsIfChanged(next))
            assertSame(next, view.layoutParams)
            assertEquals(1, view.requests)
            assertFalse(view.updateFrameLayoutParamsIfChanged(FrameLayout.LayoutParams(next)))
            assertEquals(1, view.requests)
        }
    }

    @Test fun firstAssignmentIsNotSkipped() = onMain {
        val view = CountingView(ApplicationProvider.getApplicationContext())
        view.requests = 0
        assertTrue(view.updateFrameLayoutParamsIfChanged(FrameLayout.LayoutParams(-1, -1)))
        assertEquals(1, view.requests)
    }
}
