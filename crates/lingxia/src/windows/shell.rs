use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_platform::traits::app_runtime::{
    AppRuntime, LxAppOpenMode, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_platform::windows::webview_host::{
    WindowsChromeCommand, WindowsPanelPosition, WindowsWindowLayout, hide_host_panel,
    is_panel_visible, present_webview_in_active_group, restore_presented_group_main,
    set_webview_chrome_event_handler, set_webview_window_layout,
};
#[cfg(feature = "shell-runtime")]
use lingxia_shell::windows::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout,
};
use lingxia_webview::WebTag;
use lxapp::{LxApp, LxAppDelegate, LxAppStartupOptions, LxAppUiEventType, ReleaseType};
#[cfg(not(feature = "shell-runtime"))]
use shell_layout_fallback::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout,
};

#[cfg(not(feature = "shell-runtime"))]
#[allow(dead_code)]
mod shell_layout_fallback {
    use std::sync::Arc;

    use lingxia_platform::windows::webview_host::WindowsPanelPosition;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum WindowsShellTabBarPosition {
        #[default]
        Bottom,
        Left,
        Right,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellNavigationBarLayout {
        pub visible: bool,
        pub title: String,
        pub background_color: u32,
        pub text_color: u32,
        pub show_back_button: bool,
        pub show_home_button: bool,
        pub height: i32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellTabBarItemLayout {
        pub page_path: String,
        pub text: String,
        pub icon_path: String,
        pub selected_icon_path: String,
        pub badge: Option<String>,
        pub has_red_dot: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellAuxiliaryItemLayout {
        pub id: String,
        pub title: String,
        pub active: bool,
        pub icon_png: Option<Arc<Vec<u8>>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellHeaderActionLayout {
        pub id: String,
        pub glyph: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellTabBarLayout {
        pub visible: bool,
        pub position: WindowsShellTabBarPosition,
        pub dimension: i32,
        pub app_name: String,
        pub group_id: String,
        pub color: u32,
        pub selected_color: u32,
        pub background_color: u32,
        pub border_color: u32,
        pub selected_index: i32,
        pub items: Vec<WindowsShellTabBarItemLayout>,
        pub collapsed: bool,
        pub items_collapsed: bool,
        pub auxiliary_items: Vec<WindowsShellAuxiliaryItemLayout>,
        pub show_auxiliary_add: bool,
        pub header_actions: Vec<WindowsShellHeaderActionLayout>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct WindowsShellAddressBarLayout {
        pub visible: bool,
        pub url_text: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsShellPanelActivatorLayout {
        pub id: String,
        pub label: String,
        pub icon_path: String,
        pub position: WindowsPanelPosition,
        pub active: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct WindowsShellWindowLayout {
        pub navigation_bar: Option<WindowsShellNavigationBarLayout>,
        pub address_bar: Option<WindowsShellAddressBarLayout>,
        pub tab_bar: Option<WindowsShellTabBarLayout>,
        pub panel_activators: Vec<WindowsShellPanelActivatorLayout>,
    }
}

const DEFAULT_NAV_BAR_HEIGHT: i32 = 38;
const MIN_SIDEBAR_WIDTH: i32 = 180;

/// How many times to retry presenting a freshly opened browser tab whose
/// WebView creation is still in flight, and the delay between attempts.
const PRESENT_BROWSER_TAB_MAX_RETRY: u32 = 30;
const PRESENT_BROWSER_TAB_RETRY_DELAY_MS: u64 = 100;

/// Panel ids whose lxapp open is still in flight, used to ignore repeated
/// activator clicks until the open completes.
static PENDING_PANEL_OPENS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// The lxapp that owns the main shell window (set when the home app opens
/// and refreshed on every chrome event); browser tab-change notifications
/// re-sync this app's layout.
static SHELL_OWNER_APPID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Browser tab currently presented over the main content card, if any.
static PRESENTED_BROWSER_TAB: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Sidebar header action ids and their browser targets.
const SIDEBAR_ACTION_SETTINGS: &str = "settings";
const SIDEBAR_ACTION_DOWNLOADS: &str = "downloads";
const SETTINGS_PAGE_URL: &str = "lingxia://settings";
const DOWNLOADS_PAGE_URL: &str = "lingxia://downloads";

/// Segoe Fluent Icons glyphs of the sidebar header actions (passed through
/// layout data so the webview layer stays product-agnostic).
const GLYPH_SETTINGS: &str = "\u{e713}";
const GLYPH_DOWNLOAD: &str = "\u{e896}";

mod chrome_command {
    pub(super) const TAB_BAR_CLICK: &str = "tabbar.click";
    pub(super) const PANEL_ACTIVATOR_CLICK: &str = "panel-activator.click";
    pub(super) const NAVIGATION_BACK: &str = "navigation.back";
    pub(super) const NAVIGATION_HOME: &str = "navigation.home";
    pub(super) const BROWSER_NEW_TAB: &str = "browser.new-tab";
    pub(super) const BROWSER_TAB_CLICK: &str = "browser.tab.click";
    pub(super) const BROWSER_TAB_CLOSE: &str = "browser.tab.close";
    pub(super) const NATIVE_PANEL_TAB_CLICK: &str = "native-panel.tab.click";
    pub(super) const NATIVE_PANEL_TAB_CLOSE: &str = "native-panel.tab.close";
    pub(super) const NATIVE_PANEL_NEW_TAB: &str = "native-panel.new-tab";
    pub(super) const NATIVE_PANEL_MAXIMIZE: &str = "native-panel.maximize";
    pub(super) const NATIVE_PANEL_TAB_RENAME: &str = "native-panel.tab.rename";
    pub(super) const NATIVE_PANEL_RIGHT_CLICK: &str = "native-panel.right-click";
    pub(super) const BROWSER_NAV_BACK: &str = "browser.nav.back";
    pub(super) const BROWSER_NAV_FORWARD: &str = "browser.nav.forward";
    pub(super) const BROWSER_NAV_RELOAD: &str = "browser.nav.reload";
    pub(super) const BROWSER_ADDRESS_BAR: &str = "browser.address-bar";
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
}

/// Per-group (per shell-owner lxapp) sidebar UI state, kept for the
/// session: whole-sidebar collapse and the lxapp items-group collapse.
#[derive(Debug, Clone, Copy, Default)]
struct SidebarUiState {
    collapsed: bool,
    items_collapsed: bool,
}

static SIDEBAR_UI_STATE: OnceLock<Mutex<HashMap<String, SidebarUiState>>> = OnceLock::new();

fn sidebar_ui_state(group: &str) -> SidebarUiState {
    SIDEBAR_UI_STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.get(group).copied())
        .unwrap_or_default()
}

fn update_sidebar_ui_state(group: &str, update: impl FnOnce(&mut SidebarUiState)) {
    let state = SIDEBAR_UI_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        update(state.entry(group.to_string()).or_default());
    }
}

fn pending_panel_opens() -> std::sync::MutexGuard<'static, HashSet<String>> {
    PENDING_PANEL_OPENS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        // The pending set has no invariants that poisoning can break.
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(super) fn set_shell_owner_appid(appid: &str) {
    let slot = SHELL_OWNER_APPID.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(appid.to_string());
    }
}

fn shell_owner_appid() -> Option<String> {
    SHELL_OWNER_APPID
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

/// Re-syncs the shell-owner app's layout (panel activator states etc.)
/// after a panel changed visibility outside a chrome event, e.g. the
/// terminal panel closing itself because its last session exited (the
/// only caller, hence unused without the terminal runtime).
#[cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]
pub(super) fn sync_owner_shell_layout() {
    if let Some(appid) = shell_owner_appid() {
        sync_shell_layout(&appid);
    }
}

fn presented_browser_tab() -> Option<String> {
    PRESENTED_BROWSER_TAB
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

fn set_presented_browser_tab(tab_id: Option<String>) {
    let slot = PRESENTED_BROWSER_TAB.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = tab_id;
    }
}

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
    // Mirror browser tab list/title changes into the sidebar. The handler
    // may fire from webview UI threads, so hop onto the executor before
    // touching window state (layout syncs block on those UI threads).
    crate::browser::set_tabs_changed_handler(Arc::new(|| {
        let _ = crate::task::spawn(async {
            on_browser_tabs_changed();
        });
    }));
    // Keep in-app open-url targets (new-window requests from browser tabs,
    // lxapp openURL with self/new_browser_tab) inside the app as browser
    // tabs; unhandled requests fall back to the OS shell handler.
    lingxia_platform::set_windows_open_url_handler(Arc::new(handle_open_url_request));

    // Deliver lx.surface closes (user window-close or programmatic) back to the
    // logic layer so the JS Surface handle fires onClose, mirroring the
    // apple/android/harmony FFI bridges.
    lingxia_platform::set_windows_surface_closed_handler(Arc::new(|id, reason| {
        lingxia_logic::notify_surface_closed(id, reason);
    }));

    // Report surface page visibility to lxapp so a presented surface fires
    // onShow and is not reclaimed by the page-instance dispose timer (which
    // would close the surface window), mirroring the Apple/Harmony FFI
    // notify_page_instance_visible bridges.
    lingxia_platform::set_windows_page_visibility_handler(Arc::new(|page_instance_id, visible| {
        let event = if visible {
            lxapp::PageInstanceEvent::Visible
        } else {
            lxapp::PageInstanceEvent::Hidden {
                reason: lxapp::CloseReason::Unknown,
            }
        };
        let _ = lxapp::notify_page_instance_by_id(page_instance_id, event);
    }));

    // Dispose a surface's content page instance when the surface closes (native
    // close button or programmatic). Disposing detaches and destroys the page's
    // webview, which is what actually closes the surface window/overlay; the
    // page instance otherwise keeps the webview alive so a bare destroy cannot.
    // Mirrors the dispose_page_instance FFI bridges on the mobile platforms.
    lingxia_platform::set_windows_surface_dispose_handler(Arc::new(|page_instance_id, reason| {
        let reason = match reason.trim().to_ascii_lowercase().as_str() {
            "user" => lxapp::CloseReason::User,
            "owner_closed" => lxapp::CloseReason::OwnerClosed,
            "app_closed" => lxapp::CloseReason::AppClosed,
            "programmatic" => lxapp::CloseReason::Programmatic,
            "reclaimed" => lxapp::CloseReason::Reclaimed,
            _ => lxapp::CloseReason::Unknown,
        };
        let _ = lxapp::dispose_page_instance_by_id(page_instance_id, reason);
    }));

    // Programmatic lx.startPullDownRefresh(). Dispatch the page's
    // onPullDownRefresh lifecycle through the same PullDownRefresh UI event the
    // gesture path uses, which also enforces the page's pull-down-enabled config.
    // The platform layer owns the native refresh indicator show/hide.
    lingxia_platform::set_windows_pull_to_refresh_handler(Arc::new(|appid, path, start| {
        if start && let Some(app) = lxapp::try_get(appid) {
            app.on_lxapp_event(LxAppUiEventType::PullDownRefresh, path.to_string());
        }
    }));
}

/// Routes `open_url` requests with in-app targets into the internal
/// browser. Returns `false` (let the platform open the system handler)
/// for explicit external targets or when no shell/browser is available.
fn handle_open_url_request(req: &OpenUrlRequest) -> bool {
    match req.target {
        OpenUrlTarget::External => false,
        OpenUrlTarget::SelfTarget | OpenUrlTarget::NewBrowserTab => {
            if !crate::browser::runtime_enabled() {
                return false;
            }
            let Some(owner_appid) = shell_owner_appid() else {
                return false;
            };
            // Presentation policy: requests from the presented browser tab
            // (or from a non-browser surface such as an lxapp page) present
            // the new tab; background browser tabs only add a sidebar row.
            let from_browser_tab = req.owner_appid == crate::browser::APP_ID;
            let present = !from_browser_tab || presented_browser_tab().is_some();
            let url = req.url.clone();
            // May be called on a webview UI thread (NewWindowRequested);
            // hop onto the executor before touching tab/window state.
            let _ = crate::task::spawn(async move {
                open_browser_tab_for_open_url(&owner_appid, &url, present);
            });
            true
        }
    }
}

/// Opens `url` as a new in-app browser tab owned by the shell app and, when
/// `present` is set, shows it over the main content card (same flow as the
/// sidebar rows). The tabs-changed observer keeps the sidebar in sync.
fn open_browser_tab_for_open_url(owner_appid: &str, url: &str, present: bool) {
    let Some(app) = lxapp::try_get(owner_appid) else {
        log::warn!("no shell owner app for in-app open-url of {url}");
        return;
    };
    match crate::browser::open_for_app(owner_appid, app.session_id(), url, None) {
        Ok(tab_id) if present => present_browser_tab_when_ready(owner_appid, tab_id),
        Ok(_) => sync_shell_layout(owner_appid),
        Err(err) => log::error!("failed to open browser tab for {url}: {err}"),
    }
}

/// Re-syncs the shell after any browser tab change: drops a stale
/// presentation when the presented tab disappeared and refreshes the
/// sidebar of the shell owner app.
fn on_browser_tabs_changed() {
    if let Some(presented) = presented_browser_tab()
        && crate::browser::tab_summary(&presented).is_none()
    {
        set_presented_browser_tab(None);
        if let Err(err) = restore_presented_group_main() {
            log::warn!("failed to restore main webview after browser tab close: {err}");
        }
    }
    if let Some(appid) = shell_owner_appid() {
        sync_shell_layout(&appid);
    }
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

    if let Err(err) = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout)) {
        log::warn!(
            "failed to sync Windows shell layout for {}:{}: {}",
            appid,
            path,
            err
        );
    }
}

fn build_window_layout(app: &LxApp, path: &str) -> WindowsShellWindowLayout {
    // The Arc-style address bar owns the top bar while a browser tab is
    // presented; the lxapp navigation bar yields for that time.
    let address_bar = build_address_bar_layout();
    let navigation_bar = if address_bar.is_some() {
        None
    } else {
        Some(build_navigation_bar_layout(app, path))
    };
    WindowsShellWindowLayout {
        navigation_bar,
        address_bar,
        tab_bar: build_tab_bar_layout(app),
        panel_activators: build_panel_activators(app),
    }
}

/// Address-bar layout for the presented browser tab, or `None` while the
/// main surface shows an lxapp webview.
fn build_address_bar_layout() -> Option<WindowsShellAddressBarLayout> {
    let presented = presented_browser_tab()?;
    let tab = crate::browser::tab_summary(&presented)?;
    Some(WindowsShellAddressBarLayout {
        visible: true,
        url_text: browser_tab_display_url(&tab),
    })
}

/// Capsule text of the presented tab: its current URL, else its title
/// (matching the sidebar row fallback).
fn browser_tab_display_url(tab: &crate::browser::BrowserTabSummary) -> String {
    tab.current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| browser_tab_display_title(tab))
}

fn build_navigation_bar_layout(app: &LxApp, path: &str) -> WindowsShellNavigationBarLayout {
    let navbar = app.get_navbar_state(path);
    let text_color = match navbar.navigationBarTextStyle.as_str() {
        "white" => 0xffffff,
        _ => 0x111111,
    };
    WindowsShellNavigationBarLayout {
        visible: navbar.show_navbar,
        title: navbar.navigationBarTitleText,
        background_color: parse_css_color(&navbar.navigationBarBackgroundColor, 0xffffff),
        text_color,
        show_back_button: navbar.show_back_button,
        show_home_button: navbar.show_home_button,
        height: DEFAULT_NAV_BAR_HEIGHT,
    }
}

fn build_tab_bar_layout(app: &LxApp) -> Option<WindowsShellTabBarLayout> {
    let tabbar = app.get_tabbar()?;
    let ui_state = sidebar_ui_state(&app.appid);
    Some(WindowsShellTabBarLayout {
        visible: !tabbar.list.is_empty(),
        position: WindowsShellTabBarPosition::Left,
        dimension: tabbar.dimension.max(MIN_SIDEBAR_WIDTH),
        app_name: app.runtime_info().app_name,
        group_id: app.appid.clone(),
        collapsed: ui_state.collapsed,
        items_collapsed: ui_state.items_collapsed,
        color: parse_css_color(&tabbar.color, 0x666666),
        selected_color: parse_css_color(&tabbar.selectedColor, 0x1677ff),
        background_color: parse_css_color(&tabbar.backgroundColor, 0xffffff),
        border_color: parse_css_color(&tabbar.borderStyle, 0xf0f0f0),
        selected_index: tabbar.get_selected_index(),
        items: tabbar
            .list
            .into_iter()
            .map(|item| WindowsShellTabBarItemLayout {
                page_path: item.pagePath,
                text: item.text.unwrap_or_default(),
                icon_path: item.iconPath.unwrap_or_default(),
                selected_icon_path: item.selectedIconPath.unwrap_or_default(),
                badge: item.badge,
                has_red_dot: item.has_red_dot,
            })
            .collect(),
        auxiliary_items: build_browser_tab_items(),
        show_auxiliary_add: crate::browser::runtime_enabled(),
        header_actions: build_sidebar_header_actions(),
    })
}

/// Sidebar header actions (settings / downloads), shown only when the
/// browser runtime backing their target pages is compiled in.
fn build_sidebar_header_actions() -> Vec<WindowsShellHeaderActionLayout> {
    if !crate::browser::runtime_enabled() {
        return Vec::new();
    }
    vec![
        WindowsShellHeaderActionLayout {
            id: SIDEBAR_ACTION_SETTINGS.to_string(),
            glyph: GLYPH_SETTINGS.to_string(),
        },
        WindowsShellHeaderActionLayout {
            id: SIDEBAR_ACTION_DOWNLOADS.to_string(),
            glyph: GLYPH_DOWNLOAD.to_string(),
        },
    ]
}

fn build_browser_tab_items() -> Vec<WindowsShellAuxiliaryItemLayout> {
    let presented = presented_browser_tab();
    crate::browser::tabs()
        .into_iter()
        .map(|tab| {
            let active = presented.as_deref() == Some(tab.tab_id.as_str());
            let title = browser_tab_display_title(&tab);
            let icon_png = tab.favicon_png.clone();
            WindowsShellAuxiliaryItemLayout {
                id: tab.tab_id,
                title,
                active,
                icon_png,
            }
        })
        .collect()
}

/// Sidebar row title for a browser tab: page title, else the URL host,
/// else "New Tab".
fn browser_tab_display_title(tab: &crate::browser::BrowserTabSummary) -> String {
    if let Some(title) = tab
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        return title.to_string();
    }
    if let Some(host) = tab.current_url.as_deref().and_then(url_host) {
        return host;
    }
    "New Tab".to_string()
}

fn url_host(url: &str) -> Option<String> {
    let (_, rest) = url.trim().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or(authority).trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn build_panel_activators(app: &LxApp) -> Vec<WindowsShellPanelActivatorLayout> {
    let asset_dir = app.runtime.asset_dir();
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .map(|panels| {
            panels
                .items
                .into_iter()
                .map(|item| WindowsShellPanelActivatorLayout {
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

fn handle_chrome_event(appid: &str, event: WindowsChromeCommand) {
    set_shell_owner_appid(appid);
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };

    let handled = match event.id.as_str() {
        chrome_command::TAB_BAR_CLICK => {
            let Some(index) = payload_usize(&event, "index") else {
                return;
            };
            // Selecting an lxapp item while a browser tab is presented
            // returns the main surface to the lxapp webview.
            return_to_lxapp_from_browser(appid);
            // No immediate re-sync after this event: the tab switch
            // completes asynchronously and the runtime's own sync_host_ui
            // (before and after the page navigation) is the authoritative
            // layout source. A sync issued now races it and can overwrite
            // the new selection with the not-yet-switched page state.
            let _ = app.on_lxapp_event(LxAppUiEventType::TabBarClick, index.to_string());
            return;
        }
        chrome_command::NAVIGATION_BACK => {
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "back".to_string())
        }
        chrome_command::NAVIGATION_HOME => {
            return_to_lxapp_from_browser(appid);
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "home".to_string())
        }
        chrome_command::PANEL_ACTIVATOR_CLICK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            // The activator handlers sync the shell layout in every branch.
            handle_panel_activator(appid, panel_id);
            return;
        }
        chrome_command::BROWSER_NEW_TAB => {
            handle_browser_new_tab(appid, app.session_id());
            return;
        }
        chrome_command::BROWSER_TAB_CLICK => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            handle_browser_tab_click(appid, &tab_id);
            return;
        }
        chrome_command::BROWSER_TAB_CLOSE => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            handle_browser_tab_close(appid, &tab_id);
            return;
        }
        // Native-panel header events (terminal dock): pure terminal policy,
        // interpreted by the terminal panel facade. Tab/panel closes may
        // change panel visibility; those paths re-sync the layout
        // themselves via `sync_owner_shell_layout`.
        chrome_command::NATIVE_PANEL_TAB_CLICK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(tab_id) = payload_u64(&event, "tab_id") else {
                return;
            };
            super::terminal_panel::activate_terminal_tab(&panel_id, tab_id);
            return;
        }
        chrome_command::NATIVE_PANEL_TAB_CLOSE => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(tab_id) = payload_u64(&event, "tab_id") else {
                return;
            };
            super::terminal_panel::close_terminal_tab(&panel_id, tab_id);
            return;
        }
        chrome_command::NATIVE_PANEL_NEW_TAB => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            super::terminal_panel::open_terminal_tab(&panel_id);
            return;
        }
        chrome_command::NATIVE_PANEL_MAXIMIZE => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            super::terminal_panel::toggle_terminal_panel_maximized(&panel_id);
            return;
        }
        chrome_command::NATIVE_PANEL_TAB_RENAME => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(tab_id) = payload_u64(&event, "tab_id") else {
                return;
            };
            super::terminal_panel::begin_terminal_tab_rename(&panel_id, tab_id);
            return;
        }
        chrome_command::NATIVE_PANEL_RIGHT_CLICK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(screen_x) = payload_i32(&event, "screen_x") else {
                return;
            };
            let Some(screen_y) = payload_i32(&event, "screen_y") else {
                return;
            };
            super::terminal_panel::show_terminal_context_menu(appid, &panel_id, screen_x, screen_y);
            return;
        }
        // Address-bar navigation targets the presented browser tab; URL and
        // title updates flow back through the tabs-changed observer.
        chrome_command::BROWSER_NAV_BACK => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::go_back(&tab_id)
            {
                log::warn!("browser back failed for tab {tab_id}");
            }
            return;
        }
        chrome_command::BROWSER_NAV_FORWARD => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::go_forward(&tab_id)
            {
                log::warn!("browser forward failed for tab {tab_id}");
            }
            return;
        }
        chrome_command::BROWSER_NAV_RELOAD => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::reload(&tab_id)
            {
                log::warn!("browser reload failed for tab {tab_id}");
            }
            return;
        }
        chrome_command::BROWSER_ADDRESS_BAR => {
            begin_presented_tab_address_edit(&app);
            return;
        }
        chrome_command::SIDEBAR_TOGGLE => {
            update_sidebar_ui_state(appid, |state| state.collapsed = !state.collapsed);
            sync_shell_layout(appid);
            return;
        }
        chrome_command::SIDEBAR_GROUP_TOGGLE => {
            let Some(group) = payload_string(&event, "group") else {
                return;
            };
            update_sidebar_ui_state(&group, |state| {
                state.items_collapsed = !state.items_collapsed;
            });
            sync_shell_layout(appid);
            return;
        }
        chrome_command::SIDEBAR_ACTION => {
            let Some(action_id) = payload_string(&event, "action_id") else {
                return;
            };
            handle_sidebar_action(appid, app.session_id(), &action_id);
            return;
        }
        other => {
            log::warn!("unknown Windows shell chrome command for {appid}: {other}");
            false
        }
    };

    if handled {
        sync_shell_layout(appid);
    } else {
        log::error!("Windows shell chrome event was not handled for {appid}");
    }
}

