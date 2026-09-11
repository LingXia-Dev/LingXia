package com.lingxia.lxapp.APIs.media.player

import com.lingxia.app.media.UrlPlayerErrorCode
import org.junit.Assert.assertEquals
import org.junit.Test

class UrlPlayerErrorCodeTest {
    @Test
    fun publicCodesMatchInternalValues() {
        assertEquals(ErrorCode.ABORTED.value, UrlPlayerErrorCode.ABORTED.value)
        assertEquals(ErrorCode.NETWORK.value, UrlPlayerErrorCode.NETWORK.value)
        assertEquals(ErrorCode.TIMEOUT.value, UrlPlayerErrorCode.TIMEOUT.value)
        assertEquals(ErrorCode.DECODE.value, UrlPlayerErrorCode.DECODE.value)
        assertEquals(ErrorCode.UNSUPPORTED.value, UrlPlayerErrorCode.UNSUPPORTED.value)
        assertEquals(ErrorCode.DRM.value, UrlPlayerErrorCode.DRM.value)
        assertEquals(ErrorCode.SURFACE.value, UrlPlayerErrorCode.SURFACE.value)
        assertEquals(ErrorCode.INTERNAL.value, UrlPlayerErrorCode.INTERNAL.value)
        assertEquals(ErrorCode.UNKNOWN.value, UrlPlayerErrorCode.UNKNOWN.value)
        assertEquals(ErrorCode.entries.size, UrlPlayerErrorCode.entries.size)
    }

    @Test
    fun adapterMapsEveryPublicCode() {
        for (code in UrlPlayerErrorCode.entries) {
            assertEquals(code.value, code.toInternal().value)
        }
    }
}
