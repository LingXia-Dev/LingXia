//! Host-app update channel: `direct` (self-install) vs `store`.
//!
//! Baked from `lingxia.yaml` `update.channel` / `update.platforms` into
//! `app.json`. A store-installed process always behaves as `store`, even
//! when this build still says `direct` — so an Android sideload that later
//! gets replaced by Play flips channel without a yaml lock-in.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app_config;

/// How this binary is allowed to apply host-app updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    /// Download the LingXia feed package and install it in place.
    #[default]
    Direct,
    /// Store owns updates. Never download or self-install from the feed.
    Store,
}

/// Platform id used in `update.platforms` / `app.platforms`.
pub fn host_platform_id() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(all(target_os = "linux", target_env = "ohos")) {
        "harmony"
    } else {
        "unknown"
    }
}

/// Compile-time default when yaml omits both `channel` and this platform.
/// iOS and HarmonyOS cannot self-install; everything else starts `direct`.
pub fn default_update_channel(platform: &str) -> UpdateChannel {
    match platform {
        "ios" | "harmony" => UpdateChannel::Store,
        _ => UpdateChannel::Direct,
    }
}

/// Resolve the baked channel for `platform` without considering install source.
pub fn resolve_update_channel(
    default_channel: Option<UpdateChannel>,
    platforms: &BTreeMap<String, UpdateChannel>,
    platform: &str,
) -> UpdateChannel {
    platforms
        .get(platform)
        .copied()
        .or(default_channel)
        .unwrap_or_else(|| default_update_channel(platform))
}

/// Channel declared for this running platform in `app.json`.
pub fn configured_update_channel() -> UpdateChannel {
    let platform = host_platform_id();
    match app_config() {
        Some(config) => {
            resolve_update_channel(config.update_channel, &config.update_channels, platform)
        }
        None => default_update_channel(platform),
    }
}

/// Effective channel: a store-installed process is always `store`.
pub fn effective_update_channel(store_installed: bool) -> UpdateChannel {
    if store_installed {
        UpdateChannel::Store
    } else {
        configured_update_channel()
    }
}

/// Whether this process may download and self-install a host update.
pub fn self_update_allowed(physical: bool, store_installed: bool) -> bool {
    physical && effective_update_channel(store_installed) == UpdateChannel::Direct
}

/// Baked listing id for this host platform (`ios.store.appId`, …).
/// Android Play uses the running package name instead.
pub fn store_listing_id() -> Option<String> {
    let platform = host_platform_id();
    app_config().and_then(|config| {
        config
            .store_listing_ids
            .get(platform)
            .cloned()
            .filter(|id| !id.trim().is_empty())
    })
}

/// Deep link that opens this app's store listing. No country path — Apple's
/// storefront follows the signed-in Apple ID, not yaml.
///
/// Android's `market://` is only a portable hint: the SDK opens the
/// installer store (Play / Huawei / Honor / Xiaomi / OPPO / vivo / …)
/// using the running package name. Harmony prefers `appmarket://` with
/// the bundle name and falls back to the AppGallery HTTPS C-id page.
/// Product `appLinks` hosts are not used — those open this app, not the
/// store.
pub fn store_update_url(package_id: Option<&str>) -> Option<String> {
    store_update_url_for(
        host_platform_id(),
        store_listing_id().as_deref(),
        package_id,
    )
}

pub fn store_update_url_for(
    platform: &str,
    listing_id: Option<&str>,
    package_id: Option<&str>,
) -> Option<String> {
    match platform {
        "ios" | "macos" => {
            let id = listing_id?.trim();
            let id = id.strip_prefix("id").unwrap_or(id);
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(format!("itms-apps://apps.apple.com/app/id{id}"))
        }
        "android" => {
            let pkg = package_id.map(str::trim).filter(|s| !s.is_empty())?;
            Some(format!("market://details?id={pkg}"))
        }
        "windows" => {
            let id = listing_id.map(str::trim).filter(|s| !s.is_empty())?;
            Some(format!("ms-windows-store://pdp/?ProductId={id}"))
        }
        "harmony" => {
            if let Some(pkg) = package_id.map(str::trim).filter(|s| !s.is_empty()) {
                return Some(format!("appmarket://details?id={pkg}"));
            }
            let id = listing_id.map(str::trim).filter(|s| !s.is_empty())?;
            let id = if id.starts_with('C') || id.starts_with('c') {
                id.to_string()
            } else {
                format!("C{id}")
            };
            Some(format!("https://appgallery.huawei.com/app/{id}"))
        }
        _ => None,
    }
}

