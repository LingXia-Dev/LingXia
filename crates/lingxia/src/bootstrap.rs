use rong_rt::{InstallGlobalExecutorError, RongExecutor};

fn default_runtime_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().min(4))
        .unwrap_or(1)
}

fn install_global_executor() {
    let executor = match RongExecutor::builder()
        .threads(default_runtime_threads())
        .thread_name("lingxia")
        .build()
    {
        Ok(executor) => executor,
        Err(err) => {
            log::warn!("Failed to build dedicated RongExecutor: {}", err);
            return;
        }
    };

    match executor.install_global() {
        Ok(()) => {
            log::info!("Installed dedicated RongExecutor for host async work");
        }
        Err(InstallGlobalExecutorError::AlreadyInstalled) => {}
    }
}

fn load_bundled_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
) -> Option<lingxia_app_context::AppConfig> {
    use lingxia_platform::traits::app_runtime::AppRuntime;
    use std::io::Read;

    let mut reader = match runtime.read_asset("app.json") {
        Ok(reader) => reader,
        Err(e) => {
            log::error!("Failed to read app.json: {}", e);
            return None;
        }
    };
    let mut content = String::new();
    if let Err(e) = reader.read_to_string(&mut content) {
        log::error!("Failed to read app.json: {}", e);
        return None;
    }
    match lingxia_app_context::AppConfig::parse_and_validate(&content) {
        Ok(mut config) => {
            if config.panels.is_none()
                && let Some(panels) = load_panels_from_ui_config(runtime)
            {
                config.panels = Some(panels);
            }
            Some(config)
        }
        Err(e) => {
            log::error!("Failed to load app configuration: {}", e);
            None
        }
    }
}

fn load_panels_from_ui_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
) -> Option<lingxia_app_context::PanelsConfig> {
    use lingxia_platform::traits::app_runtime::AppRuntime;
    use std::io::Read;

    let mut reader = runtime.read_asset("ui.json").ok()?;
    let mut content = String::new();
    reader.read_to_string(&mut content).ok()?;
    let ui = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    panels_from_ui_config(&ui)
}

