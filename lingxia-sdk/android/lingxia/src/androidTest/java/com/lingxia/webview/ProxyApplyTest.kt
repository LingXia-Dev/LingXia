package com.lingxia.webview

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.webkit.WebViewFeature
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ProxyApplyTest {
    @Test fun backgroundApplyAndClearReceiveChromiumAcknowledgement() {
        assumeTrue(WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE))
        try {
            assertNull(LingXiaWebView.applyHttpProxy("127.0.0.1", 9, emptyArray()))
        } finally {
            assertNull(LingXiaWebView.applyHttpProxy(null, 0, emptyArray()))
        }
    }

    @Test fun mainThreadCallFailsWithoutBlockingItsOwnCallback() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            assertEquals("ERROR:proxy apply requires a background caller",
                LingXiaWebView.applyHttpProxy("127.0.0.1", 9, emptyArray()))
        }
    }
}
