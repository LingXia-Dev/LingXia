package com.lingxia.lxapp.APIs.media

import android.net.Uri

/**
 * Shared URI helpers for the media subsystem. Kept tiny on purpose — these
 * predicates are used in hot paths (per-frame / per-page) so duplication
 * would creep back in if the helper had any non-trivial dependencies.
 */

/**
 * `true` for URIs whose bytes live on the device — file paths, content
 * provider entries, and bare paths. Network schemes (`http(s)`, `lx://`,
 * etc.) return false. Used by playlist / preview to decide whether
 * first-frame extraction (LocalVideoFrameCache) is worthwhile.
 */
internal fun isLocalUri(uri: Uri): Boolean {
    if (uri == Uri.EMPTY) return false
    val scheme = uri.scheme
    return scheme.isNullOrEmpty() ||
        scheme.equals("file", ignoreCase = true) ||
        scheme.equals("content", ignoreCase = true)
}