fn payload_string(command: &WindowsChromeCommand, field: &str) -> Option<String> {
    command
        .payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            log::warn!(
                "Windows shell chrome command {} missing string field {field}",
                command.id
            );
            None
        })
}

fn payload_u64(command: &WindowsChromeCommand, field: &str) -> Option<u64> {
    command
        .payload
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            log::warn!(
                "Windows shell chrome command {} missing u64 field {field}",
                command.id
            );
            None
        })
}

fn payload_usize(command: &WindowsChromeCommand, field: &str) -> Option<usize> {
    payload_u64(command, field).and_then(|value| usize::try_from(value).ok())
}

fn payload_i32(command: &WindowsChromeCommand, field: &str) -> Option<i32> {
    command
        .payload
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| {
            log::warn!(
                "Windows shell chrome command {} missing i32 field {field}",
                command.id
            );
            None
        })
}

/// Ends a browser-tab presentation (if any), restoring the lxapp webview
/// as the main surface. Safe to call when nothing is presented.
fn return_to_lxapp_from_browser(appid: &str) {
    if presented_browser_tab().is_none() {
        return;
    }
    set_presented_browser_tab(None);
    if let Err(err) = restore_presented_group_main() {
        log::warn!("failed to restore lxapp webview for {appid}: {err}");
    }
}

