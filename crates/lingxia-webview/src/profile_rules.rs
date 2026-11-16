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
    /// Console output reaches the delegate through the platform's console
    /// hook. Otherwise it travels only in a document-bound bridge envelope.
    pub(crate) native_console: bool,
    /// The runtime's baseline stylesheet and platform classes are injected.
    pub(crate) platform_baseline: bool,
    /// `alert` / `confirm` / `prompt` show their UI. Otherwise they are
    /// answered without one.
    pub(crate) js_dialogs: bool,
    /// A new window may open with no creator handler registered.
    pub(crate) new_windows_without_handler: bool,
    /// A response may become a download, handed to the creator's handler.
    pub(crate) downloads: bool,
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
    /// The native-component JavaScript interface is exposed (Android).
    pub(crate) native_component_bridge: bool,
}

impl SecurityProfile {
    pub(crate) const fn rules(self) -> ProfileRules {
        match self {
            SecurityProfile::StrictDefault => ProfileRules {
                native_console: true,
                platform_baseline: true,
                js_dialogs: false,
                new_windows_without_handler: false,
                downloads: false,
                persistent_data: false,
                site_storage: false,
                proxy: false,
                browser_user_agent: false,
                https_scheme_interception: true,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
                native_component_bridge: true,
            },
            SecurityProfile::BrowserRelaxed => ProfileRules {
                native_console: false,
                platform_baseline: false,
                js_dialogs: true,
                new_windows_without_handler: true,
                downloads: true,
                persistent_data: true,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: true,
                full_context_menu: true,
                proven_document_binding: true,
                native_component_bridge: false,
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
    #[cfg(any(
        target_os = "android",
        all(target_os = "linux", target_env = "ohos"),
        test
    ))]
    pub(crate) fn platform_token_json(&self) -> serde_json::Result<Vec<u8>> {
        #[derive(Serialize)]
        struct Token<'a> {
            #[serde(flatten)]
            options: &'a EffectiveWebViewCreateOptions,
            rules: ProfileRules,
        }
        serde_json::to_vec(&Token {
            options: self,
            rules: self.rules(),
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
    fn strict_rules_are_pinned() {
        assert_eq!(
            SecurityProfile::StrictDefault.rules(),
            ProfileRules {
                native_console: true,
                platform_baseline: true,
                js_dialogs: false,
                new_windows_without_handler: false,
                downloads: false,
                persistent_data: false,
                site_storage: false,
                proxy: false,
                browser_user_agent: false,
                https_scheme_interception: true,
                scrollbars: false,
                full_context_menu: false,
                proven_document_binding: false,
                native_component_bridge: true,
            }
        );
    }

    #[test]
    fn browser_rules_are_pinned() {
        assert_eq!(
            SecurityProfile::BrowserRelaxed.rules(),
            ProfileRules {
                native_console: false,
                platform_baseline: false,
                js_dialogs: true,
                new_windows_without_handler: true,
                downloads: true,
                persistent_data: true,
                site_storage: true,
                proxy: true,
                browser_user_agent: true,
                https_scheme_interception: false,
                scrollbars: true,
                full_context_menu: true,
                proven_document_binding: true,
                native_component_bridge: false,
            }
        );
    }

    // The Java and ArkTS decoders reject a token that lacks any of these.
    #[test]
    fn platform_token_carries_the_options_and_every_rule() {
        let json = options(SecurityProfile::BrowserRelaxed)
            .platform_token_json()
            .unwrap();
        let token: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(token["profile"], "browser_relaxed");
        assert_eq!(token["data_mode"], "profile_default");
        assert_eq!(token["has_download_handler"], false);
        let rules = token["rules"].as_object().unwrap();
        let mut names: Vec<&str> = rules.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "browser_user_agent",
                "downloads",
                "full_context_menu",
                "https_scheme_interception",
                "js_dialogs",
                "native_component_bridge",
                "native_console",
                "new_windows_without_handler",
                "persistent_data",
                "platform_baseline",
                "proven_document_binding",
                "proxy",
                "scrollbars",
                "site_storage",
            ]
        );
        assert!(rules.values().all(serde_json::Value::is_boolean));
    }
}
