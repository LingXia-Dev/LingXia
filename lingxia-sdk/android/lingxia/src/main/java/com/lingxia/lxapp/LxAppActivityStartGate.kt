package com.lingxia.lxapp

/**
 * Opens that reach [LxApp] while an LxAppActivity has been started but not yet
 * created.
 *
 * `startActivity` returns before the activity exists, so every open in that
 * window also finds no current activity. A cold start through an App Link is
 * the common case: the bootstrap opens home and the runtime opens the link's
 * target a few milliseconds later, and each started its own LxAppActivity —
 * two stacked in the task, the lower one a stale copy Back would reveal.
 *
 * Main thread only, like the opens it gates.
 */
internal class LxAppActivityStartGate(
    private val staleAfterMs: Long = STALE_AFTER_MS,
    private val clock: () -> Long = { android.os.SystemClock.uptimeMillis() },
) {
    data class Open(val appId: String, val path: String, val sessionId: Long)

    enum class Decision {
        /** No start is in flight: start the activity for this open. */
        START,

        /** The activity being started performs exactly this open already. */
        COVERED,

        /** Held until the activity exists; [activityReady] hands it back. */
        QUEUED,
    }

    private var starting: Open? = null
    private var startedAt = 0L
    private val queued = ArrayList<Open>()

    /** An open found no current activity. */
    fun request(open: Open): Decision {
        val inFlight = starting
        // A start whose activity never arrived (it finished in onCreate, the
        // system dropped it) must not swallow every open after it.
        if (inFlight == null || clock() - startedAt > staleAfterMs) {
            starting = open
            startedAt = clock()
            queued.clear()
            return Decision.START
        }
        if (open == inFlight) return Decision.COVERED
        // Only the latest open of an app matters once the activity can take it.
        queued.removeAll { it.appId == open.appId }
        queued.add(open)
        return Decision.QUEUED
    }

    /** The started activity exists. Returns the opens it still has to perform, in order. */
    fun activityReady(): List<Open> {
        starting = null
        val waiting = queued.toList()
        queued.clear()
        return waiting
    }

    companion object {
        /** Longer than any activity start the user would still be waiting on. */
        const val STALE_AFTER_MS = 5_000L
    }
}
