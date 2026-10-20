package com.lingxia.lxapp

import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29, 35])
class LxAppThemeTest {
    /**
     * Force dark recolours a light window whenever the system is dark, even
     * when the lxapp is pinned to light. The activity theme must opt out
     * before the window exists; the view-level opt-out in onCreate runs too
     * late for OEM window force dark.
     */
    @Test
    fun theLxAppActivityThemeOptsOutOfForceDark() {
        val context = RuntimeEnvironment.getApplication()
        context.setTheme(R.style.Theme_LingXia_LxApp)
        val attributes = context.obtainStyledAttributes(intArrayOf(android.R.attr.forceDarkAllowed))
        try {
            assertFalse(attributes.getBoolean(0, true))
        } finally {
            attributes.recycle()
        }
    }
}
