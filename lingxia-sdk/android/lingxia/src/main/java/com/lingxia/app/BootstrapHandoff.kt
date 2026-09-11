package com.lingxia.app

/**
 * What a bootstrap (launcher) activity does once it has asked for home.
 *
 * The bootstrap is the host app's launcher activity: [Lingxia.quickStart]
 * runs in it, and on a cold start it stays at the root of the task under
 * LxAppActivity. Any later creation of it — a launcher tap on a task the
 * system does not simply bring forward, a link that arrives while the app
 * is running — lands on top of the live app with no content of its own.
 */
internal enum class BootstrapHandoff {
    /** Cold start, or the root the live app sits on: keep it. */
    STAY,

    /** A second bootstrap over the live app in the same task: reveal the app. */
    FINISH,

    /**
     * A bootstrap in some other task — another app's, when a link opens without
     * a new task: raise the live app's task so the user sees where the link
     * went, then finish.
     */
    RAISE_APP_TASK_AND_FINISH,
}

/**
 * @param isTaskRoot whether the bootstrap is the root of its task.
 * @param taskId the bootstrap's task.
 * @param liveAppTaskId the task of the LxAppActivity that existed before this
 *   bootstrap asked for home, or null when there was none (a cold start).
 */
internal fun bootstrapHandoff(isTaskRoot: Boolean, taskId: Int, liveAppTaskId: Int?): BootstrapHandoff =
    when {
        liveAppTaskId == null -> BootstrapHandoff.STAY
        liveAppTaskId != taskId -> BootstrapHandoff.RAISE_APP_TASK_AND_FINISH
        isTaskRoot -> BootstrapHandoff.STAY
        else -> BootstrapHandoff.FINISH
    }
