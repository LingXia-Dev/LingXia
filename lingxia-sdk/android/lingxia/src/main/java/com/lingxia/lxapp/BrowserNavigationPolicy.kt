package com.lingxia.lxapp

import com.lingxia.app.NativeApi

import org.json.JSONObject

internal enum class BrowserNavigationDecision {
    IN_WEBVIEW,
    OPEN_EXTERNAL,
    DENY;

    companion object {
        fun fromRaw(raw: String?): BrowserNavigationDecision? {
            return when (raw) {
                "in_webview" -> IN_WEBVIEW
                "open_external" -> OPEN_EXTERNAL
                "deny" -> DENY
                else -> null
            }
        }
    }
}

internal data class BrowserNavigationPolicyResult(
    val decision: BrowserNavigationDecision,
)

internal fun resolveBrowserNavigationPolicy(
    rawUrl: String?,
    hasUserGesture: Boolean,
    isMainFrame: Boolean = true
): BrowserNavigationPolicyResult? {
    val trimmed = rawUrl?.trim().orEmpty()
    if (trimmed.isEmpty()) {
        return null
    }

    val request = JSONObject().apply {
        put("raw_url", trimmed)
        put("has_user_gesture", hasUserGesture)
        put("is_main_frame", isMainFrame)
    }

    val responseJson = try {
        NativeApi.handleBrowserNavigationPolicy(request.toString())
    } catch (_: Throwable) {
        null
    } ?: return null

    return try {
        val response = JSONObject(responseJson)
        val decision = BrowserNavigationDecision.fromRaw(response.optString("decision"))
            ?: return null
        BrowserNavigationPolicyResult(decision = decision)
    } catch (_: Throwable) {
        null
    }
}
