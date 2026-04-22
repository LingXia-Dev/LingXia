use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

use super::bundles::PreparedResourceBundle;
use super::icons::{PreparedAppUiIcon, rewrite_app_ui_icon_paths};

pub(super) fn build_app_json_from_config(
    config: &LingXiaConfig,
    home_bundle: Option<&PreparedResourceBundle>,
    dev_ws_url: Option<&str>,
) -> Result<String> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;
    let lingxia_server = app.lingxia_server.as_deref();

    let mut obj = serde_json::Map::new();
    obj.insert(
        "productName".to_string(),
        serde_json::json!(app.product_name),
    );
    obj.insert(
        "productVersion".to_string(),
        serde_json::json!(app.product_version),
    );

    if let Some(lingxia_server) = lingxia_server.filter(|s| !s.is_empty()) {
        obj.insert(
            "lingxiaServer".to_string(),
            serde_json::json!(lingxia_server),
        );
    }
    if let Some(lingxia_id) = app.lingxia_id.as_deref().filter(|s| !s.is_empty()) {
        obj.insert("lingxiaId".to_string(), serde_json::json!(lingxia_id));
    }

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

pub(super) fn build_ui_json_from_config(
    config: &LingXiaConfig,
    app_ui_icons: &[PreparedAppUiIcon],
) -> Result<Option<String>> {
    let Some(ui) = config.ui.as_ref() else {
        return Ok(None);
    };
    let mut rewritten = ui.clone();
    if !app_ui_icons.is_empty() {
        let by_source = app_ui_icons
            .iter()
            .map(|icon| (icon.source_path.as_str(), icon.relative_path.as_str()))
            .collect::<HashMap<_, _>>();
        rewrite_app_ui_icon_paths(&mut rewritten, &by_source)?;
    }
    Ok(Some(serde_json::to_string_pretty(&rewritten)?))
}
