package com.lingxia.app

/** Snapshot of the currently active LxApp on the Rust stack. */
data class CurrentLxApp(
    val appId: String,
    val path: String,
    val sessionId: Long,
) {
    fun isValid(): Boolean = appId.isNotEmpty() && sessionId > 0L
    fun isEmpty(): Boolean = appId.isEmpty()
}
