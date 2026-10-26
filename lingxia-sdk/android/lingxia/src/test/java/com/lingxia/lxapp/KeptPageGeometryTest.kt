package com.lingxia.lxapp

import com.lingxia.lxapp.chrome.TabBarState
import org.junit.Assert.assertEquals
import org.junit.Test

class KeptPageGeometryTest {
    /** Three tab pages: a navbar page between two immersive ones. */
    private val chrome = listOf(
        KeptPageGeometry.Chrome(navigationBar = false, tabBar = true, fullscreen = false),
        KeptPageGeometry.Chrome(navigationBar = true, tabBar = true, fullscreen = false),
        KeptPageGeometry.Chrome(navigationBar = false, tabBar = true, fullscreen = false),
    )

    private fun bars(navigationBar: Int, tabBar: Int) = KeptPageGeometry.Bars(
        navigationBar = navigationBar, tabBarHeight = tabBar, tabBarWidth = 0,
        tabBarTransparent = false, position = TabBarState.Position.BOTTOM, systemBottom = 0,
    )

    /** Where a page is placed (x, y, width, height) under the parent's current margins. */
    private fun placed(w: Int, h: Int, bars: KeptPageGeometry.Bars, page: Int, current: Int): List<Float> {
        val margins = KeptPageGeometry.margins(chrome[current], bars)
        if (page == current) return listOf(
            margins.left.toFloat(), margins.top.toFloat(),
            (w - margins.left - margins.right).toFloat(), (h - margins.top - margins.bottom).toFloat(),
        )
        val frame = KeptPageGeometry.frame(w, h, KeptPageGeometry.margins(chrome[page], bars), margins)
        return listOf(
            margins.left + frame.translationX, margins.top + frame.translationY,
            frame.width.toFloat(), frame.height.toFloat(),
        )
    }

    @Test
    fun keptPagesSitWhereTheyWillBeShownUnderTheCurrentConfigurationsBars() {
        // Portrait with a bottom navigation bar inside the tab bar; landscape moves it aside.
        for ((size, bars) in listOf((1080 to 2160) to bars(308, 308), (2160 to 1080) to bars(198, 176))) {
            val (width, height) = size
            for (page in chrome.indices) {
                val shown = placed(width, height, bars, page, page)
                for (current in chrome.indices) {
                    assertEquals("page $page under page $current at ${width}x$height",
                        shown, placed(width, height, bars, page, current))
                }
            }
        }
    }

    @Test
    fun pagesWithoutTheTabBarStayAboveTheSystemBarAndTheKeyboard() {
        val bars = bars(300, 200).copy(systemBottom = 132)
        val pushed = KeptPageGeometry.Chrome(navigationBar = true, tabBar = false, fullscreen = false)
        assertEquals(KeptPageGeometry.Margins(300, 132, 0, 0), KeptPageGeometry.margins(pushed, bars))
        assertEquals(800, KeptPageGeometry.margins(pushed, bars, ime = 800).bottom)
        assertEquals(0, KeptPageGeometry.margins(pushed.copy(fullscreen = true, navigationBar = false), bars).bottom)
    }

    @Test
    fun aKeptPageReportsTheOverlapOfItsOwnTabBarAsShown() {
        // Transparent bar above a three-button navigation bar: the page runs under both.
        val page = KeptPageGeometry.Box(0, 0, 1080, 2160)
        val bar = KeptPageGeometry.Box(0, 1676, 1080, 1852)
        val tabPage = chrome[0]
        assertEquals(KeptPageGeometry.Overlap(0, 0, 484),
            KeptPageGeometry.overlap(page, bar, tabPage, TabBarState.Position.BOTTOM))
        assertEquals(KeptPageGeometry.Overlap(0, 0, 0),
            KeptPageGeometry.overlap(page, bar, tabPage.copy(tabBar = false), TabBarState.Position.BOTTOM))
        // An opaque bar's margins keep it off the page.
        assertEquals(KeptPageGeometry.Overlap(0, 0, 0),
            KeptPageGeometry.overlap(KeptPageGeometry.Box(0, 0, 1080, 1676), bar, tabPage, TabBarState.Position.BOTTOM))
    }
}
