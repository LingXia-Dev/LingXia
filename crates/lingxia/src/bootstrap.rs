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
        // rong_rt offers no way to probe for an installed global executor, so
        // discarding the freshly built executor on AlreadyInstalled is accepted.
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
            if let Some(mut generated) = load_panels_from_ui_config(runtime) {
                if let Some(existing) = config.panels.as_mut() {
                    let mut ids = existing
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<std::collections::HashSet<_>>();
                    existing.items.extend(
                        generated
                            .items
                            .drain(..)
                            .filter(|item| ids.insert(item.id.clone())),
                    );
                } else {
                    config.panels = Some(generated);
                }
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
    use lingxia_app_context::PanelsConfig;

    let surfaces = ui.get("surfaces")?.as_array()?;
    let activators_by_surface = ui
        .get("activators")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|activator| {
            let surface = activator.get("action")?.get("surface")?.as_str()?.trim();
            (!surface.is_empty()).then_some((surface, activator))
        })
        .collect::<std::collections::HashMap<_, _>>();

    let items = surfaces
        .iter()
        .filter_map(|surface| {
            let id = surface.get("id")?.as_str()?.trim();
            panel_item_from_surface(id, surface, activators_by_surface.get(id).copied())
        })
        .collect::<Vec<_>>();

    (!items.is_empty()).then_some(PanelsConfig { items })
}

fn panel_item_from_surface(
    surface_id: &str,
    surface: &serde_json::Value,
    activator: Option<&serde_json::Value>,
) -> Option<lingxia_app_context::PanelItem> {
    use lingxia_app_context::{PanelContent, PanelContentKind, PanelItem};

    if surface_id.is_empty()
        || surface.get("role").and_then(serde_json::Value::as_str) != Some("aside")
    {
        return None;
    }
    let content = surface.get("content")?;
    let content_kind = match content
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
    {
        Some("terminal") => PanelContentKind::Terminal,
        Some("lxapp") => PanelContentKind::LxApp,
        _ => return None,
    };
    let app_id = content
        .get("appId")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|app_id| !app_id.is_empty());
    if content_kind == PanelContentKind::LxApp && app_id.is_none() {
        return None;
    }

    let label = activator
        .and_then(|activator| activator.get("label"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| app_id.unwrap_or(surface_id))
        .to_string();
    let icon = activator
        .and_then(|activator| activator.get("icon"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let position = surface
        .get("edge")
        .and_then(serde_json::Value::as_str)
        .and_then(panel_position_from_edge)?;
    let path = content
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned);

    Some(PanelItem {
        id: surface_id.to_string(),
        label,
        icon,
        position,
        content: PanelContent {
            kind: content_kind,
            app_id: app_id.unwrap_or_default().to_string(),
            path,
        },
    })
}

fn panel_position_from_edge(edge: &str) -> Option<lingxia_app_context::PanelPosition> {
    match edge.trim() {
        "left" => Some(lingxia_app_context::PanelPosition::Left),
        "top" => Some(lingxia_app_context::PanelPosition::Top),
        "bottom" => Some(lingxia_app_context::PanelPosition::Bottom),
        "right" => Some(lingxia_app_context::PanelPosition::Right),
        _ => None,
    }
}

const RUNNER_DISPLAY_LANGUAGE_ENV: &str = "LINGXIA_RUNNER_DISPLAY_LANGUAGE";

fn resolved_display_language_seed(
    saved: Option<String>,
    runner_override: Option<&str>,
) -> Option<String> {
    match runner_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some("auto") => None,
        Some(language) => Some(language.to_string()),
        None => saved,
    }
}

fn seed_display_language(app_data_dir: &std::path::Path) {
    match lingxia_service::settings::display_language(app_data_dir) {
        Ok(saved) => lxapp::set_display_language(resolved_display_language_seed(
            saved,
            std::env::var(RUNNER_DISPLAY_LANGUAGE_ENV).ok().as_deref(),
        )),
        Err(error) => log::warn!("Failed to load display language: {error}"),
    }
}

