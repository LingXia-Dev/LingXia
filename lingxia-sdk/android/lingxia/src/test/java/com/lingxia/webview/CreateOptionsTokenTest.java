package com.lingxia.webview;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import android.util.Base64;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;

import java.nio.charset.StandardCharsets;

@RunWith(RobolectricTestRunner.class)
public final class CreateOptionsTokenTest {
    private static String token(String json) {
        return Base64.encodeToString(
                json.getBytes(StandardCharsets.UTF_8),
                Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
    }

    @Test
    public void decodes_the_two_known_profiles() {
        LingXiaWebView.CreateOptions strict =
                LingXiaWebView.CreateOptions.fromToken(token("{\"profile\":\"strict_default\"}"));
        assertEquals("strict_default", strict.profile);
        assertEquals("profile_default", strict.dataMode);

        LingXiaWebView.CreateOptions browser = LingXiaWebView.CreateOptions.fromToken(token(
                "{\"profile\":\"browser_relaxed\",\"data_mode\":\"ephemeral\",\"has_download_handler\":true}"));
        assertEquals("browser_relaxed", browser.profile);
        assertEquals("ephemeral", browser.dataMode);
        assertTrue(browser.hasDownloadHandler);
    }

    @Test
    public void rejects_an_unknown_or_missing_profile_instead_of_granting_strict() {
        assertThrows(IllegalArgumentException.class,
                () -> LingXiaWebView.CreateOptions.fromToken(token("{\"profile\":\"embedded_document\"}")));
        assertThrows(IllegalArgumentException.class,
                () -> LingXiaWebView.CreateOptions.fromToken(token("{\"profile\":\"Strict_Default\"}")));
        assertThrows(IllegalArgumentException.class,
                () -> LingXiaWebView.CreateOptions.fromToken(token("{\"data_mode\":\"ephemeral\"}")));
    }

    @Test
    public void rejects_an_unknown_data_mode_instead_of_the_persistent_default() {
        assertThrows(IllegalArgumentException.class,
                () -> LingXiaWebView.CreateOptions.fromToken(token(
                        "{\"profile\":\"browser_relaxed\",\"data_mode\":\"partitioned\"}")));
    }

    @Test
    public void rejects_a_missing_or_undecodable_token() {
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(null));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(""));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken("!!!"));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(token("[1]")));
    }
}