fn panels_from_ui_config(ui: &serde_json::Value) -> Option<lingxia_app_context::PanelsConfig> {
    use lingxia_app_context::{
        PanelContent, PanelContentKind, PanelItem, PanelPosition, PanelsConfig,
    };

    let surfaces = ui.get("surfaces")?.as_array()?;
    let surfaces_by_id = surfaces
        .iter()
        .filter_map(|surface| {
            let id = surface.get("id")?.as_str()?.trim();
            (!id.is_empty()).then_some((id, surface))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let activators = ui.get("activators")?.as_array()?;
    let mut items = Vec::new();

    for activator in activators {
        if activator.get("kind").and_then(serde_json::Value::as_str) != Some("sidebarItem") {
            continue;
        }
        let id = match activator.get("id").and_then(serde_json::Value::as_str) {
            Some(id) if !id.trim().is_empty() => id.trim(),
            _ => continue,
        };
        let Some(action) = activator.get("action") else {
            continue;
        };
        if !matches!(
            action.get("kind").and_then(serde_json::Value::as_str),
            Some("toggleSurface" | "openSurface")
        ) {
            continue;
        }
        let surface_id = match action.get("surface").and_then(serde_json::Value::as_str) {
            Some(surface_id) if !surface_id.trim().is_empty() => surface_id.trim(),
            _ => continue,
        };
        let Some(surface) = surfaces_by_id.get(surface_id) else {
            continue;
        };
        if surface
            .get("presentation")
            .and_then(|presentation| presentation.get("kind"))
            .and_then(serde_json::Value::as_str)
            != Some("attachPanel")
        {
            continue;
        }
        let Some(content) = surface.get("content") else {
            continue;
        };
        let content_kind = match content
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("lxapp")
        {
            "terminal" => PanelContentKind::Terminal,
            "lxapp" => PanelContentKind::LxApp,
            _ => continue,
        };
        let app_id = content
            .get("appId")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|app_id| !app_id.is_empty());
        if content_kind == PanelContentKind::LxApp && app_id.is_none() {
            continue;
        }

        let label = activator
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(id)
            .to_string();
        let icon = activator
            .get("icon")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let position = surface
            .get("presentation")
            .and_then(|presentation| presentation.get("edge"))
            .and_then(serde_json::Value::as_str)
            .map(panel_position_from_edge)
            .unwrap_or(PanelPosition::Right);
        let path = surface
            .get("content")
            .and_then(|content| content.get("path"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned);

        items.push(PanelItem {
            id: id.to_string(),
            label,
            icon,
            position,
            content: PanelContent {
                kind: content_kind,
                app_id: app_id.unwrap_or_default().to_string(),
                path,
            },
        });
    }

    (!items.is_empty()).then_some(PanelsConfig { items })
}

fn panel_position_from_edge(edge: &str) -> lingxia_app_context::PanelPosition {
    match edge {
        "leading" | "left" => lingxia_app_context::PanelPosition::Left,
        "bottom" => lingxia_app_context::PanelPosition::Bottom,
        _ => lingxia_app_context::PanelPosition::Right,
    }
}

/// Common initialization after Platform is created.
/// Registers built-in runtime and initializes the lxapp system.
pub(crate) fn init_with_platform(platform: lingxia_platform::Platform) -> Option<String> {
    use lingxia_platform::traits::app_runtime::AppRuntime;

    #[cfg(feature = "devtool")]
    let _ = crate::devtool::install_lxapp_dev_config_from_env();
    crate::host_addon::run_before_init();

    let runtime = std::sync::Arc::new(platform.clone());
    crate::runtime::set_platform(runtime.clone());
    #[cfg(feature = "devtool")]
    let app_config = crate::devtool::load_host_app_config(&runtime, load_bundled_app_config)?;
    #[cfg(not(feature = "devtool"))]
    let app_config = load_bundled_app_config(&runtime)?;
    crate::app::set_data_dir(runtime.app_data_dir());
    install_global_executor();
    if let Err(err) = lingxia_app_context::set_app_config(app_config.clone()) {
        log::error!("Failed to initialize app configuration: {}", err);
        return None;
    }
    crate::host_addon::run_install_logic_extensions();
    crate::host_addon::run_install_host_apis();
    crate::browser::register_bundled_app();
    crate::browser::register_builtin_runtime();
    crate::applink::install_handler();
    #[cfg(feature = "standard")]
    lingxia_logic::register_logic_runtime();
    #[cfg(feature = "devtool")]
    crate::devtool::register_bundle_source_override();
    let home_app_id = lxapp::init(platform);
    crate::update::install_auto_trigger(runtime.clone());
    crate::browser::register_builtin_assets();
    crate::host_addon::run_after_init();
    crate::browser::warmup();
    crate::host_addon::run_start_services();
    home_app_id
}

#[cfg(test)]
mod tests {
    use super::panels_from_ui_config;
    use lingxia_app_context::PanelPosition;

    #[test]
    fn derives_lxapp_attach_panels_from_ui_config() {
        let ui = serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "presentation": { "kind": "window" },
                "content": { "kind": "lxapp", "appId": "home" }
            }, {
                "id": "assistant",
                "presentation": {
                    "kind": "attachPanel",
                    "attachTo": "main",
                    "edge": "trailing"
                },
                "content": { "kind": "lxapp", "appId": "lingxia-chat", "path": "pages/chat/index" }
            }],
            "activators": [{
                "id": "assistantSidebar",
                "kind": "sidebarItem",
                "hostSurface": "main",
                "label": "AI Chat",
                "icon": "icons/chat.pdf",
                "action": { "kind": "toggleSurface", "surface": "assistant" }
            }]
        });

        let panels = panels_from_ui_config(&ui).expect("panel config");
        assert_eq!(panels.items.len(), 1);
        assert_eq!(panels.items[0].id, "assistantSidebar");
        assert_eq!(panels.items[0].label, "AI Chat");
        assert_eq!(panels.items[0].icon, "icons/chat.pdf");
        assert_eq!(panels.items[0].position, PanelPosition::Right);
        assert_eq!(panels.items[0].content.app_id, "lingxia-chat");
        assert_eq!(
            panels.items[0].content.path.as_deref(),
            Some("pages/chat/index")
        );
    }

    #[test]
    fn derives_terminal_attach_panels_from_ui_config() {
        let ui = serde_json::json!({
            "surfaces": [{
                "id": "terminal",
                "presentation": {
                    "kind": "attachPanel",
                    "edge": "bottom"
                },
                "content": { "kind": "terminal" }
            }],
            "activators": [{
                "id": "terminalSidebar",
                "kind": "sidebarItem",
                "label": "Terminal",
                "action": { "kind": "toggleSurface", "surface": "terminal" }
            }]
        });

        let panels = panels_from_ui_config(&ui).expect("panel config");
        assert_eq!(panels.items.len(), 1);
        assert_eq!(panels.items[0].id, "terminalSidebar");
        assert_eq!(panels.items[0].label, "Terminal");
        assert_eq!(panels.items[0].position, PanelPosition::Bottom);
        assert_eq!(
            panels.items[0].content.kind,
            lingxia_app_context::PanelContentKind::Terminal
        );
        assert!(panels.items[0].content.app_id.is_empty());
    }
}
