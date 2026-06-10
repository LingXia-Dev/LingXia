use std::path::{Path, PathBuf};
use std::sync::Arc;

use lingxia_platform::traits::app_runtime::{AppRuntime, LxAppOpenMode};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    WindowsChromeEvent, WindowsNavigationBarLayout, WindowsPanelActivatorLayout,
    WindowsPanelPosition, WindowsTabBarItemLayout, WindowsTabBarLayout, WindowsTabBarPosition,
    WindowsWindowLayout, is_panel_visible, set_webview_chrome_event_handler,
    set_webview_window_layout,
};

use super::{LxApp, try_get};
use crate::delegate::{LxAppDelegate, LxAppUiEventType};
use crate::{LxAppStartupOptions, ReleaseType, error, warn};

const DEFAULT_NAV_BAR_HEIGHT: i32 = 38;
const DEFAULT_SIDEBAR_WIDTH: i32 = 180;

pub(crate) fn install_update_handler() {
    lingxia_platform::set_windows_ui_update_handler(Arc::new(|appid| {
        if let Some(lxapp) = try_get(&appid) {
            lxapp.sync_windows_shell_layout();
        }
    }));
}

impl LxApp {
    pub(crate) fn sync_windows_shell_layout(&self) {
        let path = self
            .peek_current_page()
            .unwrap_or_else(|| self.config.get_initial_route());
        if path.is_empty() {
            return;
        }

        let webtag = WebTag::new(&self.appid, &path, Some(self.session_id()));
        let layout = self.build_windows_shell_layout(&path);
        let appid = self.appid.clone();
        set_webview_chrome_event_handler(
            &webtag,
            Arc::new(move |event| handle_windows_chrome_event(&appid, event)),
        );

        if let Err(err) = set_webview_window_layout(&webtag, layout) {
            warn!("Failed to sync Windows shell layout: {}", err)
                .with_appid(self.appid.clone())
                .with_path(path);
        }
    }

    fn build_windows_shell_layout(&self, path: &str) -> WindowsWindowLayout {
        WindowsWindowLayout {
            navigation_bar: Some(self.build_windows_navigation_bar_layout(path)),
            tab_bar: self.build_windows_tab_bar_layout(),
            panel_activators: self.build_windows_panel_activators(),
        }
    }

    fn build_windows_navigation_bar_layout(&self, path: &str) -> WindowsNavigationBarLayout {
        let navbar = self.get_navbar_state(path);
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

    fn build_windows_tab_bar_layout(&self) -> Option<WindowsTabBarLayout> {
        let tabbar = self.get_tabbar()?;
        let position = WindowsTabBarPosition::Left;
        let dimension = tabbar.dimension.max(DEFAULT_SIDEBAR_WIDTH);
        Some(WindowsTabBarLayout {
            visible: !tabbar.list.is_empty(),
            position,
            dimension,
            app_name: self.config.appName.clone(),
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

    fn build_windows_panel_activators(&self) -> Vec<WindowsPanelActivatorLayout> {
        let asset_dir = self.runtime.asset_dir();
        lingxia_app_context::app_config()
            .and_then(|config| config.panels.as_ref().cloned())
            .map(|panels| {
                panels
                    .items
                    .into_iter()
                    .map(|item| {
                        let active = is_panel_visible(&item.id);
                        WindowsPanelActivatorLayout {
                            id: item.id,
                            label: item.label,
                            icon_path: resolve_windows_asset_path(asset_dir, &item.icon)
                                .map(|path| path.to_string_lossy().to_string())
                                .unwrap_or(item.icon),
                            position: windows_panel_position(item.position),
                            active,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn handle_windows_chrome_event(appid: &str, event: WindowsChromeEvent) {
    let Some(lxapp) = try_get(appid) else {
        return;
    };

    let handled = match event {
        WindowsChromeEvent::TabBarClick { index } => {
            lxapp.on_lxapp_event(LxAppUiEventType::TabBarClick, index.to_string())
        }
        WindowsChromeEvent::NavigationBack => {
            lxapp.on_lxapp_event(LxAppUiEventType::NavigationClick, "back".to_string())
        }
        WindowsChromeEvent::NavigationHome => {
            lxapp.on_lxapp_event(LxAppUiEventType::NavigationClick, "home".to_string())
        }
        WindowsChromeEvent::PanelActivatorClick { panel_id } => {
            return handle_windows_panel_activator(appid, panel_id);
        }
    };

    if handled {
        lxapp.sync_windows_shell_layout();
    } else {
        error!("Windows shell chrome event was not handled").with_appid(appid.to_string());
    }
}

fn handle_windows_panel_activator(appid: &str, panel_id: String) {
    let Some((panel_appid, path)) = panel_item_for_id(&panel_id) else {
        error!("Windows panel activator was not found")
            .with_appid(appid.to_string())
            .with_path(panel_id);
        return;
    };

    if is_panel_visible(&panel_id) {
        if let Some(panel) = try_get(&panel_appid)
            && let Err(err) = panel
                .runtime
                .hide_lxapp(panel_appid.clone(), panel.session_id())
        {
            error!("Failed to close Windows panel lxapp: {}", err).with_appid(panel_appid);
        }
        if let Some(owner) = try_get(appid) {
            owner.sync_windows_shell_layout();
        }
        return;
    }

    let owner_appid = appid.to_string();
    crate::executor::spawn(async move {
        if let Err(err) = open_panel_lxapp(&panel_id, &panel_appid, &path).await {
            error!("Failed to open Windows panel lxapp: {}", err).with_appid(panel_appid);
            return;
        }
        if let Some(owner) = try_get(&owner_appid) {
            owner.sync_windows_shell_layout();
        }
    });
}

fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id))
        .map(|item| (item.content.app_id, item.content.path.unwrap_or_default()))
}

async fn open_panel_lxapp(
    panel_id: &str,
    appid: &str,
    path: &str,
) -> Result<(), crate::LxAppError> {
    crate::prepare_lxapp_open(appid, ReleaseType::Release).await?;
    let _ = crate::open_lxapp(
        appid,
        LxAppStartupOptions::new(path)
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(panel_id.to_string()),
    )?;
    crate::schedule_lxapp_update_check(appid, ReleaseType::Release);
    Ok(())
}

fn windows_panel_position(position: lingxia_app_context::PanelPosition) -> WindowsPanelPosition {
    match position {
        lingxia_app_context::PanelPosition::Left => WindowsPanelPosition::Left,
        lingxia_app_context::PanelPosition::Right => WindowsPanelPosition::Right,
        lingxia_app_context::PanelPosition::Bottom => WindowsPanelPosition::Bottom,
    }
}

fn resolve_windows_asset_path(asset_dir: &Path, raw: &str) -> Option<PathBuf> {
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
