package com.lingxia.lxapp.chrome

import android.content.Context
import android.content.res.Configuration
import android.graphics.Color

/**
 * Colors for chrome that belongs to no single page — modals and action sheets.
 * Resolved per presentation from the activity's ui mode: an lxapp that pinned
 * its own scheme sets the activity's night mode, so this follows the lxapp
 * first and the system second.
 */
internal data class OverlayPalette(
    val scrim: Int,
    val surface: Int,
    val title: Int,
    val body: Int,
    val secondaryFill: Int,
    val secondaryText: Int,
    val separator: Int,
    val gap: Int
) {
    companion object {
        fun of(context: Context): OverlayPalette {
            val dark = (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
                Configuration.UI_MODE_NIGHT_YES
            return if (dark) {
                OverlayPalette(
                    scrim = Color.parseColor("#A6000000"),
                    surface = Color.parseColor("#1C1C1E"),
                    title = Color.WHITE,
                    body = Color.parseColor("#A1A1AA"),
                    secondaryFill = Color.parseColor("#2C2C2E"),
                    secondaryText = Color.parseColor("#E5E5E7"),
                    separator = Color.parseColor("#3A3A3C"),
                    gap = Color.parseColor("#000000")
                )
            } else {
                OverlayPalette(
                    scrim = Color.parseColor("#80000000"),
                    surface = Color.WHITE,
                    title = Color.BLACK,
                    body = Color.parseColor("#666666"),
                    secondaryFill = Color.parseColor("#F5F5F5"),
                    secondaryText = Color.parseColor("#666666"),
                    separator = Color.parseColor("#E0E0E0"),
                    gap = Color.parseColor("#E0E0E0")
                )
            }
        }
    }
}
