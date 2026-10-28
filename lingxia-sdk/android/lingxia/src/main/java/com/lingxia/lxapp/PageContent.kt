package com.lingxia.lxapp

import android.view.View

internal fun View.getAppId(): String? = when (this) {
    is WebView -> getAppId()
    is GpuPage -> getAppId()
    else -> null
}

internal fun View.getCurrentPath(): String? = when (this) {
    is WebView -> getCurrentPath()
    is GpuPage -> getCurrentPath()
    else -> null
}

internal fun View.getWebTag(): String? = when (this) {
    is WebView -> getWebTag()
    is GpuPage -> getWebTag()
    else -> null
}

/**
 * The page instance a lifecycle callback names: `path#instance#session` from the page's tag,
 * so a re-entered route resolves to this instance and not one being torn down.
 */
internal fun View.pageIdentity(): String? {
    val appId = getAppId() ?: return null
    val tag = getWebTag()
    val identity = tag?.removePrefix("$appId:")
    return if (identity != null && identity != tag) identity else getCurrentPath()
}

internal fun View.resume() {
    when (this) { is WebView -> resume(); is GpuPage -> resume() }
}

internal fun View.pause() {
    when (this) { is WebView -> pause(); is GpuPage -> pause() }
}
