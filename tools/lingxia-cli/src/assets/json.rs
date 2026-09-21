use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, ResolvedEnv, UpdateSigningConfig};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::collections::HashMap;

use super::bundles::PreparedResourceBundle;
use super::icons::{
    PreparedAppUiIcon, rewrite_app_ui_icon_paths, rewrite_windows_app_ui_icon_paths,
};
use super::ui::effective_ui_config;

/// Build the runtime `app.json` for the host app.
///
/// `resolved_env` is the single source of truth for the active environment:
/// - `lingxiaServer` is taken from the resolved environment.
/// - `lingxiaId` is emitted verbatim (env-independent).
/// - `env` is always emitted (defaults to `prod`).
/// - `appLinks.hosts` is the resolved list for this env (omitted when empty).
pub(super) fn build_app_json_from_config(
    config: &LingXiaConfig,
    home_bundle: Option<&PreparedResourceBundle>,
    dev_ws_url: Option<&str>,
    resolved_env: &ResolvedEnv,
) -> Result<String> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;
    let lingxia_server = resolved_env.lingxia_server.as_str();

    let mut obj = serde_json::Map::new();
    obj.insert(
        "productName".to_string(),
        serde_json::json!(app.product_name),
    );
    if !app.product_names.is_empty() {
        obj.insert(
            "productNames".to_string(),
            serde_json::to_value(&app.product_names)?,
        );
    }
    obj.insert(
        "productVersion".to_string(),
        serde_json::json!(app.product_version),
    );

    if !lingxia_server.is_empty() {
        obj.insert(
            "lingxiaServer".to_string(),
            serde_json::json!(lingxia_server),
        );
    }
    if let Some(lingxia_id) = app.lingxia_id.as_deref().filter(|s| !s.is_empty()) {
        // Verbatim: the env suffix is package-id only, never lingxiaId.
        obj.insert("lingxiaId".to_string(), serde_json::json!(lingxia_id));
    }
    if let Some(windows_app_id) = config.resolved_windows_app_id(resolved_env)? {
        obj.insert(
            "windowsAppId".to_string(),
            serde_json::json!(windows_app_id),
        );
    }
    obj.insert(
        "env".to_string(),
        serde_json::json!(resolved_env.version.as_str()),
    );

    if let Some(home_bundle) = home_bundle {
        let home_app_id = app.home_app_id.as_deref().ok_or_else(|| {
            anyhow!("prepared a home lxapp bundle but app.homeAppId is not configured")
        })?;
        obj.insert("homeAppId".to_string(), serde_json::json!(home_app_id));
        obj.insert(
            "homeAppVersion".to_string(),
            serde_json::json!(home_bundle.version.as_str()),
        );
    }
    if let Some(browser) = config.browser.as_ref() {
        obj.insert(
            "browser".to_string(),
            serde_json::json!({ "bookmarks": browser.bookmarks }),
        );
    }
    if let Some(storage) = config.storage.as_ref() {
        obj.insert("storage".to_string(), serde_json::to_value(storage)?);
    }
    if let Some(dev_ws_url) = dev_ws_url.map(str::trim).filter(|value| !value.is_empty()) {
        obj.insert("devWsUrl".to_string(), serde_json::json!(dev_ws_url));
        if let Some(dev_bundle_base_url) = dev_bundle_base_url(dev_ws_url) {
            obj.insert(
                "devBundleBaseUrl".to_string(),
                serde_json::json!(dev_bundle_base_url),
            );
        }
    }
    if !resolved_env.app_link_hosts.is_empty() {
        obj.insert(
            "appLinks".to_string(),
            serde_json::json!({ "hosts": resolved_env.app_link_hosts }),
        );
    }
    if let Some(capabilities) = config.capabilities.as_ref() {
        obj.insert(
            "capabilities".to_string(),
            serde_json::to_value(capabilities)?,
        );
    }
    if let Some(theme) = config.theme.as_ref() {
        obj.insert("theme".to_string(), serde_json::to_value(theme)?);
    }
    if let Some(destination) = config.settings_destination.as_ref() {
        obj.insert(
            "settingsDestination".to_string(),
            serde_json::to_value(destination)?,
        );
    }
    // Only the minimum-hold time reaches the runtime: the images and colors are
    // platform resources, and the upper bound is a framework constant.
    if let Some(splash) = config.splash.as_ref() {
        obj.insert(
            "splash".to_string(),
            serde_json::json!({ "minDuration": splash.min_duration }),
        );
    }

    if let Some(update) = config.update.as_ref() {
        update.validate()?;
        warn_missing_store_listing_ids(config, update);
        if !update.trusted_public_keys.is_empty() {
            obj.insert(
                "updateTrustedPublicKeys".to_string(),
                serde_json::json!(update.trusted_public_keys),
            );
        }
        if let Some(channel) = update.channel {
            obj.insert("updateChannel".to_string(), serde_json::to_value(channel)?);
        }
        if !update.platforms.is_empty() {
            obj.insert(
                "updateChannels".to_string(),
                serde_json::to_value(&update.platforms)?,
            );
        }
    }

    let store_listing_ids = store_listing_ids_from_config(config);
    if !store_listing_ids.is_empty() {
        obj.insert(
            "storeListingIds".to_string(),
            serde_json::to_value(store_listing_ids)?,
        );
    }

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(
        obj,
    ))?)
}