/// Common initialization after Platform is created.
/// Registers built-in runtime and initializes the lxapp system.
pub(crate) fn init_with_platform(
    platform: lingxia_platform::Platform,
) -> crate::Result<crate::RuntimeInfo> {
    use lingxia_platform::traits::app_runtime::AppRuntime;

    crate::host_addon::run_before_init();

    let runtime = std::sync::Arc::new(platform.clone());
    crate::runtime::set_platform(runtime.clone());
    #[cfg(feature = "devtool")]
    let app_config = crate::devtool::prepare_host_app_config(&runtime, load_bundled_app_config)
        .ok_or_else(|| crate::Error::internal("failed to load host app configuration"))?;
    #[cfg(not(feature = "devtool"))]
    let app_config = load_bundled_app_config(&runtime)
        .ok_or_else(|| crate::Error::internal("failed to load host app configuration"))?;
    crate::app::set_data_dir(runtime.app_data_dir());
    seed_display_language(&runtime.app_data_dir());
    install_global_executor();
    if let Err(err) = lingxia_app_context::set_app_config(app_config.clone()) {
        return Err(crate::Error::internal(format!(
            "failed to initialize app configuration: {err}"
        )));
    }
    // App config (with the device dev-ws-url) is now loaded, so a dev session is
    // detectable: default logging to debug unless LINGXIA_LOG_LEVEL pinned it.
    crate::logging::apply_dev_session_level();
    #[cfg(feature = "devtool")]
    crate::devtool::prepare_bundle_sources(&runtime);
    crate::host_addon::run_install_logic_extensions();
    crate::host_addon::run_install_host_apis();
    crate::browser::register_bundled_app();
    crate::browser::register_builtin_runtime();
    crate::applink::install_handler();
    #[cfg(feature = "standard")]
    lingxia_logic::register_logic_runtime();
    #[cfg(feature = "automation")]
    lingxia_automation::register_automation_runtime();
    let home_app_id = lxapp::init(platform)?;
    if let Err(error) = crate::shell::initialize(runtime.clone()) {
        log::error!("Failed to initialize host shell state: {error}");
    }
    crate::update::install_auto_trigger(runtime.clone());
    crate::browser::register_builtin_assets();
    crate::host_addon::run_after_init();
    crate::browser::warmup();
    crate::host_addon::run_start_services();
    Ok(crate::RuntimeInfo::new(home_app_id))
}

#[cfg(test)]
mod tests {
    use super::{panels_from_ui_config, resolved_display_language_seed, seed_display_language};
    use lingxia_app_context::PanelPosition;

    #[test]
    fn seeds_saved_display_language_before_runtime_init() {
        let dir = tempfile::tempdir().expect("temp app data");
        lingxia_service::settings::set_display_language(dir.path(), Some("zh-CN"))
            .expect("save display language");

        seed_display_language(dir.path());

        assert_eq!(crate::app::display_language(), "zh-CN");
        lxapp::set_display_language(None);
    }

    #[test]
    fn runner_display_language_override_is_session_scoped() {
        assert_eq!(
            resolved_display_language_seed(Some("zh-CN".to_string()), Some("en-US")),
            Some("en-US".to_string())
        );
        assert_eq!(
            resolved_display_language_seed(Some("zh-CN".to_string()), Some("auto")),
            None
        );
        assert_eq!(
            resolved_display_language_seed(Some("zh-CN".to_string()), None),
            Some("zh-CN".to_string())
        );
    }

