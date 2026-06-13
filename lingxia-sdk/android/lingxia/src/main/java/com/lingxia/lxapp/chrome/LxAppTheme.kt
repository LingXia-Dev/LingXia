package com.lingxia.lxapp.chrome

internal object LxAppTheme {

    object Metrics {
        const val CAPSULE_BASE_TOP_DP = 52f

        const val CAPSULE_MIN_MARGIN_FROM_STATUSBAR_DP = 4f

        const val CAPSULE_HEIGHT_DP = 36f
        const val CAPSULE_BUTTON_WIDTH_DP = 44f
        const val CAPSULE_DIVIDER_WIDTH_DP = 0.5f
        const val CAPSULE_DIVIDER_HEIGHT_DP = 20f
        const val CAPSULE_PADDING_HORIZONTAL_DP = 2f

        const val CAPSULE_TRAILING_MARGIN_DP = 12f

        fun calculateCapsuleTopMargin(statusBarHeightPx: Int, density: Float): Int {
            val statusBarHeightDp = statusBarHeightPx / density

            var topDp = CAPSULE_BASE_TOP_DP

            val minSafeTop = statusBarHeightDp + CAPSULE_MIN_MARGIN_FROM_STATUSBAR_DP
            if (minSafeTop > topDp) {
                topDp = minSafeTop
            }

            return (topDp * density).toInt()
        }

        fun calculateCapsuleTopDp(statusBarHeightPx: Int, density: Float): Float {
            val statusBarHeightDp = statusBarHeightPx / density

            var topDp = CAPSULE_BASE_TOP_DP

            val minSafeTop = statusBarHeightDp + CAPSULE_MIN_MARGIN_FROM_STATUSBAR_DP
            if (minSafeTop > topDp) {
                topDp = minSafeTop
            }

            return topDp
        }
    }
}