/// Platforms whose store listing cannot be opened at runtime: the channel
/// resolves to `store` but no listing id is configured. Android and Harmony
/// address the listing by package / bundle name, so they never appear here.
pub(super) fn store_platforms_missing_listing_id(
    app_platforms: &[String],
    update: &UpdateSigningConfig,
    listing_ids: &HashMap<String, String>,
) -> Vec<String> {
    use lingxia_app_context::update::{UpdateChannel, resolve_update_channel};

    app_platforms
        .iter()
        .filter(|platform| matches!(platform.as_str(), "ios" | "macos" | "windows"))
        .filter(|platform| {
            resolve_update_channel(update.channel, &update.platforms, platform)
                == UpdateChannel::Store
        })
        .filter(|platform| !listing_ids.contains_key(platform.as_str()))
        .cloned()
        .collect()
}

fn warn_missing_store_listing_ids(config: &LingXiaConfig, update: &UpdateSigningConfig) {
    let app_platforms = config
        .app
        .as_ref()
        .map(|app| app.platforms.as_slice())
        .unwrap_or(&[]);
    let listing_ids = store_listing_ids_from_config(config);
    for platform in store_platforms_missing_listing_id(app_platforms, update, &listing_ids) {
        eprintln!(
            "{}: {platform} uses update channel `store` but {platform}.store.appId is not set; the update prompt will have no listing to open",
            "warning".yellow().bold()
        );
    }
}

/// Numeric / Partner-Center listing ids used to open the store page.
/// Android Play uses the running package name, so it is not baked here.
fn store_listing_ids_from_config(config: &LingXiaConfig) -> HashMap<String, String> {
    let mut ids = HashMap::new();
    let mut push = |platform: &str, id: Option<&str>| {
        if let Some(id) = id.map(str::trim).filter(|id| !id.is_empty()) {
            ids.insert(platform.to_string(), id.to_string());
        }
    };
    push(
        "ios",
        config
            .ios
            .as_ref()
            .and_then(|cfg| cfg.store.as_ref())
            .and_then(|store| store.app_id.as_deref()),
    );
    push(
        "macos",
        config
            .macos
            .as_ref()
            .and_then(|cfg| cfg.store.as_ref())
            .and_then(|store| store.app_id.as_deref()),
    );
    push(
        "windows",
        config
            .windows
            .as_ref()
            .and_then(|cfg| cfg.store.as_ref())
            .map(|store| store.app_id.as_str()),
    );
    push(
        "harmony",
        config
            .harmony
            .as_ref()
            .and_then(|cfg| cfg.store.as_ref())
            .map(|store| store.app_id.as_str()),
    );
    ids
}

fn dev_bundle_base_url(dev_ws_url: &str) -> Option<String> {
    let rest = dev_ws_url
        .strip_prefix("ws://")
        .or_else(|| dev_ws_url.strip_prefix("wss://"))?;
    let scheme = if dev_ws_url.starts_with("wss://") {
        "https"
    } else {
        "http"
    };
    let (authority_and_path, query) = rest
        .split_once('?')
        .map(|(base, query)| (base, Some(query)))
        .unwrap_or((rest, None));
    let authority = authority_and_path
        .split('/')
        .next()
        .filter(|authority| !authority.is_empty())?;
    let mut url = format!("{scheme}://{authority}/__lingxia/dev");
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}

pub(super) fn build_ui_json_from_config(
    config: &LingXiaConfig,
    app_ui_icons: &[PreparedAppUiIcon],
    platform: &str,
) -> Result<Option<String>> {
    let Some(ui) = effective_ui_config(config, Some(platform))? else {
        return Ok(None);
    };
    let mut rewritten = ui;
    if !app_ui_icons.is_empty() {
        let by_source = app_ui_icons
            .iter()
            .map(|icon| (icon.source_path.as_str(), icon.relative_path.as_str()))
            .collect::<HashMap<_, _>>();
        rewrite_app_ui_icon_paths(&mut rewritten, &by_source)?;
    }
    Ok(Some(serde_json::to_string_pretty(&rewritten)?))
}

pub(super) fn build_windows_ui_json_from_config(
    config: &LingXiaConfig,
    app_ui_icons: &[PreparedAppUiIcon],
) -> Result<Option<String>> {
    let Some(ui) = effective_ui_config(config, Some("windows"))? else {
        return Ok(None);
    };
    let mut rewritten = ui;
    if !app_ui_icons.is_empty() {
        let by_source = app_ui_icons
            .iter()
            .map(|icon| {
                (
                    icon.source_path.as_str(),
                    icon.windows_relative_path.as_str(),
                )
            })
            .collect::<HashMap<_, _>>();
        rewrite_windows_app_ui_icon_paths(&mut rewritten, &by_source)?;
    }
    Ok(Some(serde_json::to_string_pretty(&rewritten)?))
}

#[cfg(test)]
mod tests {
    use super::dev_bundle_base_url;

    #[test]
    fn dev_bundle_base_url_preserves_auth_after_the_http_path() {
        assert_eq!(
            dev_bundle_base_url("ws://127.0.0.1:39000").as_deref(),
            Some("http://127.0.0.1:39000/__lingxia/dev")
        );
        assert_eq!(
            dev_bundle_base_url("ws://192.168.1.20:39000/?token=abc").as_deref(),
            Some("http://192.168.1.20:39000/__lingxia/dev?token=abc")
        );
    }
}
