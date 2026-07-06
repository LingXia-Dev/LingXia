package com.lingxia.app

/**
 * Routes SDK-native logs into the LingXia Rust log pipeline.
 *
 * Unlike [android.util.Log], records emitted here flow through the same pipeline
 * as Rust logs: they reach logcat *and* the dev-server stream, so they show up
 * in `lxdev logs` tagged with the originating [appId]/[path].
 *
 * **Forward errors and important warnings only.** Records here are also buffered
 * for cloud upload / crash diagnosis, so routing routine info/debug dilutes that
 * bounded buffer (evicting the errors it's meant to keep) and pays an FFI
 * crossing per call. Keep lifecycle/info/debug and high-frequency traces on
 * `android.util.Log`; send through `LxLog` the diagnostics you'd want in an
 * uploaded log bundle. On a hot path, guard with `LxLog.isEnabled(level)`.
 *
 * Method names and primary argument order follow `android.util.Log`; optional
 * appId/path metadata routes records to the owning lxapp in dev logs.
 */
internal object LxLog {
    // Mirrors the Rust FFI level contract (see `logging::forward_host_log`).
    private const val VERBOSE = 0
    private const val DEBUG = 1
    private const val INFO = 2
    private const val WARN = 3
    private const val ERROR = 4

    /** Whether a log at [level] (0=verbose … 4=error) would be recorded. Guard an
     *  expensive hot-path log with this to skip building the message. */
    fun isEnabled(level: Int): Boolean = NativeApi.hostLogEnabled(level)

    fun v(tag: String, message: String, appId: String = "", path: String = "") =
        NativeApi.forwardHostLog(VERBOSE, tag, appId, path, message)

    fun d(tag: String, message: String, appId: String = "", path: String = "") =
        NativeApi.forwardHostLog(DEBUG, tag, appId, path, message)

    fun i(tag: String, message: String, appId: String = "", path: String = "") =
        NativeApi.forwardHostLog(INFO, tag, appId, path, message)

    fun w(tag: String, message: String, tr: Throwable? = null, appId: String = "", path: String = "") =
        NativeApi.forwardHostLog(WARN, tag, appId, path, message.withThrowable(tr))

    fun e(tag: String, message: String, tr: Throwable? = null, appId: String = "", path: String = "") =
        NativeApi.forwardHostLog(ERROR, tag, appId, path, message.withThrowable(tr))

    private fun String.withThrowable(tr: Throwable?): String =
        if (tr == null) this else "$this\n${tr.stackTraceToString()}"
}
