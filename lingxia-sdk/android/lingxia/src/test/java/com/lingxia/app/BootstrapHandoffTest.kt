package com.lingxia.app

import org.junit.Assert.assertEquals
import org.junit.Test

class BootstrapHandoffTest {
    @Test
    fun coldStartKeepsTheBootstrap() {
        assertEquals(BootstrapHandoff.STAY, bootstrapHandoff(isTaskRoot = true, taskId = 7, liveAppTaskId = null))
    }

    @Test
    fun coldStartInsideAnotherTaskStillKeepsTheBootstrap() {
        assertEquals(BootstrapHandoff.STAY, bootstrapHandoff(isTaskRoot = false, taskId = 7, liveAppTaskId = null))
    }

    @Test
    fun theRootUnderTheLiveAppStays() {
        assertEquals(BootstrapHandoff.STAY, bootstrapHandoff(isTaskRoot = true, taskId = 7, liveAppTaskId = 7))
    }

    @Test
    fun aSecondBootstrapOverTheLiveAppFinishes() {
        // Launcher tap or a link that arrives while the app runs, same task.
        assertEquals(BootstrapHandoff.FINISH, bootstrapHandoff(isTaskRoot = false, taskId = 7, liveAppTaskId = 7))
    }

    @Test
    fun aBootstrapInAnotherTaskRaisesTheAppAndFinishes() {
        assertEquals(
            BootstrapHandoff.RAISE_APP_TASK_AND_FINISH,
            bootstrapHandoff(isTaskRoot = false, taskId = 9, liveAppTaskId = 7),
        )
        assertEquals(
            BootstrapHandoff.RAISE_APP_TASK_AND_FINISH,
            bootstrapHandoff(isTaskRoot = true, taskId = 9, liveAppTaskId = 7),
        )
    }
}
