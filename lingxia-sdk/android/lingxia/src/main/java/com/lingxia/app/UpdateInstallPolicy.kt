package com.lingxia.app

import android.content.pm.PackageInstaller
import android.os.Build
import com.lingxia.lxapp.R

internal fun useLegacyUpdateInstaller(sdk: Int, manufacturer: String, model: String, isTv: Boolean): Boolean =
    // Mi TV's Android 5.x package manager rejects ordinary-app sessions before
    // presenting consent. Its privileged ACTION_VIEW installer owns that consent.
    sdk <= Build.VERSION_CODES.LOLLIPOP_MR1 &&
        manufacturer.equals("xiaomi", ignoreCase = true) &&
        // Older MIUI TV firmware can omit both TV uiMode and Leanback features.
        (isTv || model.startsWith("MiTV", ignoreCase = true))

internal fun updateInstallErrorResource(status: Int, message: String): Int {
    // Android also maps USER_RESTRICTED to INCOMPATIBLE (7). Preserve the
    // specific cause instead of telling users their APK/CPU is incompatible.
    if (message.substringBefore(':').trim() == "INSTALL_FAILED_USER_RESTRICTED") {
        return R.string.lx_update_install_blocked
    }
    return when (status) {
        PackageInstaller.STATUS_FAILURE_ABORTED -> R.string.lx_update_install_aborted
        PackageInstaller.STATUS_FAILURE_BLOCKED -> R.string.lx_update_install_blocked
        PackageInstaller.STATUS_FAILURE_CONFLICT -> R.string.lx_update_install_conflict
        PackageInstaller.STATUS_FAILURE_INCOMPATIBLE -> R.string.lx_update_install_incompatible
        PackageInstaller.STATUS_FAILURE_INVALID -> R.string.lx_update_install_invalid
        PackageInstaller.STATUS_FAILURE_STORAGE -> R.string.lx_update_install_storage
        else -> R.string.lx_update_install_failure
    }
}
