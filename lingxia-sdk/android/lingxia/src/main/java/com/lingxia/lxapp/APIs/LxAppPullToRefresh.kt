package com.lingxia.lxapp.APIs

import android.util.Log
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity

/**
 * Pull-to-refresh API for LxApp
 *
 * Provides programmatic control over pull-to-refresh functionality.
 */
internal object LxAppPullToRefresh {
    private const val TAG = "LxAppPullToRefresh"

    /**
     * Runs [block] on the UI thread when the page instance [webtag] names is
     * the one on screen. The activity has a single indicator, so a request
     * for any other instance, another one of the same route included, is
     * dropped.
     */
    private fun onPresentedPage(action: String, appId: String, webtag: String, block: (LxAppActivity) -> Unit) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null || activity.appId != appId) {
            LxLog.w(TAG, "$action ignored: no active activity for $appId", appId = appId)
            return
        }
        activity.runOnUiThread {
            val presented = activity.getCurrentWebView()?.getWebTag()
            if (presented != webtag) {
                Log.d(TAG, "$action skipped: $webtag is not the presented page ($presented)")
                return@runOnUiThread
            }
            block(activity)
        }
    }

    /** Runtime bridge entry (JNI): show the indicator of the page instance [webtag] names. */
    @JvmStatic
    fun startPullDownRefresh(appId: String, webtag: String) {
        onPresentedPage("startPullDownRefresh", appId, webtag) { activity ->
            activity.pullToRefreshHelper?.let { helper ->
                if (helper.isEnabled()) {
                    helper.startRefreshing()
                } else {
                    Log.d(TAG, "startPullDownRefresh skipped: disabled for $appId")
                }
            } ?: LxLog.w(TAG, "startPullDownRefresh ignored: helper not initialized", appId = appId)
        }
    }

    /** Runtime bridge entry (JNI): hide the indicator of the page instance [webtag] names. */
    @JvmStatic
    fun stopPullDownRefresh(appId: String, webtag: String) {
        onPresentedPage("stopPullDownRefresh", appId, webtag) { activity ->
            activity.pullToRefreshHelper?.endRefreshing()
        }
    }
}
