use std::sync::Arc;

use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    WindowsChromeEvent, WindowsNavigationBarLayout, WindowsTabBarItemLayout, WindowsTabBarLayout,
    WindowsTabBarPosition, WindowsWindowLayout, set_webview_chrome_event_handler,
    set_webview_window_layout,
};

use super::tabbar::TabBarPosition;
use super::{LxApp, try_get};
use crate::delegate::{LxAppDelegate, LxAppUiEventType};
use crate::{error, warn};

const DEFAULT_NAV_BAR_HEIGHT: i32 = 48;
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
        }
    }

    fn build_windows_navigation_bar_layout(&self, path: &str) -> WindowsNavigationBarLayout {
        let navbar = self.get_navbar_state(path);
        let text_color = match navbar.navigationBarTextStyle.as_str() {
            "white" => 0xffffff,
            _ => 0x111111,
        };
        let title = if navbar.navigationBarTitleText.trim().is_empty() {
            self.config.appName.clone()
        } else {
            navbar.navigationBarTitleText
        };

        WindowsNavigationBarLayout {
            visible: navbar.show_navbar,
            title,
            background_color: parse_css_color(&navbar.navigationBarBackgroundColor, 0xffffff),
            text_color,
            show_back_button: navbar.show_back_button,
            show_home_button: navbar.show_home_button,
            height: DEFAULT_NAV_BAR_HEIGHT,
        }
    }

    fn build_windows_tab_bar_layout(&self) -> Option<WindowsTabBarLayout> {
        let tabbar = self.get_tabbar()?;
        let position = windows_tab_bar_position(&tabbar.position);
        let dimension = match position {
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right => {
                tabbar.dimension.max(DEFAULT_SIDEBAR_WIDTH)
            }
            WindowsTabBarPosition::Bottom => tabbar.dimension,
        };
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
                    badge: item.badge,
                    has_red_dot: item.has_red_dot,
                })
                .collect(),
        })
    }
}

fn windows_tab_bar_position(position: &TabBarPosition) -> WindowsTabBarPosition {
    match position {
        TabBarPosition::Bottom => WindowsTabBarPosition::Bottom,
        TabBarPosition::Left => WindowsTabBarPosition::Left,
        TabBarPosition::Right => WindowsTabBarPosition::Right,
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
    };

    if handled {
        lxapp.sync_windows_shell_layout();
    } else {
        error!("Windows shell chrome event was not handled").with_appid(appid.to_string());
    }
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
