//! What each security profile permits.
//!
//! Backends read these fields and never compare profiles, so a new profile is
//! one more arm in [`SecurityProfile::rules`] and the compiler asks for every
//! field.

use serde::Serialize;

use crate::webview::{EffectiveWebViewCreateOptions, SecurityProfile};

/// Permissions of one security profile. Android and Harmony receive the table
/// inside the create-options token and read nothing else about the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ProfileRules {
    /// The lxapp bridge transports are installed in the page: the `LingXia`
    /// message handler and `lx-apple` scheme (Apple), `LingXiaProxy` and the
    /// message port (Android, Harmony), `LingXiaProxy` and web messages
    /// (Windows). Without them no page script can reach the bridge at all;
    /// with them, admission is decided downstream per message.
    pub(crate) bridge_transport: bool,
    /// The one-way `window.lingxia.postMessage` mailbox is injected into
    /// top-level embedded documents.
    pub(crate) embed_mailbox: bool,
    /// The native-component JavaScript interface (Android) and same-level
    /// native embed mode (Harmony) are exposed.
    pub(crate) native_component_bridge: bool,
    /// `lx` is served even when the creator registered no scheme (Apple).
    /// Otherwise only the creator's schemes exist.
    pub(crate) default_lx_scheme: bool,
    /// Console output reaches the delegate through the platform's console
    /// hook. Otherwise it travels only in a document-bound bridge envelope,
    /// and not at all without a bridge transport.
    pub(crate) native_console: bool,
    /// The runtime's baseline stylesheet and platform classes are injected.
    pub(crate) platform_baseline: bool,
    /// `alert` / `confirm` / `prompt` show their UI. Otherwise they are
    /// answered without one.
    pub(crate) js_dialogs: bool,
    /// A new window may open with no creator handler registered.
    pub(crate) new_windows_without_handler: bool,
    /// A new window loaded in place is rewritten from `http` to `https`
    /// (Apple). Otherwise the creator's handler judged the URL it loads.
    pub(crate) new_window_https_upgrade: bool,
    /// Creation fails without a creator navigation handler.
    pub(crate) requires_navigation_handler: bool,
    /// A response may become a download: the navigation is cancelled and the
    /// full request, cookies included, is handed to the creator to perform.
    pub(crate) downloads: bool,
    /// A response that would download is cancelled, handler or not, and the
    /// creator is told its URL and nothing it could fetch it with.
    pub(crate) reports_blocked_downloads: bool,
    /// Website data defaults to the persistent store browser tabs share.
    /// Otherwise a view's data is discarded with it.
    pub(crate) persistent_data: bool,
    /// DOM storage and web databases are enabled (Android, Harmony).
    pub(crate) site_storage: bool,
    /// The configured proxy applies, and creation fails when it cannot.
    pub(crate) proxy: bool,
    /// The user agent carries browser tokens (iPhone).
    pub(crate) browser_user_agent: bool,
    /// A top-level `https` navigation is offered to a registered `https`
    /// scheme handler (Apple).
    pub(crate) https_scheme_interception: bool,
    /// Scroll bars are drawn (Android, Harmony).
    pub(crate) scrollbars: bool,
    /// The whole context menu is kept. Otherwise only its editing items are
    /// (Windows).
    pub(crate) full_context_menu: bool,
    /// A document is bound to the bridge only on proof of its visible commit,
    /// and its port is minted once (Android, Harmony).
    pub(crate) proven_document_binding: bool,
}

impl ProfileRules {
    /// A response that would download never proceeds in the WebView.
    pub(crate) const fn intercepts_downloads(&self) -> bool {
        self.downloads || self.reports_blocked_downloads
    }
}

impl SecurityProfile {
    pub(crate) const fn rules(self) -> ProfileRules {
        match self {
            SecurityProfile::AppPage => ProfileRules {
                bridge_transport: true,
                embed_mailbox: false,
                native_component_bridge: true,
                default_lx_scheme: true,
                native_console: true,
                platform_baseline: true,
                js_dialogs: false,
                new_windows_without_handler: false,
                new_window_https_upgrade: true,
                requires_navigation_handler: false,
                downloads: false,
                reports_blocked_downloads: false,
                persistent_data: false,
                site_storage: false,
                proxy: false,
                browser_user_agent: false,
                https_scheme_interception: true,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
            },
            SecurityProfile::BrowserTab => ProfileRules {
                bridge_transport: true,
                embed_mailbox: false,
                native_component_bridge: false,
                default_lx_scheme: true,
                native_console: false,
                platform_baseline: false,
                js_dialogs: true,
                new_windows_without_handler: true,
                new_window_https_upgrade: true,
                requires_navigation_handler: false,
                downloads: true,
                reports_blocked_downloads: false,
                persistent_data: true,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: true,
                full_context_menu: true,
                proven_document_binding: true,
            },
            SecurityProfile::Embedded => ProfileRules {
                bridge_transport: false,
                embed_mailbox: true,
                native_component_bridge: false,
                default_lx_scheme: false,
                native_console: false,
                platform_baseline: false,
                js_dialogs: false,
                new_windows_without_handler: false,
                new_window_https_upgrade: false,
                requires_navigation_handler: true,
                downloads: false,
                reports_blocked_downloads: true,
                persistent_data: false,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
            },
        }
    }
}

impl EffectiveWebViewCreateOptions {
    pub(crate) const fn rules(&self) -> ProfileRules {
        self.profile.rules()
    }