/// Opens a new browser tab at `lingxia://newtab` owned by the shell app
/// and presents it once its webview is ready.
fn handle_browser_new_tab(appid: &str, session_id: u64) {
    match crate::browser::open_for_app(appid, session_id, "lingxia://newtab", None) {
        Ok(tab_id) => present_browser_tab_when_ready(appid, tab_id),
        Err(err) => log::error!("failed to open new browser tab for {appid}: {err}"),
    }
}

fn handle_browser_tab_click(appid: &str, tab_id: &str) {
    if !crate::browser::activate(tab_id) {
        log::warn!("browser tab no longer exists: {tab_id}");
        sync_shell_layout(appid);
        return;
    }
    present_browser_tab_when_ready(appid, tab_id.to_string());
}

fn handle_browser_tab_close(appid: &str, tab_id: &str) {
    if presented_browser_tab().as_deref() == Some(tab_id) {
        return_to_lxapp_from_browser(appid);
    }
    if let Err(err) = crate::browser::close(tab_id) {
        log::error!("failed to close browser tab {tab_id}: {err}");
    }
    // The tabs-changed observer re-syncs as well; sync directly so the row
    // disappears even if no observer is installed.
    sync_shell_layout(appid);
}

/// Presents `tab_id`'s webview over the main content card, retrying while
/// the tab's WebView creation is still in flight (new tabs create their
/// webview asynchronously).
fn present_browser_tab_when_ready(appid: &str, tab_id: String) {
    let owner_appid = appid.to_string();
    let _ = crate::task::spawn(async move {
        for attempt in 0..PRESENT_BROWSER_TAB_MAX_RETRY {
            let Some(tab) = crate::browser::tab_summary(&tab_id) else {
                // Tab was closed while waiting.
                return;
            };
            let webtag = WebTag::new(crate::browser::APP_ID, &tab.path, Some(tab.session_id));
            let result = present_webview_in_active_group(&webtag);
            match result {
                Ok(()) => {
                    set_presented_browser_tab(Some(tab_id.clone()));
                    sync_shell_layout(&owner_appid);
                    return;
                }
                Err(err) => {
                    if attempt + 1 == PRESENT_BROWSER_TAB_MAX_RETRY {
                        log::error!("failed to present browser tab {tab_id}: {err}");
                        return;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                PRESENT_BROWSER_TAB_RETRY_DELAY_MS,
            ))
            .await;
        }
    });
}

/// Opens the browser page behind a sidebar header action (settings /
/// downloads) as a presented browser tab.
fn handle_sidebar_action(appid: &str, session_id: u64, action_id: &str) {
    let target = match action_id {
        SIDEBAR_ACTION_SETTINGS => SETTINGS_PAGE_URL,
        SIDEBAR_ACTION_DOWNLOADS => DOWNLOADS_PAGE_URL,
        _ => {
            log::warn!("unknown Windows sidebar action: {action_id}");
            return;
        }
    };
    open_or_present_browser_page(appid, session_id, target);
}

/// Presents `url` as a browser page: when a tab already shows it, that tab
/// is activated and presented; otherwise a new tab opens at `url` (same
/// flow as the sidebar "New Tab" row, just with a target URL).
fn open_or_present_browser_page(appid: &str, session_id: u64, url: &str) {
    let existing = crate::browser::tabs()
        .into_iter()
        .find(|tab| tab.current_url.as_deref() == Some(url));
    if let Some(existing) = existing {
        handle_browser_tab_click(appid, &existing.tab_id);
        return;
    }
    match crate::browser::open_for_app(appid, session_id, url, None) {
        Ok(tab_id) => present_browser_tab_when_ready(appid, tab_id),
        Err(err) => log::error!("failed to open browser page {url} for {appid}: {err}"),
    }
}

/// Starts the inline URL edit over the top-bar address capsule (the shell
/// chrome's EDIT helper, the same one terminal tab renames use). The commit
/// resolves the input through the shell address resolver and navigates the
/// presented tab. Requires the shell chrome; without it there is no
/// address bar to edit.
/// Host-window handle of `appid`'s shell window (the window whose chrome
/// painted the sidebar/top bar), for product UI that needs a real HWND
/// (inline edits, context menus).
#[cfg(feature = "shell-runtime")]
pub(super) fn owner_window_handle(appid: &str) -> Option<isize> {
    let app = lxapp::try_get(appid)?;
    let path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    let webtag = WebTag::new(&app.appid, &path, Some(app.session_id()));
    let snapshot = Some(lingxia_platform::windows::webview_host::webview_window_snapshot(&webtag));
    match snapshot {
        Some(Ok(snapshot)) => Some(snapshot.window_id as isize),
        Some(Err(err)) => {
            log::warn!("no shell window handle for {appid}: {err}");
            None
        }
        None => {
            log::warn!("no shell window handle for {appid}: WebView handler is not ready");
            None
        }
    }
}

#[cfg(feature = "shell-runtime")]
fn begin_presented_tab_address_edit(app: &LxApp) {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = crate::browser::tab_summary(&tab_id) else {
        return;
    };
    // The capsule was painted by the shell-owner window's chrome; its host
    // window handle comes from the owner webtag's window snapshot.
    let Some(window) = owner_window_handle(&app.appid) else {
        return;
    };

    let owner_appid = app.appid.clone();
    let initial = tab.current_url.clone().unwrap_or_default();
    lingxia_shell::windows::begin_address_edit(
        window,
        &initial,
        Arc::new(move |text: String| {
            commit_address_input(&owner_appid, &tab_id, &text);
        }),
    );
}

#[cfg(not(feature = "shell-runtime"))]
fn begin_presented_tab_address_edit(_app: &LxApp) {
    // Without the shell chrome no address bar is drawn (plain OS frame),
    // so there is nothing to edit.
}

/// Resolves a committed address input and navigates the presented tab.
/// Runs on the host window's UI thread (inline-edit commit); the actual
/// navigation hops onto the executor so webview work never blocks that
/// thread.
#[cfg(feature = "shell-runtime")]
fn commit_address_input(appid: &str, tab_id: &str, raw_input: &str) {
    if raw_input.trim().is_empty() {
        return;
    }
    let response = lingxia_shell::resolve_input(lingxia_shell::BrowserAddressInputRequest {
        raw_input: raw_input.to_string(),
        trigger: lingxia_shell::BrowserAddressInputTrigger::Submit,
        context: lingxia_shell::BrowserAddressInputContext::default(),
    });
    let Some(navigation) = response.navigation else {
        log::info!(
            "address input did not resolve to a navigation: {}",
            response
                .error
                .map(|error| error.code)
                .unwrap_or_else(|| "no navigation".to_string())
        );
        return;
    };

    let appid = appid.to_string();
    let tab_id = tab_id.to_string();
    let _ = crate::task::spawn(async move {
        if let Err(err) = crate::browser::navigate(&tab_id, &navigation.url) {
            log::error!("failed to navigate browser tab {tab_id}: {err}");
        }
        // The tabs-changed observer re-syncs as well; sync directly so the
        // capsule reflects the committed URL even without an observer.
        sync_shell_layout(&appid);
    });
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
        if let Err(err) = hide_host_panel(&panel_id) {
            log::warn!("failed to hide Windows panel {panel_id}: {err}");
        }
        lxapp::mark_lxapp_active(appid);
        sync_shell_layout(appid);
        return;
    }

    if !pending_panel_opens().insert(panel_id.clone()) {
        return;
    }

    let owner_appid = appid.to_string();
    let _ = crate::task::spawn(async move {
        let result = open_panel_lxapp(&panel_id, &panel_appid, &path).await;
        pending_panel_opens().remove(&panel_id);
        if let Err(err) = result {
            log::error!("failed to open Windows panel lxapp {panel_appid}: {err}");
            return;
        }
        sync_shell_layout(&owner_appid);
    });
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
        if let Err(err) = super::terminal_panel::close_windows_terminal_panel(&request.panel_id) {
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
    if let Err(err) =
        super::terminal_panel::open_windows_terminal_panel(&request.panel_id, title, position)
    {
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
    if !hex.is_ascii() {
        return fallback;
    }
    let rgb = match hex.len() {
        3 => {
            let expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            u32::from_str_radix(&expanded, 16).ok()
        }
        6 => u32::from_str_radix(hex, 16).ok(),
        // CSS 8-digit hex is #RRGGBBAA: keep the leading RGB bytes, ignore alpha.
        8 => u32::from_str_radix(&hex[..6], 16).ok(),
        _ => None,
    };
    rgb.unwrap_or(fallback)
}
