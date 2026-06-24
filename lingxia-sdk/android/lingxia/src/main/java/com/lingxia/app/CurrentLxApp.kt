package com.lingxia.app

/**
 * Snapshot of the currently active LxApp on the Rust stack — the host's
 * read-only view of "which tenant is foreground", produced by
 * [NativeApi.getCurrentLxApp] and consumed by both host (`com.lingxia.app`)
 * and tenant (`com.lingxia.lxapp`) code.
 *
 * Like [NativeApi], its package is pinned by the JNI ABI: the Rust side
 * constructs it via `find_class("com/lingxia/app/CurrentLxApp")`
 * (`crates/lingxia/src/ffi/android.rs`), so the package is part of the native
 * contract and must not move without a coordinated Rust change.
 */
data class CurrentLxApp(
    val appId: String,
    val path: String,
    val sessionId: Long,
) {
    fun isValid(): Boolean = appId.isNotEmpty() && sessionId > 0L
    fun isEmpty(): Boolean = appId.isEmpty()
}
