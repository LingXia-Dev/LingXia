package com.lingxia.lxapp.APIs

import android.util.Log
import com.lingxia.app.LxApp

/**
 * Pull-to-refresh API for LxApp
 *
 * Provides programmatic control over pull-to-refresh functionality.
 */
internal object LxAppPullToRefresh {
    private const val TAG = "LxAppPullToRefresh"
    private fun normalizePath(raw: String?) = raw?.substringBefore('?')?.substringBefore('#') ?: ""

    /**
     * Start pull-to-refresh animation programmatically
     *
     * @param appId The ID of the app
     * @param path The page path (optional, defaults to current page)
     */
    @JvmStatic
    @JvmOverloads
    fun startPullDownRefresh(appId: String, path: String = "") {
        val activity = LxApp.getCurrentActivity()
        if (activity == null || activity.appId != appId) {
            Log.w(TAG, "startPullDownRefresh ignored: no active activity for $appId")
            return
        }

        activity.runOnUiThread {
            val currentPath = normalizePath(activity.getCurrentWebView()?.getCurrentPath())
            val targetPath = normalizePath(path)
            if (targetPath.isNotEmpty() && currentPath.isNotEmpty() && currentPath != targetPath) {
                Log.d(TAG, "startPullDownRefresh skipped: path mismatch ($currentPath != $targetPath)")
                return@runOnUiThread
            }

            activity.pullToRefreshHelper?.let { helper ->
                if (helper.isEnabled()) {
                    helper.startRefreshing()
                } else {
                    Log.d(TAG, "startPullDownRefresh skipped: disabled for $appId")
                }
            } ?: Log.w(TAG, "startPullDownRefresh ignored: helper not initialized")
        }
    }

    /**
     * Stop pull-to-refresh animation
     *
     * @param appId The ID of the app
     * @param path The page path (optional)
     */
    @JvmStatic
    @JvmOverloads
    fun stopPullDownRefresh(appId: String, path: String = "") {
        val activity = LxApp.getCurrentActivity()
        if (activity == null || activity.appId != appId) {
            Log.w(TAG, "stopPullDownRefresh ignored: no active activity for $appId")
            return
        }

        activity.runOnUiThread {
            val currentPath = normalizePath(activity.getCurrentWebView()?.getCurrentPath())
            val targetPath = normalizePath(path)
            if (targetPath.isNotEmpty() && currentPath.isNotEmpty() && currentPath != targetPath) {
                Log.d(TAG, "stopPullDownRefresh skipped: path mismatch ($currentPath != $targetPath)")
                return@runOnUiThread
            }
            activity.pullToRefreshHelper?.endRefreshing()
        }
    }
}
