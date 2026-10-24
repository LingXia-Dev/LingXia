package com.lingxia.lxapp.chrome

import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Rect
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [31])
@org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
class TabBarTypographyTest {
    @Test fun largeSystemLabelsFitInsideOpaqueAndTransparentBars() {
        val base = RuntimeEnvironment.getApplication()
        for (fontScale in listOf(1f, 2f)) for (color in listOf(Color.WHITE, Color.TRANSPARENT)) {
            val configuration = Configuration(base.resources.configuration).apply { this.fontScale = fontScale }
            val context = base.createConfigurationContext(configuration)
            val bar = TabBar(context)
            bar.setConfig(TabBarState(backgroundColor = color, list = listOf(
                TabBarItem(0, "home", "Home", ""), TabBarItem(1, "device", "Device", "")
            )))
            val density = context.resources.displayMetrics.density
            val height = bar.calculatedThicknessPx()
            if (fontScale == 1f) assertEquals((64 * density).toInt(), height)
            else assertTrue(height > (64 * density).toInt())
            val width = (360 * density).toInt()
            bar.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY))
            bar.layout(0, 0, width, height)
            fun labels(view: View): List<TextView> = when (view) {
                is TextView -> listOf(view)
                is ViewGroup -> (0 until view.childCount).flatMap { labels(view.getChildAt(it)) }
                else -> emptyList()
            }
            assertEquals(2, labels(bar).size)
            for (label in labels(bar)) {
                val bounds = Rect()
                label.getDrawingRect(bounds)
                bar.offsetDescendantRectToMyCoords(label, bounds)
                assertTrue(bounds.top >= 0 && bounds.bottom <= height)
                assertTrue(label.layout.getLineBottom(0) <= label.height)
            }
        }
    }
}
