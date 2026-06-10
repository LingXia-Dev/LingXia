use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_platform::traits::app_runtime::{AppRuntime, LxAppOpenMode};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    WindowsBrowserTabItemLayout, WindowsChromeEvent, WindowsNavigationBarLayout,
    WindowsPanelActivatorLayout, WindowsPanelPosition, WindowsTabBarItemLayout,
    WindowsTabBarLayout, WindowsTabBarPosition, WindowsWindowLayout, hide_panel, is_panel_visible,
    present_webview_as_group_main, restore_presented_group_main,
    set_webview_chrome_event_handler, set_webview_window_layout,
};
use lxapp::{LxApp, LxAppDelegate, LxAppStartupOptions, LxAppUiEventType, ReleaseType};

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
        dimension: tabbar.dimension.max(MIN_SIDEBAR_WIDTH),
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
        browser_tabs: build_browser_tab_items(),
        show_browser_new_tab: crate::browser::runtime_enabled(),
    })
}

fn build_browser_tab_items() -> Vec<WindowsBrowserTabItemLayout> {
    let presented = presented_browser_tab();
    crate::browser::tabs()
        .into_iter()
        .map(|tab| WindowsBrowserTabItemLayout {
            title: browser_tab_display_title(&tab),
            active: presented.as_deref() == Some(tab.tab_id.as_str()),
            tab_id: tab.tab_id,
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
    set_shell_owner_appid(appid);
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };

    let handled = match event {
        WindowsChromeEvent::TabBarClick { index } => {
            // Selecting an lxapp item while a browser tab is presented
            // returns the main surface to the lxapp webview.
            return_to_lxapp_from_browser(appid);
            app.on_lxapp_event(LxAppUiEventType::TabBarClick, index.to_string())
        }
        WindowsChromeEvent::NavigationBack => {
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "back".to_string())
        }
        WindowsChromeEvent::NavigationHome => {
            return_to_lxapp_from_browser(appid);
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "home".to_string())
        }
        WindowsChromeEvent::PanelActivatorClick { panel_id } => {
            // The activator handlers sync the shell layout in every branch.
            handle_panel_activator(appid, panel_id);
            return;
        }
        WindowsChromeEvent::BrowserNewTabClick => {
            handle_browser_new_tab(appid, app.session_id());
            return;
        }
        WindowsChromeEvent::BrowserTabClick { tab_id } => {
            handle_browser_tab_click(appid, &tab_id);
            return;
        }
        WindowsChromeEvent::BrowserTabCloseClick { tab_id } => {
            handle_browser_tab_close(appid, &tab_id);
            return;
        }
    };

    if handled {
        sync_shell_layout(appid);
    } else {
        log::error!("Windows shell chrome event was not handled for {appid}");
    }
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
            match present_webview_as_group_main(&webtag) {
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
