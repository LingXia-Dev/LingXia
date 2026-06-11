use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_platform::traits::app_runtime::{
    AppRuntime, LxAppOpenMode, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{
    WindowsAddressBarLayout, WindowsBrowserTabItemLayout, WindowsChromeEvent,
    WindowsNavigationBarLayout, WindowsPanelActivatorLayout, WindowsPanelPosition,
    WindowsSidebarActionLayout, WindowsTabBarItemLayout, WindowsTabBarLayout,
    WindowsTabBarPosition, WindowsWindowLayout, hide_panel, is_panel_visible,
    present_webview_as_group_main, restore_presented_group_main, set_webview_chrome_event_handler,
    set_webview_window_layout,
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

/// Sidebar header action ids (echoed back through
/// `WindowsChromeEvent::SidebarActionClick`) and their browser targets.
const SIDEBAR_ACTION_SETTINGS: &str = "settings";
const SIDEBAR_ACTION_DOWNLOADS: &str = "downloads";
const SETTINGS_PAGE_URL: &str = "lingxia://settings";
const DOWNLOADS_PAGE_URL: &str = "lingxia://downloads";

/// Segoe Fluent Icons glyphs of the sidebar header actions (passed through
/// layout data so the webview layer stays product-agnostic).
const GLYPH_SETTINGS: &str = "\u{e713}";
const GLYPH_DOWNLOAD: &str = "\u{e896}";

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
/// after a panel changed visibility outside a chrome event — e.g. the
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
    // The Arc-style address bar owns the top bar while a browser tab is
    // presented; the lxapp navigation bar yields for that time.
    let address_bar = build_address_bar_layout();
    let navigation_bar = if address_bar.is_some() {
        None
    } else {
        Some(build_navigation_bar_layout(app, path))
    };
    WindowsWindowLayout {
        navigation_bar,
        address_bar,
        tab_bar: build_tab_bar_layout(app),
        panel_activators: build_panel_activators(app),
    }
}

/// Address-bar layout for the presented browser tab, or `None` while the
/// main surface shows an lxapp webview.
fn build_address_bar_layout() -> Option<WindowsAddressBarLayout> {
    let presented = presented_browser_tab()?;
    let tab = crate::browser::tab_summary(&presented)?;
    Some(WindowsAddressBarLayout {
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
    let ui_state = sidebar_ui_state(&app.appid);
    Some(WindowsTabBarLayout {
        visible: !tabbar.list.is_empty(),
        position: WindowsTabBarPosition::Left,
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
        header_actions: build_sidebar_header_actions(),
    })
}

/// Sidebar header actions (settings / downloads), shown only when the
/// browser runtime backing their target pages is compiled in.
fn build_sidebar_header_actions() -> Vec<WindowsSidebarActionLayout> {
    if !crate::browser::runtime_enabled() {
        return Vec::new();
    }
    vec![
        WindowsSidebarActionLayout {
            id: SIDEBAR_ACTION_SETTINGS.to_string(),
            glyph: GLYPH_SETTINGS.to_string(),
        },
        WindowsSidebarActionLayout {
            id: SIDEBAR_ACTION_DOWNLOADS.to_string(),
            glyph: GLYPH_DOWNLOAD.to_string(),
        },
    ]
}

fn build_browser_tab_items() -> Vec<WindowsBrowserTabItemLayout> {
    let presented = presented_browser_tab();
    crate::browser::tabs()
        .into_iter()
        .map(|tab| WindowsBrowserTabItemLayout {
            title: browser_tab_display_title(&tab),
            active: presented.as_deref() == Some(tab.tab_id.as_str()),
            favicon_png: tab.favicon_png.clone(),
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
        // Native-panel header events (terminal dock): pure terminal policy,
        // interpreted by the terminal panel facade. Tab/panel closes may
        // change panel visibility; those paths re-sync the layout
        // themselves via `sync_owner_shell_layout`.
        WindowsChromeEvent::NativePanelTabClick { panel_id, tab_id } => {
            super::terminal_panel::activate_terminal_tab(&panel_id, tab_id);
            return;
        }
        WindowsChromeEvent::NativePanelTabCloseClick { panel_id, tab_id } => {
            super::terminal_panel::close_terminal_tab(&panel_id, tab_id);
            return;
        }
        WindowsChromeEvent::NativePanelNewTabClick { panel_id } => {
            super::terminal_panel::open_terminal_tab(&panel_id);
            return;
        }
        WindowsChromeEvent::NativePanelMaximizeClick { panel_id } => {
            super::terminal_panel::toggle_terminal_panel_maximized(&panel_id);
            return;
        }
        WindowsChromeEvent::NativePanelTabRenameRequest { panel_id, tab_id } => {
            super::terminal_panel::begin_terminal_tab_rename(&panel_id, tab_id);
            return;
        }
        WindowsChromeEvent::NativePanelRightClick { panel_id } => {
            super::terminal_panel::paste_clipboard_into_panel(&panel_id);
            return;
        }
        // Address-bar navigation targets the presented browser tab; URL and
        // title updates flow back through the tabs-changed observer.
        WindowsChromeEvent::BrowserNavBackClick => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::go_back(&tab_id)
            {
                log::warn!("browser back failed for tab {tab_id}");
            }
            return;
        }
        WindowsChromeEvent::BrowserNavForwardClick => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::go_forward(&tab_id)
            {
                log::warn!("browser forward failed for tab {tab_id}");
            }
            return;
        }
        WindowsChromeEvent::BrowserNavReloadClick => {
            if let Some(tab_id) = presented_browser_tab()
                && !crate::browser::reload(&tab_id)
            {
                log::warn!("browser reload failed for tab {tab_id}");
            }
            return;
        }
        WindowsChromeEvent::BrowserAddressBarClick => {
            begin_presented_tab_address_edit(&app);
            return;
        }
        WindowsChromeEvent::SidebarToggleClick => {
            update_sidebar_ui_state(appid, |state| state.collapsed = !state.collapsed);
            sync_shell_layout(appid);
            return;
        }
        WindowsChromeEvent::SidebarGroupToggleClick { group } => {
            update_sidebar_ui_state(&group, |state| {
                state.items_collapsed = !state.items_collapsed;
            });
            sync_shell_layout(appid);
            return;
        }
        WindowsChromeEvent::SidebarActionClick { action_id } => {
            handle_sidebar_action(appid, app.session_id(), &action_id);
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
    let path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    let webtag = WebTag::new(&app.appid, &path, Some(app.session_id()));
    let window = match lingxia_webview::platform::windows::webview_window_snapshot(&webtag) {
        Ok(snapshot) => snapshot.window_id as isize,
        Err(err) => {
            log::warn!("no shell window for address edit of {}: {err}", app.appid);
            return;
        }
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
