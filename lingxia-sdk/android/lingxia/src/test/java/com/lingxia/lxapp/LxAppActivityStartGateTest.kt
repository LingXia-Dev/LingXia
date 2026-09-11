package com.lingxia.lxapp

import com.lingxia.lxapp.LxAppActivityStartGate.Decision
import com.lingxia.lxapp.LxAppActivityStartGate.Open
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LxAppActivityStartGateTest {
    private var now = 1_000L
    private val gate = LxAppActivityStartGate(staleAfterMs = 5_000L, clock = { now })

    private val home = Open("home", "pages/home/index", 1L)

    @Test
    fun theFirstOpenStartsTheActivity() {
        assertEquals(Decision.START, gate.request(home))
    }

    @Test
    fun theSameOpenWhileStartingDoesNotStartASecondActivity() {
        // Cold start through an App Link: home, then the link's target resolving to home.
        gate.request(home)
        assertEquals(Decision.COVERED, gate.request(home.copy()))
        assertTrue(gate.activityReady().isEmpty())
    }

    @Test
    fun aDifferentOpenWhileStartingIsHandedBackOnceTheActivityExists() {
        gate.request(home)
        val detail = Open("home", "pages/detail/index", 1L)
        val shop = Open("shop", "pages/index", 2L)
        assertEquals(Decision.QUEUED, gate.request(detail))
        assertEquals(Decision.QUEUED, gate.request(shop))
        assertEquals(listOf(detail, shop), gate.activityReady())
    }

    @Test
    fun onlyTheLatestQueuedOpenOfAnAppIsKept() {
        gate.request(home)
        gate.request(Open("shop", "pages/a", 2L))
        val latest = Open("shop", "pages/b", 2L)
        gate.request(latest)
        assertEquals(listOf(latest), gate.activityReady())
    }

    @Test
    fun onceTheActivityExistsTheNextMissStartsAgain() {
        gate.request(home)
        gate.activityReady()
        assertEquals(Decision.START, gate.request(home))
    }

    @Test
    fun aStartThatNeverArrivedStopsHoldingOpens() {
        gate.request(home)
        now += 5_001L
        val detail = Open("home", "pages/detail/index", 1L)
        assertEquals(Decision.START, gate.request(detail))
        assertTrue(gate.activityReady().isEmpty())
    }
}