    /// The create-options token Android and Harmony decode: these options
    /// plus the profile's rules, so neither side keeps a table of its own.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn platform_token_json(&self) -> serde_json::Result<Vec<u8>> {
        self.platform_token_json_with_embed_init(None)
    }

    #[cfg(any(
        target_os = "android",
        all(target_os = "linux", target_env = "ohos"),
        test
    ))]
    pub(crate) fn platform_token_json_with_embed_init(
        &self,
        embed_port_init: Option<&str>,
    ) -> serde_json::Result<Vec<u8>> {
        #[derive(Serialize)]
        struct Token<'a> {
            #[serde(flatten)]
            options: &'a EffectiveWebViewCreateOptions,
            rules: ProfileRules,
            #[serde(skip_serializing_if = "Option::is_none")]
            embed_port_init: Option<&'a str>,
        }
        serde_json::to_vec(&Token {
            options: self,
            rules: self.rules(),
            embed_port_init,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webview::WebViewDataMode;

    fn options(profile: SecurityProfile) -> EffectiveWebViewCreateOptions {
        EffectiveWebViewCreateOptions {
            profile,
            data_mode: WebViewDataMode::ProfileDefault,
            registered_schemes: Vec::new(),
            has_navigation_handler: false,
            has_new_window_handler: false,
            has_download_handler: false,
            has_file_chooser_handler: false,
            has_delegate: false,
            surface_owned: false,
        }
    }

    #[test]
    fn app_page_rules_are_pinned() {
        assert_eq!(
            SecurityProfile::AppPage.rules(),
            ProfileRules {
                bridge_transport: true,
                embed_mailbox: false,
                native_component_bridge: true,
                default_lx_scheme: true,
                native_console: true,
                platform_baseline: true,
                js_dialogs: false,
                new_windows_without_handler: false,
                new_window_https_upgrade: true,
                requires_navigation_handler: false,
                downloads: false,
                reports_blocked_downloads: false,
                persistent_data: false,
                site_storage: false,
                proxy: false,
                browser_user_agent: false,
                https_scheme_interception: true,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
            }
        );
    }

    #[test]
    fn browser_tab_rules_are_pinned() {
        assert_eq!(
            SecurityProfile::BrowserTab.rules(),
            ProfileRules {
                bridge_transport: true,
                embed_mailbox: false,
                native_component_bridge: false,
                default_lx_scheme: true,
                native_console: false,
                platform_baseline: false,
                js_dialogs: true,
                new_windows_without_handler: true,
                new_window_https_upgrade: true,
                requires_navigation_handler: false,
                downloads: true,
                reports_blocked_downloads: false,
                persistent_data: true,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: true,
                full_context_menu: true,
                proven_document_binding: true,
            }
        );
    }

    #[test]
    fn embedded_rules_are_pinned() {
        assert_eq!(
            SecurityProfile::Embedded.rules(),
            ProfileRules {
                bridge_transport: false,
                embed_mailbox: true,
                native_component_bridge: false,
                default_lx_scheme: false,
                native_console: false,
                platform_baseline: false,
                js_dialogs: false,
                new_windows_without_handler: false,
                new_window_https_upgrade: false,
                requires_navigation_handler: true,
                downloads: false,
                reports_blocked_downloads: true,
                persistent_data: false,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
            }
        );
    }

    // Whatever a later profile permits, a page without the bridge transport
    // must not be handed the surfaces that assume one.
    #[test]
    fn no_profile_grants_bridge_surfaces_without_the_transport() {
        for profile in [
            SecurityProfile::AppPage,
            SecurityProfile::BrowserTab,
            SecurityProfile::Embedded,
        ] {
            let rules = profile.rules();
            if !rules.bridge_transport {
                assert!(!rules.native_component_bridge, "{profile:?}");
                assert!(!rules.native_console, "{profile:?}");
                assert!(!rules.platform_baseline, "{profile:?}");
                assert!(!rules.proven_document_binding, "{profile:?}");
            }
            assert!(
                !(rules.downloads && rules.reports_blocked_downloads),
                "{profile:?}"
            );
        }
    }

    // The Java and ArkTS decoders reject a token that lacks any of these.
    #[test]
    fn platform_token_carries_the_options_and_every_rule() {
        let json = options(SecurityProfile::BrowserTab)
            .platform_token_json()
            .unwrap();
        let token: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(token["profile"], "browser_tab");
        assert_eq!(token["data_mode"], "profile_default");
        assert_eq!(token["has_download_handler"], false);
        let rules = token["rules"].as_object().unwrap();
        let mut names: Vec<&str> = rules.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "bridge_transport",
                "browser_user_agent",
                "default_lx_scheme",
                "downloads",
                "embed_mailbox",
                "full_context_menu",
                "https_scheme_interception",
                "js_dialogs",
                "native_component_bridge",
                "native_console",
                "new_window_https_upgrade",
                "new_windows_without_handler",
                "persistent_data",
                "platform_baseline",
                "proven_document_binding",
                "proxy",
                "reports_blocked_downloads",
                "requires_navigation_handler",
                "scrollbars",
                "site_storage",
            ]
        );
        assert!(rules.values().all(serde_json::Value::is_boolean));
        assert!(token.get("embed_port_init").is_none());

        let harmony_embed = options(SecurityProfile::Embedded)
            .platform_token_json_with_embed_init(Some("LingXia-embed-port:secret"))
            .unwrap();
        let harmony_token: serde_json::Value = serde_json::from_slice(&harmony_embed).unwrap();
        assert_eq!(
            harmony_token["embed_port_init"],
            "LingXia-embed-port:secret"
        );
    }
}
