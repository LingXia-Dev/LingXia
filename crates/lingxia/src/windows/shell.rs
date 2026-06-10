use std::path::{Path, PathBuf};
use std::sync::Arc;

use lingxia_platform::traits::app_runtime::{AppRuntime, LxAppOpenMode};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    WindowsChromeEvent, WindowsNavigationBarLayout, WindowsPanelActivatorLayout,
    WindowsPanelPosition, WindowsTabBarItemLayout, WindowsTabBarLayout, WindowsTabBarPosition,
    WindowsWindowLayout, hide_panel, is_panel_visible, set_webview_chrome_event_handler,
    set_webview_window_layout,
};
use lxapp::{LxApp, LxAppDelegate, LxAppStartupOptions, LxAppUiEventType, ReleaseType};

const DEFAULT_NAV_BAR_HEIGHT: i32 = 38;
const DEFAULT_SIDEBAR_WIDTH: i32 = 180;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalPanelRequest {
    panel_id: String,
    label: String,
    position: lingxia_app_context::PanelPosition,
}

enum PanelTarget {
    LxApp { appid: String, path: String },
    Terminal(TerminalPanelRequest),
}

pub(super) fn install() {
    lingxia_platform::set_windows_ui_update_handler(Arc::new(|appid| {
        sync_shell_layout(&appid);
    }));
}

fn sync_shell_layout(appid: &str) {
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };
    let path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    if path.is_empty() {
        return;
    }

    let webtag = WebTag::new(&app.appid, &path, Some(app.session_id()));
    let layout = build_window_layout(&app, &path);
    let event_appid = app.appid.clone();
    set_webview_chrome_event_handler(
        &webtag,
        Arc::new(move |event| {
            handle_chrome_event(&event_appid, event);
        }),
    );

    if let Err(err) = set_webview_window_layout(&webtag, layout) {
        log::warn!(
            "failed to sync Windows shell layout for {}:{}: {}",
            appid,
            path,
            err
        );
    }
}

fn build_window_layout(app: &LxApp, path: &str) -> WindowsWindowLayout {
    WindowsWindowLayout {
        navigation_bar: Some(build_navigation_bar_layout(app, path)),
        tab_bar: build_tab_bar_layout(app),
        panel_activators: build_panel_activators(app),
    }
}

fn build_navigation_bar_layout(app: &LxApp, path: &str) -> WindowsNavigationBarLayout {
    let navbar = app.get_navbar_state(path);
    let text_color = match navbar.navigationBarTextStyle.as_str() {
        "white" => 0xffffff,
        _ => 0x111111,
    };
    WindowsNavigationBarLayout {
        visible: navbar.show_navbar,
        title: navbar.navigationBarTitleText,
        background_color: parse_css_color(&navbar.navigationBarBackgroundColor, 0xffffff),
        text_color,
        show_back_button: navbar.show_back_button,
        show_home_button: navbar.show_home_button,
        height: DEFAULT_NAV_BAR_HEIGHT,
    }
}

fn build_tab_bar_layout(app: &LxApp) -> Option<WindowsTabBarLayout> {
    let tabbar = app.get_tabbar()?;
    Some(WindowsTabBarLayout {
        visible: !tabbar.list.is_empty(),
        position: WindowsTabBarPosition::Left,
        dimension: tabbar.dimension.max(DEFAULT_SIDEBAR_WIDTH),
        app_name: app.runtime_info().app_name,
        color: parse_css_color(&tabbar.color, 0x666666),
        selected_color: parse_css_color(&tabbar.selectedColor, 0x1677ff),
        background_color: parse_css_color(&tabbar.backgroundColor, 0xffffff),
        border_color: parse_css_color(&tabbar.borderStyle, 0xf0f0f0),
        selected_index: tabbar.get_selected_index(),
        items: tabbar
            .list
            .into_iter()
            .map(|item| WindowsTabBarItemLayout {
                page_path: item.pagePath,
                text: item.text.unwrap_or_default(),
                icon_path: item.iconPath.unwrap_or_default(),
                selected_icon_path: item.selectedIconPath.unwrap_or_default(),
                badge: item.badge,
                has_red_dot: item.has_red_dot,
            })
            .collect(),
    })
}

