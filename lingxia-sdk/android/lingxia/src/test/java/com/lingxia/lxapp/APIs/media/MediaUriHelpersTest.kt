package com.lingxia.lxapp.APIs.media

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaUriHelpersTest {
    @Test
    fun remoteHttpUrlsAreDetectedForImageAndVideo() {
        assertTrue(isRemoteHttpUrl("https://cdn.example.com/photo.jpg"))
        assertTrue(isRemoteHttpUrl("https://cdn.example.com/clip.mp4"))
        assertTrue(isRemoteHttpUrl("HTTPS://cdn.example.com/photo.jpg"))
        assertTrue(isRemoteHttpUrl("http://cdn.example.com/photo.jpg"))
        assertFalse(isRemoteHttpUrl("lx://usercache/a.png"))
        assertFalse(isRemoteHttpUrl("/sdcard/a.png"))
    }
}
