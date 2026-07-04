package com.lingxia.app

/**
 * Routes SDK-native logs into the LingXia Rust log pipeline.
 *
 * Unlike [android.util.Log], records emitted here flow through the same pipeline
 * as Rust logs: they reach logcat *and* the dev-server stream, so they show up
 * in `lxdev logs` tagged with the originating [appId]/[path].
 *
 * Prefer this over `android.util.Log` for any log a host/lxapp developer should
 * be able to observe. Pure platform / high-frequency traces may stay on `Log`.
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