    #[test]
    fn derives_lxapp_aside_panels_from_ui_config() {
        let ui = serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": { "kind": "lxapp", "appId": "home" }
            }, {
                "id": "assistant",
                "role": "aside",
                "attachTo": "main",
                "edge": "right",
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
        assert_eq!(panels.items[0].id, "assistant");
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
    fn derives_declared_asides_without_yaml_activators() {
        let ui = serde_json::json!({
            "surfaces": [{
                "id": "assistant",
                "role": "aside",
                "edge": "right",
                "content": { "kind": "lxapp", "appId": "lingxia-chat" }
            }, {
                "id": "terminal",
                "role": "aside",
                "edge": "bottom",
                "content": { "kind": "terminal" }
            }],
            "activators": []
        });

        let panels = panels_from_ui_config(&ui).expect("panel config");
        assert_eq!(panels.items.len(), 2);
        assert_eq!(panels.items[0].id, "assistant");
        assert_eq!(panels.items[0].label, "lingxia-chat");
        assert_eq!(panels.items[1].id, "terminal");
        assert_eq!(panels.items[1].label, "terminal");
    }

    #[test]
    fn derives_terminal_aside_panels_from_ui_config() {
        let ui = serde_json::json!({
            "surfaces": [{
                "id": "terminal",
                "role": "aside",
                "edge": "bottom",
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
        assert_eq!(panels.items[0].id, "terminal");
        assert_eq!(panels.items[0].label, "Terminal");
        assert_eq!(panels.items[0].position, PanelPosition::Bottom);
        assert_eq!(
            panels.items[0].content.kind,
            lingxia_app_context::PanelContentKind::Terminal
        );
        assert!(panels.items[0].content.app_id.is_empty());
    }

    #[test]
    fn derives_adaptive_aside_panels_from_ui_config() {
        let ui = serde_json::json!({
            "surfaces": [{
                "id": "lingxia-chat",
                "role": "aside",
                "edge": "right",
                "content": { "kind": "lxapp", "appId": "lingxia-chat" }
            }, {
                "id": "terminal",
                "role": "aside",
                "edge": "bottom",
                "content": { "kind": "terminal" }
            }],
            "activators": [{
                "id": "lingxia-chatSidebar",
                "kind": "sidebarItem",
                "label": "AI Chat",
                "icon": "icons/chat-8f2cc4f0240a.png",
                "action": { "kind": "toggleSurface", "surface": "lingxia-chat" }
            }, {
                "id": "terminalSidebar",
                "kind": "sidebarItem",
                "label": "Terminal",
                "icon": "icons/terminal-00c8810c398d.png",
                "action": { "kind": "toggleSurface", "surface": "terminal" }
            }]
        });

        let panels = panels_from_ui_config(&ui).expect("panel config");
        assert_eq!(panels.items.len(), 2);
        assert_eq!(panels.items[0].id, "lingxia-chat");
        assert_eq!(panels.items[0].icon, "icons/chat-8f2cc4f0240a.png");
        assert_eq!(panels.items[0].position, PanelPosition::Right);
        assert_eq!(panels.items[0].content.app_id, "lingxia-chat");
        assert_eq!(panels.items[1].id, "terminal");
        assert_eq!(panels.items[1].icon, "icons/terminal-00c8810c398d.png");
        assert_eq!(panels.items[1].position, PanelPosition::Bottom);
        assert_eq!(
            panels.items[1].content.kind,
            lingxia_app_context::PanelContentKind::Terminal
        );
    }

    #[test]
    fn derives_top_attach_panel_edge_from_ui_config() {
        let ui = serde_json::json!({
            "surfaces": [{
                "id": "logs",
                "role": "aside",
                "edge": "top",
                "content": { "kind": "lxapp", "appId": "logs" }
            }],
            "activators": [{
                "id": "logsSidebar",
                "kind": "sidebarItem",
                "label": "Logs",
                "action": { "kind": "toggleSurface", "surface": "logs" }
            }]
        });

        let panels = panels_from_ui_config(&ui).expect("panel config");
        assert_eq!(panels.items.len(), 1);
        assert_eq!(panels.items[0].id, "logs");
        assert_eq!(panels.items[0].position, PanelPosition::Top);
    }
}
