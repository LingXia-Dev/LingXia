package com.lingxia.webview;

import org.json.JSONObject;

/**
 * What the WebView's security profile permits. The table is owned by Rust and
 * arrives in the create-options token; this side reads it and holds none.
 */
final class ProfileRules {
    /** Before the token is applied nothing is permitted. */
    static final ProfileRules NONE =
            new ProfileRules(false, false, false, false, false, false, false, false, false);

    final boolean nativeConsole;
    final boolean jsDialogs;
    final boolean newWindowsWithoutHandler;
    final boolean downloads;
    final boolean siteStorage;
    final boolean proxy;
    final boolean scrollbars;
    final boolean provenDocumentBinding;
    final boolean nativeComponentBridge;

    private ProfileRules(
            boolean nativeConsole,
            boolean jsDialogs,
            boolean newWindowsWithoutHandler,
            boolean downloads,
            boolean siteStorage,
            boolean proxy,
            boolean scrollbars,
            boolean provenDocumentBinding,
            boolean nativeComponentBridge) {
        this.nativeConsole = nativeConsole;
        this.jsDialogs = jsDialogs;
        this.newWindowsWithoutHandler = newWindowsWithoutHandler;
        this.downloads = downloads;
        this.siteStorage = siteStorage;
        this.proxy = proxy;
        this.scrollbars = scrollbars;
        this.provenDocumentBinding = provenDocumentBinding;
        this.nativeComponentBridge = nativeComponentBridge;
    }

    /** A missing or non-boolean rule rejects the token: no rule has a default. */
    static ProfileRules fromJson(JSONObject rules) {
        if (rules == null) {
            throw new IllegalArgumentException("create options carry no profile rules");
        }
        return new ProfileRules(
                rule(rules, "native_console"),
                rule(rules, "js_dialogs"),
                rule(rules, "new_windows_without_handler"),
                rule(rules, "downloads"),
                rule(rules, "site_storage"),
                rule(rules, "proxy"),
                rule(rules, "scrollbars"),
                rule(rules, "proven_document_binding"),
                rule(rules, "native_component_bridge"));
    }

    private static boolean rule(JSONObject rules, String name) {
        Object value = rules.opt(name);
        if (!(value instanceof Boolean)) {
            throw new IllegalArgumentException("profile rule is missing from create options: " + name);
        }
        return (Boolean) value;
    }
}
