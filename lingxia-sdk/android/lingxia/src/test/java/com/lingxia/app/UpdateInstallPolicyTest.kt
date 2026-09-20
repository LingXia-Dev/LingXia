package com.lingxia.app

import android.content.pm.PackageInstaller
import com.lingxia.lxapp.R
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class UpdateInstallPolicyTest {
    private val restricted = "INSTALL_FAILED_USER_RESTRICTED: Install canceled by user"

    @Test
    fun failureCodeDropsTheDetailAndPadding() {
        assertEquals("INSTALL_FAILED_USER_RESTRICTED", installFailureCode(restricted))
        assertEquals("INSTALL_FAILED_INVALID_APK", installFailureCode("  INSTALL_FAILED_INVALID_APK "))
        assertEquals("", installFailureCode(""))
    }

    @Test
    fun onlyUserRestrictedCountsAsSessionRestricted() {
        assertTrue(isSessionInstallRestricted(restricted))
        assertTrue(isSessionInstallRestricted("INSTALL_FAILED_USER_RESTRICTED"))
        assertFalse(isSessionInstallRestricted("INSTALL_FAILED_VERSION_DOWNGRADE"))
        assertFalse(isSessionInstallRestricted(""))
    }

    @Test
    fun userRestrictedOverridesTheIncompatibleStatus() {
        assertEquals(
            R.string.lx_update_install_blocked,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_INCOMPATIBLE, restricted)
        )
    }

    @Test
    fun eachFailureStatusHasItsOwnMessage() {
        assertEquals(
            R.string.lx_update_install_aborted,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_ABORTED, "")
        )
        assertEquals(
            R.string.lx_update_install_blocked,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_BLOCKED, "")
        )
        assertEquals(
            R.string.lx_update_install_conflict,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_CONFLICT, "")
        )
        assertEquals(
            R.string.lx_update_install_incompatible,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_INCOMPATIBLE, "")
        )
        assertEquals(
            R.string.lx_update_install_invalid,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_INVALID, "")
        )
        assertEquals(
            R.string.lx_update_install_storage,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE_STORAGE, "")
        )
    }

    @Test
    fun unknownStatusFallsBackToTheGenericFailure() {
        assertEquals(
            R.string.lx_update_install_failure,
            updateInstallErrorResource(PackageInstaller.STATUS_FAILURE, "")
        )
        assertEquals(R.string.lx_update_install_failure, updateInstallErrorResource(-1, ""))
    }

    @Test
    fun aForgottenSessionIsNotActedOn() {
        assertEquals(
            InstallFailureAction.NONE,
            installFailureAction(PackageInstaller.STATUS_FAILURE, restricted, hasUpdate = false)
        )
        assertEquals(
            InstallFailureAction.NONE,
            installFailureAction(PackageInstaller.STATUS_FAILURE, "", hasUpdate = false)
        )
    }

    @Test
    fun userRestrictedSendsTheUpdateToTheSystemInstaller() {
        assertEquals(
            InstallFailureAction.LEGACY_INSTALLER,
            installFailureAction(
                PackageInstaller.STATUS_FAILURE_INCOMPATIBLE,
                restricted,
                hasUpdate = true
            )
        )
    }

    @Test
    fun anAbortedInstallIsNotReOffered() {
        assertEquals(
            InstallFailureAction.NONE,
            installFailureAction(PackageInstaller.STATUS_FAILURE_ABORTED, "", hasUpdate = true)
        )
    }

    @Test
    fun otherFailuresReOfferThePrompt() {
        assertEquals(
            InstallFailureAction.RETRY_PROMPT,
            installFailureAction(PackageInstaller.STATUS_FAILURE_STORAGE, "", hasUpdate = true)
        )
        assertEquals(
            InstallFailureAction.RETRY_PROMPT,
            installFailureAction(
                PackageInstaller.STATUS_FAILURE_CONFLICT,
                "INSTALL_FAILED_VERSION_DOWNGRADE",
                hasUpdate = true
            )
        )
    }
}