/// Prefer `storeUrl` from the prompt JSON; otherwise rebuild from baked ids.
pub fn store_url_from_info_json(info_json: &str, package_id: Option<&str>) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(info_json)
        && let Some(url) = value
            .get("storeUrl")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    {
        return Some(url.to_string());
    }
    store_update_url(package_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_yaml_defaults_match_today() {
        let empty = BTreeMap::new();
        assert_eq!(
            resolve_update_channel(None, &empty, "android"),
            UpdateChannel::Direct
        );
        assert_eq!(
            resolve_update_channel(None, &empty, "macos"),
            UpdateChannel::Direct
        );
        assert_eq!(
            resolve_update_channel(None, &empty, "windows"),
            UpdateChannel::Direct
        );
        assert_eq!(
            resolve_update_channel(None, &empty, "ios"),
            UpdateChannel::Store
        );
        assert_eq!(
            resolve_update_channel(None, &empty, "harmony"),
            UpdateChannel::Store
        );
    }

    #[test]
    fn platform_override_beats_default_channel() {
        let mut platforms = BTreeMap::new();
        platforms.insert("android".into(), UpdateChannel::Store);
        assert_eq!(
            resolve_update_channel(Some(UpdateChannel::Direct), &platforms, "android"),
            UpdateChannel::Store
        );
        assert_eq!(
            resolve_update_channel(Some(UpdateChannel::Direct), &platforms, "windows"),
            UpdateChannel::Direct
        );
    }

    #[test]
    fn store_install_overrides_direct_yaml() {
        assert!(!self_update_allowed(true, true));
        assert!(self_update_allowed(true, false));
        assert!(!self_update_allowed(false, false));
        assert!(!self_update_allowed(false, true));
    }

    #[test]
    fn apple_store_url_has_no_country_and_requires_numeric_id() {
        assert_eq!(
            store_update_url_for("ios", Some("1234567890"), None).as_deref(),
            Some("itms-apps://apps.apple.com/app/id1234567890")
        );
        assert_eq!(
            store_update_url_for("macos", Some("id987"), None).as_deref(),
            Some("itms-apps://apps.apple.com/app/id987")
        );
        assert_eq!(store_update_url_for("ios", Some("not-a-id"), None), None);
        assert_eq!(store_update_url_for("ios", None, None), None);
    }

    #[test]
    fn android_store_url_uses_package_name() {
        assert_eq!(
            store_update_url_for("android", None, Some("com.example.app")).as_deref(),
            Some("market://details?id=com.example.app")
        );
        assert_eq!(store_update_url_for("android", Some("ignored"), None), None);
    }

    #[test]
    fn windows_and_harmony_store_urls() {
        assert_eq!(
            store_update_url_for("windows", Some("9NABCDEF"), None).as_deref(),
            Some("ms-windows-store://pdp/?ProductId=9NABCDEF")
        );
        assert_eq!(
            store_update_url_for("harmony", Some("12345"), Some("com.example.app")).as_deref(),
            Some("appmarket://details?id=com.example.app")
        );
        assert_eq!(
            store_update_url_for("harmony", Some("12345"), None).as_deref(),
            Some("https://appgallery.huawei.com/app/C12345")
        );
        assert_eq!(
            store_update_url_for("harmony", Some("C99"), None).as_deref(),
            Some("https://appgallery.huawei.com/app/C99")
        );
    }
}
