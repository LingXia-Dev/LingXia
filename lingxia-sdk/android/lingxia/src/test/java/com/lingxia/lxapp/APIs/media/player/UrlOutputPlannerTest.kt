package com.lingxia.lxapp.APIs.media.player

import com.lingxia.app.media.UrlPlayerOutputKind
import com.lingxia.app.media.UrlPlayerSurfaceKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class UrlOutputPlannerTest {
    @Test
    fun previewTextureViewIsNotEager() {
        val resolved = UrlOutputPlanner.resolve(
            UrlPlayerSurfaceKind.PREVIEW,
            UrlPlayerOutputKind.TEXTURE_VIEW,
            sdkInt = 30,
        )
        assertEquals(UrlPlayerOutputKind.TEXTURE_VIEW, resolved.kind)
        assertNull(resolved.forceReason)
        assertFalse(UrlOutputPlanner.shouldEagerCreate(resolved.kind))
    }

    @Test
    fun previewSurfaceViewOnApi24IsEager() {
        val resolved = UrlOutputPlanner.resolve(
            UrlPlayerSurfaceKind.PREVIEW,
            UrlPlayerOutputKind.SURFACE_VIEW,
            sdkInt = 24,
        )
        assertEquals(UrlPlayerOutputKind.SURFACE_VIEW, resolved.kind)
        assertNull(resolved.forceReason)
        assertTrue(UrlOutputPlanner.shouldEagerCreate(resolved.kind))
    }

    @Test
    fun previewSurfaceViewBelowApi24ForcesTextureView() {
        val resolved = UrlOutputPlanner.resolve(
            UrlPlayerSurfaceKind.PREVIEW,
            UrlPlayerOutputKind.SURFACE_VIEW,
            sdkInt = 21,
        )
        assertEquals(UrlPlayerOutputKind.TEXTURE_VIEW, resolved.kind)
        assertEquals("api-lt-24", resolved.forceReason)
        assertFalse(UrlOutputPlanner.shouldEagerCreate(resolved.kind))
    }

    @Test
    fun inlineSurfaceViewForcesTextureView() {
        val resolved = UrlOutputPlanner.resolve(
            UrlPlayerSurfaceKind.INLINE,
            UrlPlayerOutputKind.SURFACE_VIEW,
            sdkInt = 30,
        )
        assertEquals(UrlPlayerOutputKind.TEXTURE_VIEW, resolved.kind)
        assertEquals("inline-surface-view", resolved.forceReason)
    }
}
