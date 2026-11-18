package com.lingxia.webview;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import android.net.Uri;
import android.util.Base64;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.Map;

@RunWith(RobolectricTestRunner.class)
public final class CreateOptionsTokenTest {
    private static final String[] RULES = {
        "bridge_transport",
        "embed_mailbox",
        "native_component_bridge",
        "native_console",
        "js_dialogs",
        "new_windows_without_handler",
        "downloads",
        "reports_blocked_downloads",
        "persistent_data",
        "site_storage",
        "proxy",
        "scrollbars",
        "proven_document_binding",
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
        assertFalse(options.rules.bridgeTransport);
        assertFalse(options.rules.embedMailbox);
        assertFalse(options.rules.reportsBlockedDownloads);
        assertFalse(options.rules.persistentData);
    }

    @Test
    public void rejects_a_token_that_lacks_any_rule() throws Exception {
        for (String name : RULES) {
            JSONObject rules = rules(true);
            rules.remove(name);
            assertThrows(name, IllegalArgumentException.class,
                    () -> decode(options("app_page", rules)));
        }
        assertThrows(IllegalArgumentException.class,
                () -> decode(options("app_page", rules(true).put("downloads", "true"))));
        assertThrows(IllegalArgumentException.class,
                () -> decode(new JSONObject().put("profile", "app_page")));
    }

    @Test
    public void rejects_a_token_that_names_no_profile() throws Exception {
        assertThrows(IllegalArgumentException.class,
                () -> decode(new JSONObject().put("rules", rules(true))));
    }

    @Test
    public void rejects_an_unknown_data_mode_instead_of_the_persistent_default() throws Exception {
        assertThrows(IllegalArgumentException.class,
                () -> decode(options("browser_tab", rules(true)).put("data_mode", "partitioned")));
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
                || none.downloads || none.persistentData || none.siteStorage || none.proxy
                || none.scrollbars || none.provenDocumentBinding || none.nativeComponentBridge
                || none.bridgeTransport || none.embedMailbox || none.reportsBlockedDownloads);
    }

    @Test
    public void embedded_ephemeral_mode_never_wipes_process_cookies_without_multi_profile()
            throws Exception {
        TestWebView embedded = new TestWebView(false);
        embedded.applyCreateOptions(decode(
                options("embedded_document", rules(false).put("embed_mailbox", true))
                        .put("data_mode", "ephemeral")));
        assertFalse(embedded.shouldUseGlobalEphemeralFallback(false));
        assertFalse(embedded.shouldUseGlobalEphemeralFallback(true));

        TestWebView browserAuth = new TestWebView(false);
        browserAuth.applyCreateOptions(decode(
                options("browser_tab", rules(false).put("persistent_data", true))
                        .put("data_mode", "ephemeral")));
        assertTrue(browserAuth.shouldUseGlobalEphemeralFallback(false));
        assertFalse(browserAuth.shouldUseGlobalEphemeralFallback(true));
    }

    @Test
    public void main_frame_post_navigation_is_checked_and_blocked_before_network_or_proxy() {
        TestWebView blockedView = new TestWebView(true);
        blockedView.forceSkipRustIntercept = true;
        LingXiaWebViewClient client = new LingXiaWebViewClient(blockedView);

        WebResourceResponse blocked = client.shouldInterceptRequest(
                blockedView, request("https://untrusted.example/post", "POST", true));
        assertNotNull(blocked);
        assertEquals(204, blocked.getStatusCode());
        assertEquals(1, blockedView.navigationChecks);
        assertEquals(0, blockedView.schemeChecks);

        // Allowed main-frame POST proceeds (respecting proxy skip).
        TestWebView allowedView = new TestWebView(false);
        allowedView.forceSkipRustIntercept = true;
        LingXiaWebViewClient allowedClient = new LingXiaWebViewClient(allowedView);
        assertNull(allowedClient.shouldInterceptRequest(
                allowedView, request("https://trusted.example/post", "POST", true)));
        assertEquals(1, allowedView.navigationChecks);
        assertEquals(0, allowedView.schemeChecks);

        // Main-frame GET and subframe POST are not re-checked in shouldInterceptRequest.
        TestWebView getAndSubframe = new TestWebView(true);
        getAndSubframe.forceSkipRustIntercept = true;
        LingXiaWebViewClient otherClient = new LingXiaWebViewClient(getAndSubframe);
        assertNull(otherClient.shouldInterceptRequest(
                getAndSubframe, request("https://trusted.example/get", "GET", true)));
        assertNull(otherClient.shouldInterceptRequest(
                getAndSubframe, request("https://untrusted.example/sub", "POST", false)));
        assertEquals(0, getAndSubframe.navigationChecks);
    }

    private static WebResourceRequest request(
            final String url, final String method, final boolean mainFrame) {
        return new WebResourceRequest() {
            @Override public Uri getUrl() { return Uri.parse(url); }
            @Override public boolean isForMainFrame() { return mainFrame; }
            @Override public boolean isRedirect() { return false; }
            @Override public boolean hasGesture() { return true; }
            @Override public String getMethod() { return method; }
            @Override public Map<String, String> getRequestHeaders() {
                return Collections.emptyMap();
            }
        };
    }

    private static final class TestWebView extends LingXiaWebView {
        private final boolean blockNavigation;
        boolean forceSkipRustIntercept;
        int navigationChecks;
        int schemeChecks;

        TestWebView(boolean blockNavigation) {
            super(RuntimeEnvironment.getApplication());
            this.blockNavigation = blockNavigation;
        }

        @Override
        public boolean shouldSkipRustIntercept(String url) {
            return forceSkipRustIntercept;
        }

        @Override
        boolean checkNavigationPolicy(String url, boolean hasUserGesture, boolean isMainFrame) {
            navigationChecks++;
            return blockNavigation;
        }

        @Override
        WebResourceResponseData interceptSchemeRequest(
                String url, String method, String[] headers, boolean isMainFrame) {
            schemeChecks++;
            return null;
        }
    }
}
