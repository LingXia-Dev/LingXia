package com.lingxia.app

import android.content.pm.PackageInstaller
import com.lingxia.lxapp.R

/** Leading failure code of an install-status message, e.g. `INSTALL_FAILED_USER_RESTRICTED`. */
internal fun installFailureCode(message: String): String = message.substringBefore(':').trim()

/**
 * True when the package manager refused the session itself. Such ROMs still
 * let commit() succeed, so the status broadcast is the only place the refusal
 * shows up — and the only signal that this device needs the installer UI.
 */
internal fun isSessionInstallRestricted(message: String): Boolean =
    installFailureCode(message) == "INSTALL_FAILED_USER_RESTRICTED"

/** What to do with a session that reported failure. */
internal enum class InstallFailureAction { NONE, RETRY_PROMPT, LEGACY_INSTALLER }

internal fun installFailureAction(
    status: Int,
    message: String,
    hasUpdate: Boolean
): InstallFailureAction = when {
    !hasUpdate -> InstallFailureAction.NONE
    isSessionInstallRestricted(message) -> InstallFailureAction.LEGACY_INSTALLER
    // The user declined the install; re-offering it would fight them.
    status == PackageInstaller.STATUS_FAILURE_ABORTED -> InstallFailureAction.NONE
    else -> InstallFailureAction.RETRY_PROMPT
}

internal fun updateInstallErrorResource(status: Int, message: String): Int {
    // Android also maps USER_RESTRICTED to INCOMPATIBLE (7). Preserve the
    // specific cause instead of telling users their APK/CPU is incompatible.
    if (isSessionInstallRestricted(message)) {
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
