package com.lingxia.lxapp

import org.junit.Assert.assertEquals
import org.junit.Test

class KeptPageGeometryTest {
    private val status = 117
    private val tabBar = 200

    /** Three tab pages: a navbar page (top = status + navbar) between two immersive ones. */
    private val own = listOf(
        KeptPageGeometry.Margins(0, tabBar, 0, 0),
        KeptPageGeometry.Margins(status + 137, tabBar, 0, 0),
        KeptPageGeometry.Margins(0, tabBar, 0, 0),
    )

    /** Where a page is placed (x, y, width, height) under the parent's current margins. */
    private fun placed(parentWidth: Int, parentHeight: Int, page: Int, current: Int): List<Float> {
        val margins = own[current]
        if (page == current) return listOf(
            margins.left.toFloat(), margins.top.toFloat(),
            (parentWidth - margins.left - margins.right).toFloat(),
            (parentHeight - margins.top - margins.bottom).toFloat(),
        )
        val frame = KeptPageGeometry.frame(parentWidth, parentHeight, own[page], margins)
        return listOf(
            margins.left + frame.translationX, margins.top + frame.translationY,
            frame.width.toFloat(), frame.height.toFloat(),
        )
    }

    @Test
    fun keptPagesSitWhereTheyWillBeShownWhicheverPageIsCurrentAndAfterRotation() {
        for ((width, height) in listOf(1080 to 2504, 2387 to 1080)) {
            for (page in own.indices) {
                val shown = placed(width, height, page, page)
                for (current in own.indices) {
                    assertEquals("page $page under page $current at ${width}x$height",
                        shown, placed(width, height, page, current))
                }
            }
        }
    }
}
