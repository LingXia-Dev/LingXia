package com.lingxia.lxapp

import org.json.JSONObject

internal data class BrowserAddressSubmissionResult(
    val url: String,
    val displayText: String
)

internal fun handleBrowserAddressSubmission(raw: String?, currentUrl: String?): BrowserAddressSubmissionResult? {
    val trimmed = raw?.trim().orEmpty()
    if (trimmed.isEmpty()) {
        return null
    }

    val request = JSONObject().apply {
        put("raw_input", trimmed)
        put("trigger", "submit")
        put("context", JSONObject().apply {
            put("current_url", currentUrl)
            put("allow_search_fallback", false)
        })
    }

    val responseJson = try {
        NativeApi.handleBrowserAddressInput(request.toString())
    } catch (_: Throwable) {
        null
    } ?: return null

    return try {
        val response = JSONObject(responseJson)
        if (response.optString("action") != "navigate") {
            return null
        }
        val navigation = response.optJSONObject("navigation") ?: return null
        val state = response.optJSONObject("state")
        val url = navigation.optString("url")
        if (url.isBlank()) {
            null
        } else {
            BrowserAddressSubmissionResult(
                url = url,
                displayText = state?.optString("display_text").takeUnless { it.isNullOrBlank() } ?: url
            )
        }
    } catch (_: Throwable) {
        null
    }
}
