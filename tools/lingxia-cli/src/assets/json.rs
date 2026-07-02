use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, ResolvedEnv};
use anyhow::{Result, anyhow};
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
/// - `envVersion` is always emitted (defaults to `release`).
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
    if let Some(windows_app_id) = config
        .windows
        .as_ref()
        .and_then(|windows| windows.app_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let resolved_id = match resolved_env.effective_package_id_suffix() {
            Some(suffix) => format!("{windows_app_id}{suffix}"),
            None => windows_app_id.to_string(),
        };
        obj.insert("windowsAppId".to_string(), serde_json::json!(resolved_id));
    }
    obj.insert(
        "envVersion".to_string(),
        serde_json::json!(resolved_env.version.as_str()),
    );

    if let Some(home_bundle) = home_bundle {
        obj.insert("homeAppId".to_string(), serde_json::json!(app.home_app_id));
        obj.insert(
            "homeAppVersion".to_string(),
            serde_json::json!(home_bundle.version.as_str()),
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
    if let Some(app_links) = config.app_links.as_ref()
        && !app_links.hosts.is_empty()
    {
        obj.insert(
            "appLinks".to_string(),
            serde_json::json!({ "hosts": app_links.hosts }),
        );
    }
    if let Some(capabilities) = config.capabilities.as_ref() {
        obj.insert(
            "capabilities".to_string(),
            serde_json::to_value(capabilities)?,
        );
    }

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(
        obj,
    ))?)
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
    Some(format!("{scheme}://{rest}/__lingxia/dev"))
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
