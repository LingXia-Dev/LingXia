package com.lingxia.lxapp.chrome

internal object LxAppTheme {

    object Metrics {
        const val CAPSULE_HEIGHT_DP = 36f
        const val CAPSULE_BUTTON_WIDTH_DP = 44f
        const val CAPSULE_DIVIDER_WIDTH_DP = 0.5f
        const val CAPSULE_DIVIDER_HEIGHT_DP = 20f
        const val CAPSULE_PADDING_HORIZONTAL_DP = 2f

        const val CAPSULE_TRAILING_MARGIN_DP = 12f

        // Keep in sync with LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP.
        const val NAV_BAR_CONTENT_HEIGHT_DP = 44f

        // Capsule centerY matches the navbar title centerY:
        // top = statusBar + (navBarContentHeight - capsuleHeight) / 2
        fun calculateCapsuleTopDp(statusBarHeightPx: Int, density: Float): Float {
            val statusBarHeightDp = statusBarHeightPx / density
            return statusBarHeightDp + (NAV_BAR_CONTENT_HEIGHT_DP - CAPSULE_HEIGHT_DP) / 2f
        }

        fun calculateCapsuleTopMargin(statusBarHeightPx: Int, density: Float): Int {
            return (calculateCapsuleTopDp(statusBarHeightPx, density) * density).toInt()
        }
    }
}
