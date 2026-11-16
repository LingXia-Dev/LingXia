package com.lingxia.webview;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import android.util.Base64;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;

import java.nio.charset.StandardCharsets;

@RunWith(RobolectricTestRunner.class)
public final class CreateOptionsTokenTest {
    private static final String[] RULES = {
        "native_console",
        "js_dialogs",
        "new_windows_without_handler",
        "downloads",
        "site_storage",
        "proxy",
        "scrollbars",
        "proven_document_binding",
        "native_component_bridge",
    };

    private static JSONObject rules(boolean value) throws Exception {
        JSONObject rules = new JSONObject();
        for (String name : RULES) {
            rules.put(name, value);
        }
        return rules;
    }

    private static JSONObject options(String profile, JSONObject rules) throws Exception {
        return new JSONObject().put("profile", profile).put("rules", rules);
    }

    private static String token(String json) {
        return Base64.encodeToString(
                json.getBytes(StandardCharsets.UTF_8),
                Base64.URL_SAFE | Base64.NO_WRAP | Base64.NO_PADDING);
    }

    private static LingXiaWebView.CreateOptions decode(JSONObject options) {
        return LingXiaWebView.CreateOptions.fromToken(token(options.toString()));
    }

    @Test
    public void reads_each_rule_from_the_token_whatever_the_profile_is_called() throws Exception {
        JSONObject rules = rules(false).put("native_console", true).put("downloads", true);
        LingXiaWebView.CreateOptions options = decode(
                options("a_profile_this_side_never_heard_of", rules)
                        .put("data_mode", "ephemeral")
                        .put("has_download_handler", true));

        assertEquals("ephemeral", options.dataMode);
        assertTrue(options.hasDownloadHandler);
        assertTrue(options.rules.nativeConsole);
        assertTrue(options.rules.downloads);
        assertFalse(options.rules.jsDialogs);
        assertFalse(options.rules.newWindowsWithoutHandler);
        assertFalse(options.rules.siteStorage);
        assertFalse(options.rules.proxy);
        assertFalse(options.rules.scrollbars);
        assertFalse(options.rules.provenDocumentBinding);
        assertFalse(options.rules.nativeComponentBridge);
    }

    @Test
    public void rejects_a_token_that_lacks_any_rule() throws Exception {
        for (String name : RULES) {
            JSONObject rules = rules(true);
            rules.remove(name);
            assertThrows(name, IllegalArgumentException.class,
                    () -> decode(options("strict_default", rules)));
        }
        assertThrows(IllegalArgumentException.class,
                () -> decode(options("strict_default", rules(true).put("downloads", "true"))));
        assertThrows(IllegalArgumentException.class,
                () -> decode(new JSONObject().put("profile", "strict_default")));
    }

    @Test
    public void rejects_a_token_that_names_no_profile() throws Exception {
        assertThrows(IllegalArgumentException.class,
                () -> decode(new JSONObject().put("rules", rules(true))));
    }

    @Test
    public void rejects_an_unknown_data_mode_instead_of_the_persistent_default() throws Exception {
        assertThrows(IllegalArgumentException.class,
                () -> decode(options("browser_relaxed", rules(true)).put("data_mode", "partitioned")));
    }

    @Test
    public void rejects_a_missing_or_undecodable_token() {
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(null));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(""));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken("!!!"));
        assertThrows(IllegalArgumentException.class, () -> LingXiaWebView.CreateOptions.fromToken(token("[1]")));
    }

    @Test
    public void permits_nothing_before_a_token_is_applied() {
        ProfileRules none = LingXiaWebView.CreateOptions.unconfigured().rules;
        assertFalse(none.nativeConsole || none.jsDialogs || none.newWindowsWithoutHandler
                || none.downloads || none.siteStorage || none.proxy || none.scrollbars
                || none.provenDocumentBinding || none.nativeComponentBridge);
    }
}