fn build_panel_activators(app: &LxApp) -> Vec<WindowsPanelActivatorLayout> {
    let asset_dir = app.runtime.asset_dir();
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .map(|panels| {
            panels
                .items
                .into_iter()
                .map(|item| WindowsPanelActivatorLayout {
                    id: item.id.clone(),
                    label: item.label,
                    icon_path: resolve_asset_path(asset_dir, &item.icon)
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or(item.icon),
                    position: panel_position(item.position),
                    active: is_panel_visible(&item.id),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn handle_chrome_event(appid: &str, event: WindowsChromeEvent) {
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };

    let handled = match event {
        WindowsChromeEvent::TabBarClick { index } => {
            app.on_lxapp_event(LxAppUiEventType::TabBarClick, index.to_string())
        }
        WindowsChromeEvent::NavigationBack => {
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "back".to_string())
        }
        WindowsChromeEvent::NavigationHome => {
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "home".to_string())
        }
        WindowsChromeEvent::PanelActivatorClick { panel_id } => {
            handle_panel_activator(appid, panel_id);
            true
        }
    };

    if handled {
        sync_shell_layout(appid);
    } else {
        log::error!("Windows shell chrome event was not handled for {appid}");
    }
}

fn handle_panel_activator(appid: &str, panel_id: String) {
    let Some(target) = panel_target_for_id(&panel_id) else {
        log::error!("Windows panel activator was not found: {panel_id}");
        return;
    };

    let (panel_appid, path) = match target {
        PanelTarget::LxApp { appid, path } => (appid, path),
        PanelTarget::Terminal(request) => {
            handle_terminal_panel_activator(appid, request);
            return;
        }
    };

    if is_panel_visible(&panel_id) {
        if let Some(panel) = lxapp::try_get(&panel_appid)
            && let Err(err) = panel
                .runtime
                .hide_lxapp(panel_appid.clone(), panel.session_id())
        {
            log::error!("failed to close Windows panel lxapp {panel_appid}: {err}");
        }
        if let Err(err) = hide_panel(&panel_id) {
            log::warn!("failed to hide Windows panel {panel_id}: {err}");
        }
        lxapp::mark_lxapp_active(appid);
        sync_shell_layout(appid);
        return;
    }

    let owner_appid = appid.to_string();
    std::mem::drop(crate::task::spawn(async move {
        if let Err(err) = open_panel_lxapp(&panel_id, &panel_appid, &path).await {
            log::error!("failed to open Windows panel lxapp {panel_appid}: {err}");
            return;
        }
        sync_shell_layout(&owner_appid);
    }));
}

fn panel_target_for_id(panel_id: &str) -> Option<PanelTarget> {
    let item = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id))?;

    if item.content.kind.is_lxapp() {
        Some(PanelTarget::LxApp {
            appid: item.content.app_id,
            path: item.content.path.unwrap_or_default(),
        })
    } else {
        Some(PanelTarget::Terminal(TerminalPanelRequest {
            panel_id: item.id,
            label: item.label,
            position: item.position,
        }))
    }
}

fn handle_terminal_panel_activator(appid: &str, request: TerminalPanelRequest) {
    let position = panel_position(request.position);
    if is_panel_visible(&request.panel_id) {
        if let Err(err) = super::close_windows_terminal_panel(&request.panel_id) {
            log::warn!(
                "failed to hide Windows terminal panel {}: {}",
                request.panel_id,
                err
            );
        }
        sync_shell_layout(appid);
        return;
    }

    let title = if request.label.trim().is_empty() {
        "Terminal"
    } else {
        request.label.trim()
    };
    if let Err(err) = super::open_windows_terminal_panel(&request.panel_id, title, position) {
        log::warn!(
            "failed to show Windows terminal panel {}: {}",
            request.panel_id,
            err
        );
    }
    sync_shell_layout(appid);
}

async fn open_panel_lxapp(
    panel_id: &str,
    appid: &str,
    path: &str,
) -> Result<(), lxapp::LxAppError> {
    lxapp::prepare_lxapp_open(appid, ReleaseType::Release).await?;
    let _ = lxapp::open_lxapp(
        appid,
        LxAppStartupOptions::new(path)
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(panel_id.to_string()),
    )?;
    lxapp::schedule_lxapp_update_check(appid, ReleaseType::Release);
    Ok(())
}

fn panel_position(position: lingxia_app_context::PanelPosition) -> WindowsPanelPosition {
    match position {
        lingxia_app_context::PanelPosition::Left => WindowsPanelPosition::Left,
        lingxia_app_context::PanelPosition::Right => WindowsPanelPosition::Right,
        lingxia_app_context::PanelPosition::Bottom => WindowsPanelPosition::Bottom,
    }
}

fn resolve_asset_path(asset_dir: &Path, raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }

    Some(asset_dir.join(path))
}

fn parse_css_color(raw: &str, fallback: u32) -> u32 {
    let value = raw.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("transparent") {
        return fallback;
    }
    match value.to_ascii_lowercase().as_str() {
        "black" => return 0x000000,
        "white" => return 0xffffff,
        "red" => return 0xff0000,
        "blue" => return 0x0000ff,
        "green" => return 0x008000,
        _ => {}
    }

    let hex = value.strip_prefix('#').unwrap_or(value);
    let rgb = match hex.len() {
        3 => {
            let expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            u32::from_str_radix(&expanded, 16).ok()
        }
        6 => u32::from_str_radix(hex, 16).ok(),
        8 => u32::from_str_radix(&hex[2..], 16).ok(),
        _ => None,
    };
    rgb.unwrap_or(fallback)
}
