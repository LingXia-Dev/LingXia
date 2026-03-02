package com.lingxia.lxapp.APIs.media

import android.graphics.Color
import android.os.Build
import android.view.View
import android.view.Window
import android.view.WindowManager
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

object ImmersiveWindowUi {
    data class Snapshot(
        val systemUiVisibility: Int,
        val windowFlags: Int,
        val decorFitsSystemWindows: Boolean,
        val statusBarColor: Int,
        val navigationBarColor: Int,
        val navigationBarContrastEnforced: Boolean?,
        val cutoutMode: Int?,
    )

    fun capture(window: Window): Snapshot {
        return Snapshot(
            systemUiVisibility = window.decorView.systemUiVisibility,
            windowFlags = window.attributes.flags,
            decorFitsSystemWindows = ViewCompat.getFitsSystemWindows(window.decorView),
            statusBarColor = window.statusBarColor,
            navigationBarColor = window.navigationBarColor,
            navigationBarContrastEnforced = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                window.isNavigationBarContrastEnforced
            } else {
                null
            },
            cutoutMode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                window.attributes.layoutInDisplayCutoutMode
            } else {
                null
            }
        )
    }

    fun apply(window: Window, keepScreenOn: Boolean) {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        @Suppress("DEPRECATION")
        run {
            window.decorView.systemUiVisibility = (
                View.SYSTEM_UI_FLAG_FULLSCREEN or
                    View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
                    View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                    View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                )
        }
        window.clearFlags(WindowManager.LayoutParams.FLAG_FORCE_NOT_FULLSCREEN)
        @Suppress("DEPRECATION")
        window.setFlags(
            WindowManager.LayoutParams.FLAG_FULLSCREEN,
            WindowManager.LayoutParams.FLAG_FULLSCREEN
        )
        var addFlags = WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS or
            WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
        if (keepScreenOn) {
            addFlags = addFlags or WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
        }
        window.addFlags(addFlags)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.isNavigationBarContrastEnforced = false
        }

        WindowCompat.getInsetsController(window, window.decorView)?.apply {
            hide(WindowInsetsCompat.Type.statusBars())
            hide(WindowInsetsCompat.Type.navigationBars())
            hide(WindowInsetsCompat.Type.displayCutout())
            systemBarsBehavior = androidx.core.view.WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.insetsController?.hide(
                android.view.WindowInsets.Type.statusBars() or
                    android.view.WindowInsets.Type.navigationBars() or
                    android.view.WindowInsets.Type.displayCutout()
            )
        }
        window.decorView.post {
            WindowCompat.getInsetsController(window, window.decorView)?.hide(WindowInsetsCompat.Type.systemBars())
        }
    }

    fun restore(window: Window, snapshot: Snapshot) {
        @Suppress("DEPRECATION")
        run {
            window.decorView.systemUiVisibility = snapshot.systemUiVisibility
        }
        window.attributes = window.attributes.apply {
            flags = snapshot.windowFlags
        }
        WindowCompat.setDecorFitsSystemWindows(window, snapshot.decorFitsSystemWindows)
        window.statusBarColor = snapshot.statusBarColor
        window.navigationBarColor = snapshot.navigationBarColor
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            snapshot.navigationBarContrastEnforced?.let { window.isNavigationBarContrastEnforced = it }
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            snapshot.cutoutMode?.let { mode ->
                window.attributes = window.attributes.apply { layoutInDisplayCutoutMode = mode }
            }
        }

        val wasStatusHidden = (snapshot.systemUiVisibility and View.SYSTEM_UI_FLAG_FULLSCREEN) != 0
        val wasNavHidden = (snapshot.systemUiVisibility and View.SYSTEM_UI_FLAG_HIDE_NAVIGATION) != 0
        WindowCompat.getInsetsController(window, window.decorView)?.let { controller ->
            if (wasStatusHidden || wasNavHidden) {
                controller.hide(WindowInsetsCompat.Type.systemBars())
            } else {
                controller.show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }
}
