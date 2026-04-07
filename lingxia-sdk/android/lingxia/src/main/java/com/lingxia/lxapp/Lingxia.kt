package com.lingxia.lxapp

import android.content.Context

/**
 * Top-level entry point for the LingXia SDK.
 *
 * For most apps, initialization is handled automatically by extending [LxAppLaunchActivity].
 * Call [initialize] directly only if you manage the Activity lifecycle yourself.
 */
object Lingxia {

    /**
     * Initialize the LingXia SDK.
     *
     * Idempotent — safe to call multiple times. [LxAppLaunchActivity] calls this automatically;
     * you only need to call it directly when not using [LxAppLaunchActivity].
     */
    @JvmStatic
    fun initialize(context: Context) {
        LxApp.initialize(context)
    }
}
