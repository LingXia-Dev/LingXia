use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
// Atomics here back the browser tab-sync debounce only.
#[cfg(feature = "browser-runtime")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout,
};
#[cfg(feature = "browser-runtime")]
use lingxia_browser::BrowserTabInfo;
#[cfg(feature = "browser-runtime")]
use lingxia_browser_shell::{
    BrowserAddressInputContext, BrowserAddressInputRequest, BrowserAddressInputTrigger,
    resolve_input,
};
use lingxia_platform::traits::app_runtime::{
    AppRuntime, LxAppOpenMode, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_webview::WebTag;
use lingxia_windows_host::{
    WindowsChromeCommand, WindowsPanelPosition, WindowsWindowLayout, hide_host_panel,
    is_panel_visible, restore_presented_group_main, set_webview_chrome_event_handler,
    set_webview_window_layout,
};
// Presenting a browser tab over the main card is browser-only.
#[cfg(feature = "browser-runtime")]
use lingxia_windows_host::present_webview_in_active_group;
use lxapp::{LxApp, LxAppDelegate, LxAppStartupOptions, LxAppUiEventType, ReleaseType};

const DEFAULT_NAV_BAR_HEIGHT: i32 = 38;
const MIN_SIDEBAR_WIDTH: i32 = 180;
/// iPhone-style bottom tab bar: 49 px item strip plus a home-indicator safe area.
/// Bottom tab bar content height (icons + labels). Face-ID-style devices add a
/// home-indicator safe area below it; home-button devices (e.g. iPhone SE) do
/// not, so their bar is shorter (no tall blank strip under the labels).
const BOTTOM_TABBAR_CONTENT_HEIGHT: i32 = 49;
const HOME_INDICATOR_HEIGHT: i32 = 34;

/// How many times to retry presenting a freshly opened browser tab whose
/// WebView creation is still in flight, and the delay between attempts.
#[cfg(feature = "browser-runtime")]
const PRESENT_BROWSER_TAB_MAX_RETRY: u32 = 30;
#[cfg(feature = "browser-runtime")]
const PRESENT_BROWSER_TAB_RETRY_DELAY_MS: u64 = 100;
#[cfg(feature = "browser-runtime")]
const BROWSER_TAB_SYNC_DEBOUNCE_MS: u64 = 180;

/// Panel ids whose lxapp open is still in flight, used to ignore repeated
/// activator clicks until the open completes.
static PENDING_PANEL_OPENS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// The lxapp that owns the main shell window (set when the home app opens
/// and refreshed on every chrome event); browser tab-change notifications
/// re-sync this app's layout.
static SHELL_OWNER_APPID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Browser tab currently presented over the main content card, if any.
static PRESENTED_BROWSER_TAB: OnceLock<Mutex<Option<String>>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static SUPPRESSED_BROWSER_TAB_SYNCS: OnceLock<Mutex<u32>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static BROWSER_TAB_SYNC_EPOCH: AtomicU64 = AtomicU64::new(0);
static DEFAULT_TABBAR_POSITION: OnceLock<Mutex<WindowsShellTabBarPosition>> = OnceLock::new();
static TABBAR_POSITION_OVERRIDES: OnceLock<Mutex<HashMap<String, WindowsShellTabBarPosition>>> =
    OnceLock::new();

/// Sidebar header action ids and their browser targets.
const SIDEBAR_ACTION_SETTINGS: &str = "settings";
const SIDEBAR_ACTION_DOWNLOADS: &str = "downloads";
#[cfg(feature = "browser-runtime")]
const SETTINGS_PAGE_URL: &str = "lingxia://settings";
#[cfg(feature = "browser-runtime")]
const DOWNLOADS_PAGE_URL: &str = "lingxia://downloads";
const AUX_LXAPP_PREFIX: &str = "lxapp:";

/// Segoe Fluent Icons glyphs of the sidebar header actions (passed through
/// layout data so the webview layer stays product-agnostic).
const GLYPH_SETTINGS: &str = "\u{e713}";
const GLYPH_DOWNLOAD: &str = "\u{e896}";

#[derive(Debug, Clone)]
// The type is referenced by the no-browser stubs (empty collections), but its
// fields are only read by the browser-runtime tab plumbing.
#[cfg_attr(not(feature = "browser-runtime"), allow(dead_code))]
struct BrowserTabSummary {
    tab_id: String,
    path: String,
    session_id: u64,
    title: Option<String>,
    current_url: Option<String>,
    favicon_png: Option<Arc<Vec<u8>>>,
}

fn browser_runtime_enabled() -> bool {
    cfg!(feature = "browser-runtime")
}

#[cfg(feature = "browser-runtime")]
fn browser_tab_summary_from_info(info: BrowserTabInfo) -> BrowserTabSummary {
    let favicon_png = lingxia_browser::tab_favicon(&info.tab_id);
    BrowserTabSummary {
        tab_id: info.tab_id,
        path: info.path,
        session_id: info.session_id,
        title: info.title,
        current_url: info.current_url,
        favicon_png,
    }
}

#[cfg(feature = "browser-runtime")]
fn browser_tabs() -> Vec<BrowserTabSummary> {
    lingxia_browser::tabs()
        .into_iter()
        .map(browser_tab_summary_from_info)
        .collect()
}

/// No browser engine → no browser tabs in the shell.
#[cfg(not(feature = "browser-runtime"))]
fn browser_tabs() -> Vec<BrowserTabSummary> {
    Vec::new()
}

#[cfg(feature = "browser-runtime")]
fn browser_tab_summary(tab_id: &str) -> Option<BrowserTabSummary> {
    lingxia_browser::tabs()
        .into_iter()
        .find(|tab| tab.tab_id == tab_id)
        .map(browser_tab_summary_from_info)
}

#[cfg(not(feature = "browser-runtime"))]
fn browser_tab_summary(_tab_id: &str) -> Option<BrowserTabSummary> {
    None
}

// Browser-tab navigation, stubbed to no-ops without the browser engine so the
// shell chrome's nav commands compile (they can never fire without a tab).
#[cfg(feature = "browser-runtime")]
fn browser_go_back(tab_id: &str) {
    if let Err(err) = lingxia_browser::go_back(tab_id) {
        log::warn!("browser back failed for tab {tab_id}: {err}");
    }
}

#[cfg(feature = "browser-runtime")]
fn browser_go_forward(tab_id: &str) {
    if let Err(err) = lingxia_browser::go_forward(tab_id) {
        log::warn!("browser forward failed for tab {tab_id}: {err}");
    }
}

#[cfg(feature = "browser-runtime")]
fn browser_reload(tab_id: &str) {
    if let Err(err) = lingxia_browser::reload(tab_id) {
        log::warn!("browser reload failed for tab {tab_id}: {err}");
    }
}

#[cfg(not(feature = "browser-runtime"))]
fn browser_go_back(_tab_id: &str) {}
#[cfg(not(feature = "browser-runtime"))]
fn browser_go_forward(_tab_id: &str) {}
#[cfg(not(feature = "browser-runtime"))]
fn browser_reload(_tab_id: &str) {}

/// The built-in browser lxapp id is excluded from the sidebar's open-lxapp
/// list. Without the browser engine there is no such id.
#[cfg(feature = "browser-runtime")]
fn is_builtin_browser_appid(appid: &str) -> bool {
    appid == lingxia_browser::BUILTIN_BROWSER_APPID
}

#[cfg(not(feature = "browser-runtime"))]
fn is_builtin_browser_appid(_appid: &str) -> bool {
    false
}

#[cfg(feature = "browser-runtime")]
fn navigate_browser_tab(tab_id: &str, url: &str) -> Result<(), lxapp::LxAppError> {
    lingxia_browser::open(url, Some(tab_id)).map(|_| ())
}

mod chrome_command {
    pub(super) const TAB_BAR_CLICK: &str = "tabbar.click";
    pub(super) const PANEL_ACTIVATOR_CLICK: &str = "panel-activator.click";
    pub(super) const NAVIGATION_BACK: &str = "navigation.back";
    pub(super) const NAVIGATION_HOME: &str = "navigation.home";
    pub(super) const BROWSER_NEW_TAB: &str = "browser.new-tab";
    pub(super) const BROWSER_TAB_CLICK: &str = "browser.tab.click";
    pub(super) const BROWSER_TAB_CLOSE: &str = "browser.tab.close";
    pub(super) const SIDEBAR_AUXILIARY_CONTEXT_MENU: &str = "sidebar.auxiliary.context-menu";
    pub(super) const BROWSER_PANEL_CLOSE: &str = "browser-panel.close";
    pub(super) const BROWSER_PANEL_NAV_BACK: &str = "browser-panel.nav.back";
    pub(super) const BROWSER_PANEL_NAV_FORWARD: &str = "browser-panel.nav.forward";
    pub(super) const BROWSER_PANEL_NAV_RELOAD: &str = "browser-panel.nav.reload";
    pub(super) const BROWSER_PANEL_ADDRESS_BAR: &str = "browser-panel.address-bar";
    pub(super) const NATIVE_PANEL_TAB_CLICK: &str = "native-panel.tab.click";
    pub(super) const NATIVE_PANEL_TAB_CLOSE: &str = "native-panel.tab.close";
    pub(super) const NATIVE_PANEL_NEW_TAB: &str = "native-panel.new-tab";
    pub(super) const NATIVE_PANEL_MAXIMIZE: &str = "native-panel.maximize";
    pub(super) const NATIVE_PANEL_TAB_RENAME: &str = "native-panel.tab.rename";
    pub(super) const NATIVE_PANEL_RIGHT_CLICK: &str = "native-panel.right-click";
    pub(super) const NATIVE_PANEL_PANE_FOCUS: &str = "native-panel.pane-focus";
    pub(super) const BROWSER_NAV_BACK: &str = "browser.nav.back";
    pub(super) const BROWSER_NAV_FORWARD: &str = "browser.nav.forward";
    pub(super) const BROWSER_NAV_RELOAD: &str = "browser.nav.reload";
    pub(super) const BROWSER_ADDRESS_BAR: &str = "browser.address-bar";
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
    pub(super) const APP_MENU_CLICK: &str = "app-menu.click";
}

/// Per-group (per shell-owner lxapp) sidebar UI state, kept for the
/// session: whole-sidebar collapse and the lxapp items-group collapse.
#[derive(Debug, Clone, Copy, Default)]
struct SidebarUiState {
    /// Sidebar fully hidden.
    collapsed: bool,
    /// Sidebar shown as an icon-only rail (the macOS first-collapse state).
    icon_rail: bool,
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

fn set_shell_owner_appid(appid: &str) {
    let slot = SHELL_OWNER_APPID.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(appid.to_string());
    }
}

pub(crate) fn set_home_app_id(appid: &str) {
    set_shell_owner_appid(appid);
}

fn is_shell_owner_appid(appid: &str) -> bool {
    shell_owner_appid()
        .as_deref()
        .map(|owner| owner == appid)
        .unwrap_or(false)
}

pub(crate) fn open_home_app(appid: &str) -> Result<(), String> {
    set_shell_owner_appid(appid);
    lxapp::open_lxapp(appid, LxAppStartupOptions::new(""))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn shell_owner_appid() -> Option<String> {
    SHELL_OWNER_APPID
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

/// Push the host window's logical (DIP) content width into the shell-owner
/// app's adaptive surface graph so the size class - and therefore the aside
/// cap (Compact 0 / Medium 1 / Expanded 2) - tracks the real window. Without
/// this the graph stays at its seed width (permanently Medium), so a second
/// aside evicts the first even on a wide window. Called from the host's
/// `WM_SIZE`.
pub(crate) fn update_surface_width(logical_width: f64) {
    if logical_width <= 0.0 {
        return;
    }
    if let Some(appid) = shell_owner_appid() {
        lingxia::windows::set_surface_width(&appid, logical_width);
    }
}

pub(crate) fn set_default_tabbar_position(position: WindowsShellTabBarPosition) {
    let state =
        DEFAULT_TABBAR_POSITION.get_or_init(|| Mutex::new(WindowsShellTabBarPosition::Left));
    if let Ok(mut state) = state.lock() {
        *state = position;
    }
}

pub(crate) fn set_tabbar_position(appid: &str, position: WindowsShellTabBarPosition) {
    let overrides = TABBAR_POSITION_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut overrides) = overrides.lock() {
        overrides.insert(appid.to_string(), position);
    }
    sync_shell_layout(appid);
}

fn tabbar_position(appid: &str) -> WindowsShellTabBarPosition {
    TABBAR_POSITION_OVERRIDES
        .get()
        .and_then(|overrides| overrides.lock().ok())
        .and_then(|overrides| overrides.get(appid).copied())
        .or_else(default_tabbar_position)
        .unwrap_or(WindowsShellTabBarPosition::Left)
}

fn default_tabbar_position() -> Option<WindowsShellTabBarPosition> {
    DEFAULT_TABBAR_POSITION
        .get()
        .and_then(|position| position.lock().ok())
        .map(|position| *position)
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

#[cfg(feature = "browser-runtime")]
fn suppress_next_browser_tab_sync() {
    let slot = SUPPRESSED_BROWSER_TAB_SYNCS.get_or_init(|| Mutex::new(0));
    if let Ok(mut count) = slot.lock() {
        *count = count.saturating_add(1);
    }
}

#[cfg(feature = "browser-runtime")]
fn consume_suppressed_browser_tab_sync() -> bool {
    let Some(slot) = SUPPRESSED_BROWSER_TAB_SYNCS.get() else {
        return false;
    };
    let Ok(mut count) = slot.lock() else {
        return false;
    };
    if *count == 0 {
        return false;
    }
    *count -= 1;
    true
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
    // A trimmed lxapp page that opted into pull-down refresh gets an app-level
    // "Refresh" right-click entry (mirrors the macOS lxapp menu). The webview
    // layer that builds the menu sits below lxapp / i18n / pull-refresh, so it
    // calls back here for the label and the action.
    lingxia_webview::platform::windows::set_windows_context_menu_refresh_provider(
        Arc::new(|appid: &str, path: &str| {
            lxapp::is_pull_down_refresh_enabled(appid, path)
                .then(|| lingxia_logic::i18n::t(lingxia_logic::I18nKey::CommonRefresh))
        }),
        Arc::new(|appid: &str, path: &str| {
            crate::pull_to_refresh::request_refresh(appid, path);
        }),
    );
    // Mirror browser tab list/title changes into the sidebar. The handler
    // may fire from webview UI threads, so hop onto the executor before
    // touching window state (layout syncs block on those UI threads).
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::set_tabs_changed_handler(Arc::new(|| {
        schedule_browser_tabs_changed_sync();
    }));
    // Keep in-app open-url targets (new-window requests from browser tabs,
    // lxapp openURL with self/new_browser_tab) inside the app as browser
    // tabs; unhandled requests fall back to the OS shell handler.
    lingxia_platform::set_windows_open_url_handler(Arc::new(handle_open_url_request));
    lingxia_platform::set_windows_managed_surface_visible_handler(Arc::new(
        set_managed_surface_visible,
    ));
    lingxia_platform::set_windows_managed_surface_toggle_handler(Arc::new(toggle_managed_surface));

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
        match lxapp::notify_page_instance_by_id(page_instance_id, event) {
            Ok(()) => true,
            Err(err) => {
                log::debug!(
                    "Windows surface page visibility deferred for {} visible={}: {}",
                    page_instance_id,
                    visible,
                    err
                );
                false
            }
        }
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
}

/// Routes `open_url` requests with in-app targets into the internal
/// browser. Returns `false` (let the platform open the system handler)
/// for explicit external targets or when no shell/browser is available.
fn handle_open_url_request(req: &OpenUrlRequest) -> bool {
    match req.target {
        OpenUrlTarget::External => false,
        // In-app targets are routed into the internal browser; without the
        // browser engine there is nowhere in-app to open them, so defer to the
        // OS handler.
        #[cfg(not(feature = "browser-runtime"))]
        OpenUrlTarget::SelfTarget | OpenUrlTarget::NewBrowserTab => false,
        #[cfg(feature = "browser-runtime")]
        OpenUrlTarget::SelfTarget | OpenUrlTarget::NewBrowserTab => {
            let Some(owner_appid) = shell_owner_appid() else {
                return false;
            };
            // Presentation policy: requests from the presented browser tab
            // (or from a non-browser surface such as an lxapp page) present
            // the new tab; background browser tabs only add a sidebar row.
            let from_browser_tab = req.owner_appid == lingxia_browser::BUILTIN_BROWSER_APPID;
            let present = !from_browser_tab || presented_browser_tab().is_some();
            let url = req.url.clone();
            // May be called on a webview UI thread (NewWindowRequested);
            // hop onto the executor before touching tab/window state.
            std::mem::drop(lingxia::task::spawn(async move {
                open_browser_tab_for_open_url(&owner_appid, &url, present);
            }));
            true
        }
    }
}

/// Opens `url` as a new in-app browser tab owned by the shell app and, when
/// `present` is set, shows it over the main content card (same flow as the
/// sidebar rows). The tabs-changed observer keeps the sidebar in sync.
#[cfg(feature = "browser-runtime")]
fn open_browser_tab_for_open_url(owner_appid: &str, url: &str, present: bool) {
    let Some(app) = lxapp::try_get(owner_appid) else {
        log::warn!("no shell owner app for in-app open-url of {url}");
        return;
    };
    match lingxia_browser::open_for_app(owner_appid, app.session_id(), url, None) {
        Ok(tab_id) if present => present_browser_tab_when_ready(owner_appid, tab_id),
        Ok(_) => sync_shell_layout(owner_appid),
        Err(err) => log::error!("failed to open browser tab for {url}: {err}"),
    }
}

#[cfg(feature = "browser-runtime")]
fn schedule_browser_tabs_changed_sync() {
    let epoch = BROWSER_TAB_SYNC_EPOCH.fetch_add(1, Ordering::Relaxed) + 1;
    std::mem::drop(lingxia::task::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(
            BROWSER_TAB_SYNC_DEBOUNCE_MS,
        ))
        .await;
        if BROWSER_TAB_SYNC_EPOCH.load(Ordering::Relaxed) == epoch {
            on_browser_tabs_changed();
        }
    }));
}

/// Re-syncs the shell after any browser tab change: drops a stale
/// presentation when the presented tab disappeared and refreshes the
/// sidebar of the shell owner app.
#[cfg(feature = "browser-runtime")]
fn on_browser_tabs_changed() {
    if let Some(presented) = presented_browser_tab()
        && browser_tab_summary(&presented).is_none()
    {
        set_presented_browser_tab(None);
        if let Err(err) = restore_presented_group_main() {
            log::warn!("failed to restore main webview after browser tab close: {err}");
        }
    }
    if consume_suppressed_browser_tab_sync() {
        return;
    }
    if let Some(appid) = shell_owner_appid() {
        sync_shell_layout(&appid);
    }
}

fn sync_shell_layout(appid: &str) {
    if !is_shell_owner_appid(appid) {
        return;
    }

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
    install_shell_chrome_event_handler(&webtag, &app.appid);

    // Drive the device frame's status bar to match the active page: a visible
    // navigation bar extends its color up over the status bar (with its text
    // color); a plain page keeps the chrome-colored strip with contrasting text.
    if let Some(window) = owner_window_handle(appid) {
        let (foreground, background) = match layout.navigation_bar.as_ref().filter(|nav| nav.visible)
        {
            Some(nav) => (nav.text_color, nav.background_color),
            None => {
                let chrome = super::style::shell_palette().window_background;
                let luminance = (((chrome >> 16) & 0xff) * 299
                    + ((chrome >> 8) & 0xff) * 587
                    + (chrome & 0xff) * 114)
                    / 1000;
                let foreground = if luminance > 140 { 0x111111 } else { 0xf2f2f7 };
                (foreground, chrome)
            }
        };
        set_device_frame_status_bar_style(window, foreground, background);
    }

    if let Err(err) = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout)) {
        log::warn!(
            "failed to sync Windows shell layout for {}:{}: {}",
            appid,
            path,
            err
        );
    }
    // A presented browser tab re-installs chrome handling on its own webtag.
    #[cfg(feature = "browser-runtime")]
    if let Some(tab_id) = presented_browser_tab()
        && let Some(tab) = browser_tab_summary(&tab_id)
    {
        let browser_webtag = WebTag::new(
            lingxia_browser::BUILTIN_BROWSER_APPID,
            &tab.path,
            Some(tab.session_id),
        );
        install_shell_chrome_event_handler(&browser_webtag, &app.appid);
        let layout = build_window_layout(&app, &path);
        if let Err(err) =
            set_webview_window_layout(&browser_webtag, WindowsWindowLayout::new(layout))
        {
            log::warn!(
                "failed to sync Windows browser shell layout for {}:{}: {}",
                browser_webtag.extract_appid(),
                tab.path,
                err
            );
        }
    }
}

fn install_shell_chrome_event_handler(webtag: &WebTag, appid: &str) {
    let event_appid = appid.to_string();
    set_webview_chrome_event_handler(
        webtag,
        Arc::new(move |event| {
            handle_chrome_event(&event_appid, event);
        }),
    );
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
    let panel_activators = build_panel_activators(app);
    // A simulator-framed window (the runner) gets its window controls from the
    // device-frame toolbar, so the shell drops its own caption on that screen.
    let owner_window = owner_window_handle(&app.appid);
    let suppress_window_controls = owner_window.map(window_has_device_frame).unwrap_or(false);
    // Reserve the device frame's status-bar strip so the nav bar + content stack
    // below it (the status bar overlay owns the top strip), matching the macOS
    // runner's status-bar + nav-bar layout.
    let top_inset = owner_window.map(device_frame_status_bar_height).unwrap_or(0);
    WindowsShellWindowLayout {
        navigation_bar,
        address_bar,
        tab_bar: build_tab_bar_layout(app, !panel_activators.is_empty()),
        panel_activators,
        top_inset,
        // The product/app launcher icon (NOT the home lxapp's icon) for the
        // leading-edge app-menu button.
        app_icon_path: crate::app_icon::current_app_icon_path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        suppress_window_controls,
    }
}

fn prime_tabbar_selection(app: &LxApp, selected_index: usize) {
    let Some(tabbar) = app.get_tabbar() else {
        return;
    };
    let current_path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    if current_path.is_empty() {
        return;
    }

    let selected_index = selected_index as i32;
    let selected_path = tabbar
        .list
        .get(selected_index as usize)
        .map(|item| item.pagePath.clone());
    let event_appid = app.appid.clone();
    let handler = Arc::new(move |event| {
        handle_chrome_event(&event_appid, event);
    });
    let mut paths = vec![current_path];
    if let Some(selected_path) = selected_path {
        paths.push(selected_path);
    }
    paths.sort();
    paths.dedup();

    // Mirror the new selection onto each page's *own* chrome layout: the
    // outgoing page so its highlight moves the instant the item is clicked, and
    // the incoming page so its navigation bar and content rect are already
    // correct when its WebView is swapped into the host. Priming the incoming
    // webtag with the outgoing page's layout instead would show the outgoing
    // page's bar for a frame and then snap to the incoming one, which reads as
    // a jitter on tab click.
    for path in paths {
        if path.is_empty() {
            continue;
        }
        let mut layout = build_window_layout(app, &path);
        if let Some(tabbar_layout) = layout.tab_bar.as_mut() {
            tabbar_layout.selected_index = selected_index;
        }
        let webtag = WebTag::new(&app.appid, &path, Some(app.session_id()));
        set_webview_chrome_event_handler(&webtag, handler.clone());
        let _ = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout));
    }
}

/// Address-bar layout for the presented browser tab, or `None` while the
/// main surface shows an lxapp webview.
fn build_address_bar_layout() -> Option<WindowsShellAddressBarLayout> {
    let presented = presented_browser_tab()?;
    let tab = browser_tab_summary(&presented)?;
    Some(WindowsShellAddressBarLayout {
        visible: true,
        url_text: browser_tab_display_url(&tab),
    })
}

/// Capsule text of the presented tab: its current URL, else its title
/// (matching the sidebar row fallback).
fn browser_tab_display_url(tab: &BrowserTabSummary) -> String {
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

fn build_tab_bar_layout(
    app: &LxApp,
    has_panel_activators: bool,
) -> Option<WindowsShellTabBarLayout> {
    let tabbar = app.get_tabbar();
    let ui_state = sidebar_ui_state(&app.appid);
    let items = tabbar
        .as_ref()
        .map(|tabbar| {
            tabbar
                .list
                .iter()
                .map(|item| WindowsShellTabBarItemLayout {
                    page_path: item.pagePath.clone(),
                    text: item.text.clone().unwrap_or_default(),
                    icon_path: item.iconPath.clone().unwrap_or_default(),
                    selected_icon_path: item.selectedIconPath.clone().unwrap_or_default(),
                    badge: item.badge.clone(),
                    has_red_dot: item.has_red_dot,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut auxiliary_items = build_open_lxapp_items(&app.appid);
    auxiliary_items.extend(build_browser_tab_items());
    // The "+" opens a new browser tab, so it belongs to the full browser
    // environment only — not the device-framed dev runner, which hosts a
    // single lxapp with no browser.
    let owner_window = owner_window_handle(&app.appid);
    let device_framed = owner_window.map(window_has_device_frame).unwrap_or(false);
    // A home-indicator safe area is reserved below the bar only on devices that
    // have one (a tall status bar ⇒ Face-ID notch/island). Home-button devices
    // (short status bar, e.g. SE) and un-framed windows keep the legacy height.
    let has_home_indicator = if device_framed {
        owner_window.map(device_frame_status_bar_height).unwrap_or(0) > 30
    } else {
        true
    };
    let show_auxiliary_add = browser_runtime_enabled() && !device_framed;
    let header_actions = build_sidebar_header_actions();
    let sidebar_has_content =
        !items.is_empty() || !auxiliary_items.is_empty() || has_panel_activators;
    if !sidebar_has_content {
        return None;
    }
    // The LingXia icon is copied next to the app by the CLI; record its path so
    // the chrome can load it as the default icon (lxapp items / browser tabs
    // with no icon of their own).
    super::chrome::set_default_icon_path(
        app.runtime
            .asset_dir()
            .join("icons")
            .join("lingxia.png")
            .to_string_lossy()
            .into_owned(),
    );
    let position = tabbar_position(&app.appid);
    // `dimension` is the bar's cross-axis size: a sidebar's width, but a bottom
    // bar's *height*. A bottom bar is a compact icon+label strip, so it must not
    // borrow the (much taller) sidebar minimum width.
    let dimension = match position {
        WindowsShellTabBarPosition::Bottom => {
            BOTTOM_TABBAR_CONTENT_HEIGHT
                + if has_home_indicator {
                    HOME_INDICATOR_HEIGHT
                } else {
                    0
                }
        }
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right => tabbar
            .as_ref()
            .map(|tabbar| tabbar.dimension.max(MIN_SIDEBAR_WIDTH))
            .unwrap_or(MIN_SIDEBAR_WIDTH),
    };
    let tabbar_background = tabbar
        .as_ref()
        .map(|tabbar| tabbar.backgroundColor.as_str())
        .unwrap_or("#ffffff");
    Some(WindowsShellTabBarLayout {
        visible: true,
        position,
        dimension,
        app_name: app.runtime_info().app_name,
        app_icon_path: app.get_lxapp_info().icon,
        group_id: app.appid.clone(),
        collapsed: ui_state.collapsed,
        icon_rail: ui_state.icon_rail,
        items_collapsed: ui_state.items_collapsed,
        color: parse_css_color(
            tabbar
                .as_ref()
                .map(|tabbar| tabbar.color.as_str())
                .unwrap_or("#666666"),
            0x666666,
        ),
        selected_color: parse_css_color(
            tabbar
                .as_ref()
                .map(|tabbar| tabbar.selectedColor.as_str())
                .unwrap_or("#1677ff"),
            0x1677ff,
        ),
        // A "transparent" bottom bar can't truly composite over the opaque
        // WebView2 surface on Windows, and a color-key overlay fringes its
        // anti-aliased icons/labels (pink halos). Blend the strip into the
        // ambient shell background instead — a clean, opaque bar.
        background_color: if is_transparent_css_color(tabbar_background) {
            super::style::shell_palette().window_background
        } else {
            parse_css_color(tabbar_background, 0xffffff)
        },
        background_transparent: false,
        border_color: parse_css_color(
            tabbar
                .as_ref()
                .map(|tabbar| tabbar.borderStyle.as_str())
                .unwrap_or("#f0f0f0"),
            0xf0f0f0,
        ),
        selected_index: tabbar
            .as_ref()
            .map(|tabbar| tabbar.get_selected_index())
            .unwrap_or(0),
        items,
        auxiliary_items,
        show_auxiliary_add,
        header_actions,
    })
}

/// Sidebar header actions (settings / downloads), shown only when the
/// browser runtime backing their target pages is compiled in.
fn build_sidebar_header_actions() -> Vec<WindowsShellHeaderActionLayout> {
    if !browser_runtime_enabled() || !cfg!(feature = "browser-shell") {
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
    browser_tabs()
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

fn build_open_lxapp_items(owner_appid: &str) -> Vec<WindowsShellAuxiliaryItemLayout> {
    lxapp::list_lxapps()
        .into_iter()
        .filter(|info| !info.is_home)
        .filter(|info| info.appid != owner_appid)
        .filter(|info| !is_builtin_browser_appid(&info.appid))
        .filter(|info| matches!(info.status.as_str(), "opening" | "opened"))
        .map(|info| {
            let title = if info.app_name.trim().is_empty() {
                info.appid.clone()
            } else {
                info.app_name
            };
            WindowsShellAuxiliaryItemLayout {
                id: format!("{AUX_LXAPP_PREFIX}{}", info.appid),
                title,
                active: false,
                icon_png: None,
            }
        })
        .collect()
}

fn auxiliary_lxapp_id(raw: &str) -> Option<&str> {
    raw.strip_prefix(AUX_LXAPP_PREFIX)
        .map(str::trim)
        .filter(|appid| !appid.is_empty())
}

/// Sidebar row title for a browser tab: page title, else the URL host,
/// else "New Tab".
fn browser_tab_display_title(tab: &BrowserTabSummary) -> String {
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
                .map(|item| {
                    // Prefer the activator's own declared icon; when it has
                    // none, fall back to the target lxapp's app icon (via the
                    // app-info API, like macOS). The renderer then falls back
                    // to the default LingXia mark if neither resolves.
                    let icon_path = if !item.icon.trim().is_empty() {
                        resolve_asset_path(asset_dir, &item.icon)
                            .map(|path| path.to_string_lossy().to_string())
                            .unwrap_or_else(|| item.icon.clone())
                    } else if item.content.kind.is_lxapp() {
                        lxapp::try_get(&item.content.app_id)
                            .map(|app| app.get_lxapp_info().icon)
                            .filter(|icon| !icon.trim().is_empty())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    WindowsShellPanelActivatorLayout {
                        id: item.id.clone(),
                        label: item.label,
                        icon_path,
                        position: panel_position(item.position),
                        active: is_panel_visible(&item.id),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn handle_chrome_event(appid: &str, event: WindowsChromeCommand) {
    if !is_shell_owner_appid(appid) {
        log::debug!("ignoring Windows shell chrome event for non-owner app {appid}");
        return;
    }
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
            prime_tabbar_selection(&app, index);
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
            if let Some(target_appid) = auxiliary_lxapp_id(&tab_id) {
                handle_lxapp_auxiliary_click(appid, target_appid);
                return;
            }
            handle_browser_tab_click(appid, &tab_id);
            return;
        }
        chrome_command::BROWSER_TAB_CLOSE => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            if let Some(target_appid) = auxiliary_lxapp_id(&tab_id) {
                handle_lxapp_auxiliary_close(appid, target_appid);
                return;
            }
            handle_browser_tab_close(appid, &tab_id);
            return;
        }
        chrome_command::SIDEBAR_AUXILIARY_CONTEXT_MENU => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            let Some(target_appid) = auxiliary_lxapp_id(&tab_id) else {
                return;
            };
            let screen_x = payload_i32(&event, "screen_x").unwrap_or(0);
            let screen_y = payload_i32(&event, "screen_y").unwrap_or(0);
            show_lxapp_auxiliary_context_menu(appid, target_appid, screen_x, screen_y);
            return;
        }
        chrome_command::BROWSER_PANEL_CLOSE => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            crate::window_host::close_webview_panel(&panel_id);
            sync_shell_layout(appid);
            return;
        }
        chrome_command::BROWSER_PANEL_NAV_BACK => {
            let Some(tab_id) = payload_browser_panel_tab_id(&event) else {
                return;
            };
            browser_go_back(&tab_id);
            return;
        }
        chrome_command::BROWSER_PANEL_NAV_FORWARD => {
            let Some(tab_id) = payload_browser_panel_tab_id(&event) else {
                return;
            };
            browser_go_forward(&tab_id);
            return;
        }
        chrome_command::BROWSER_PANEL_NAV_RELOAD => {
            let Some(tab_id) = payload_browser_panel_tab_id(&event) else {
                return;
            };
            browser_reload(&tab_id);
            return;
        }
        chrome_command::BROWSER_PANEL_ADDRESS_BAR => {
            let Some(webtag_key) = payload_string(&event, "webtag_key") else {
                return;
            };
            let Some(tab_id) = payload_browser_panel_tab_id(&event) else {
                return;
            };
            // Editing a browser aside's address bar is a browser-only action.
            #[cfg(feature = "browser-runtime")]
            begin_browser_panel_address_edit(appid, &webtag_key, &tab_id);
            #[cfg(not(feature = "browser-runtime"))]
            let _ = (&webtag_key, &tab_id);
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
        chrome_command::NATIVE_PANEL_PANE_FOCUS => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(screen_x) = payload_i32(&event, "screen_x") else {
                return;
            };
            let Some(screen_y) = payload_i32(&event, "screen_y") else {
                return;
            };
            if let Some((cx, cy)) = screen_to_panel_client(appid, screen_x, screen_y) {
                super::terminal_panel::focus_pane_at(&panel_id, cx, cy);
            }
            return;
        }
        // Address-bar navigation targets the presented browser tab; URL and
        // title updates flow back through the tabs-changed observer.
        chrome_command::BROWSER_NAV_BACK => {
            if let Some(tab_id) = presented_browser_tab() {
                browser_go_back(&tab_id);
            }
            return;
        }
        chrome_command::BROWSER_NAV_FORWARD => {
            if let Some(tab_id) = presented_browser_tab() {
                browser_go_forward(&tab_id);
            }
            return;
        }
        chrome_command::BROWSER_NAV_RELOAD => {
            if let Some(tab_id) = presented_browser_tab() {
                browser_reload(&tab_id);
            }
            return;
        }
        chrome_command::BROWSER_ADDRESS_BAR => {
            begin_presented_tab_address_edit(&app);
            return;
        }
        chrome_command::SIDEBAR_TOGGLE => {
            // User toggle is two-state: expanded <-> icon rail. Fully hidden
            // sidebars are controlled by content-driven auto-hide only.
            update_sidebar_ui_state(appid, |state| {
                state.collapsed = false;
                state.icon_rail = !state.icon_rail;
            });
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
        chrome_command::APP_MENU_CLICK => {
            let screen_x = payload_i32(&event, "screen_x").unwrap_or(0);
            let screen_y = payload_i32(&event, "screen_y").unwrap_or(0);
            show_app_menu(appid, &app, screen_x, screen_y);
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

fn payload_browser_panel_tab_id(command: &WindowsChromeCommand) -> Option<String> {
    let webtag_key = payload_string(command, "webtag_key")?;
    browser_tab_id_for_webtag_key(&webtag_key).or_else(|| {
        log::warn!("browser aside webtag has no tab: {webtag_key}");
        None
    })
}

#[cfg(feature = "browser-runtime")]
fn browser_tab_id_for_webtag_key(webtag_key: &str) -> Option<String> {
    browser_tabs().into_iter().find_map(|tab| {
        let webtag = WebTag::new(
            lingxia_browser::BUILTIN_BROWSER_APPID,
            &tab.path,
            Some(tab.session_id),
        );
        (webtag.key() == webtag_key).then_some(tab.tab_id)
    })
}

#[cfg(not(feature = "browser-runtime"))]
fn browser_tab_id_for_webtag_key(_webtag_key: &str) -> Option<String> {
    None
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
#[cfg(feature = "browser-runtime")]
fn handle_browser_new_tab(appid: &str, session_id: u64) {
    match lingxia_browser::open_for_app(appid, session_id, "lingxia://newtab", None) {
        Ok(tab_id) => present_browser_tab_when_ready(appid, tab_id),
        Err(err) => log::error!("failed to open new browser tab for {appid}: {err}"),
    }
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_new_tab(_appid: &str, _session_id: u64) {}

#[cfg(feature = "browser-runtime")]
fn handle_browser_tab_click(appid: &str, tab_id: &str) {
    let active_changed = lingxia_browser::current_tab()
        .map(|tab| tab.tab_id != tab_id)
        .unwrap_or(true);
    if active_changed {
        suppress_next_browser_tab_sync();
    }
    if lingxia_browser::activate(tab_id).is_err() {
        if active_changed {
            let _ = consume_suppressed_browser_tab_sync();
        }
        log::warn!("browser tab no longer exists: {tab_id}");
        sync_shell_layout(appid);
        return;
    }
    present_browser_tab_when_ready(appid, tab_id.to_string());
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_tab_click(_appid: &str, _tab_id: &str) {}

#[cfg(feature = "browser-runtime")]
fn handle_browser_tab_close(appid: &str, tab_id: &str) {
    if presented_browser_tab().as_deref() == Some(tab_id) {
        return_to_lxapp_from_browser(appid);
    }
    if let Err(err) = lingxia_browser::close(tab_id) {
        log::error!("failed to close browser tab {tab_id}: {err}");
    }
    // The tabs-changed observer re-syncs as well; sync directly so the row
    // disappears even if no observer is installed.
    sync_shell_layout(appid);
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_tab_close(_appid: &str, _tab_id: &str) {}

fn handle_lxapp_auxiliary_click(owner_appid: &str, target_appid: &str) {
    return_to_lxapp_from_browser(owner_appid);
    if let Some((panel_id, path)) = panel_item_for_lxapp(target_appid) {
        register_managed_aside(owner_appid, &panel_id);
        if let Some(panel) = lxapp::try_get(target_appid) {
            let path = panel
                .peek_current_page()
                .unwrap_or_else(|| non_empty(Some(&path)).unwrap_or_else(|| panel.initial_route()));
            if let Err(err) = panel.runtime.show_lxapp(
                target_appid.to_string(),
                path,
                panel.session_id(),
                LxAppOpenMode::Panel,
                panel_id.clone(),
            ) {
                log::error!("failed to show sidebar lxapp panel {target_appid}: {err}");
            }
        } else if let Err(err) = lxapp::open_lxapp(
            target_appid,
            LxAppStartupOptions::new(&path)
                .set_open_mode(LxAppOpenMode::Panel)
                .set_panel_id(panel_id.clone()),
        ) {
            log::error!("failed to open sidebar lxapp panel {target_appid}: {err}");
        }
    } else {
        log::warn!(
            "sidebar lxapp {target_appid} has no panel slot; leaving current surface unchanged"
        );
    }
    sync_shell_layout(owner_appid);
    sync_shell_layout(target_appid);
}

fn handle_lxapp_auxiliary_close(owner_appid: &str, target_appid: &str) {
    if let Err(err) = lxapp::close_lxapp(target_appid) {
        log::error!("failed to close sidebar lxapp {target_appid}: {err}");
    }
    if let Some((panel_id, _)) = panel_item_for_lxapp(target_appid) {
        if let Err(err) = hide_host_panel(&panel_id) {
            log::warn!("failed to hide sidebar lxapp panel {panel_id}: {err}");
        }
        unregister_managed_aside(owner_appid, &panel_id);
    }
    sync_shell_layout(owner_appid);
}

fn show_lxapp_auxiliary_context_menu(
    owner_appid: &str,
    target_appid: &str,
    screen_x: i32,
    screen_y: i32,
) {
    let Some(window) = owner_window_handle(owner_appid) else {
        return;
    };
    let Some(target) = lxapp::try_get(target_appid) else {
        log::warn!("sidebar lxapp context menu ignored; lxapp is not active: {target_appid}");
        return;
    };
    let info = target.get_lxapp_info();
    let version = info.version.trim();
    let version_item = if version.is_empty() {
        "Version".to_string()
    } else {
        format!("Version {version}")
    };
    #[cfg(feature = "browser-shell")]
    let about = AboutInfo {
        title: "About".to_string(),
        app_name: if info.app_name.trim().is_empty() {
            target_appid.to_string()
        } else {
            info.app_name.clone()
        },
        version_line: version_item.clone(),
        icon_path: info.icon.clone(),
    };
    let target_appid = target_appid.to_string();
    let items = vec![
        version_item,
        String::new(),
        "Restart".to_string(),
        "Clean Cache && Restart".to_string(),
    ];
    super::context_menu::show_context_menu_checked(
        window,
        (screen_x, screen_y),
        items,
        Vec::new(),
        Arc::new(move |index| match index {
            0 => {
                #[cfg(feature = "browser-shell")]
                show_about_dialog(window, &about);
            }
            2 => {
                if let Err(err) = restart_lxapp_in_place(&target_appid) {
                    log::warn!("failed to restart sidebar lxapp {target_appid}: {err}");
                }
            }
            3 => {
                if let Err(err) = clear_lxapp_user_cache(&target_appid)
                    .and_then(|_| restart_lxapp_in_place(&target_appid))
                {
                    log::warn!(
                        "failed to clean cache + reload sidebar lxapp {target_appid}: {err}"
                    );
                }
            }
            _ => {}
        }),
    );
}

/// Whether `window` is wrapped in a simulator device frame. Always `false` when
/// the `device-frame` feature is off — production shell hosts never are, so they
/// don't compile (or depend on) the device-frame module.
#[cfg(feature = "device-frame")]
fn window_has_device_frame(window: isize) -> bool {
    crate::device_frame::window_has_device_frame(window)
}

#[cfg(not(feature = "device-frame"))]
fn window_has_device_frame(_window: isize) -> bool {
    false
}

/// Height of the device frame's simulated status bar for `window` (0 when the
/// window is not framed or the device has no status bar). The shell reserves
/// this strip at the top so the navigation bar + content sit below the status
/// bar instead of under it.
#[cfg(feature = "device-frame")]
fn device_frame_status_bar_height(window: isize) -> i32 {
    crate::device_frame::device_frame_status_bar_height(window)
}

#[cfg(not(feature = "device-frame"))]
fn device_frame_status_bar_height(_window: isize) -> i32 {
    0
}

#[cfg(feature = "device-frame")]
fn set_device_frame_status_bar_style(window: isize, foreground: u32, background: u32) {
    crate::device_frame::set_device_frame_status_bar_style(window, foreground, background);
}

#[cfg(not(feature = "device-frame"))]
fn set_device_frame_status_bar_style(_window: isize, _foreground: u32, _background: u32) {}

fn restart_lxapp_in_place(appid: &str) -> Result<(), String> {
    lxapp::try_get(appid)
        .ok_or_else(|| format!("lxapp is not active: {appid}"))?
        .restart_in_place()
        .map_err(|err| err.to_string())
}

fn clear_lxapp_user_cache(appid: &str) -> Result<(), String> {
    lxapp::try_get(appid)
        .ok_or_else(|| format!("lxapp is not active: {appid}"))?
        .clear_user_cache()
        .map_err(|err| err.to_string())
}

/// Presents `tab_id`'s webview over the main content card, retrying while
/// the tab's WebView creation is still in flight (new tabs create their
/// webview asynchronously).
#[cfg(feature = "browser-runtime")]
fn present_browser_tab_when_ready(appid: &str, tab_id: String) {
    let owner_appid = appid.to_string();
    std::mem::drop(lingxia::task::spawn(async move {
        for attempt in 0..PRESENT_BROWSER_TAB_MAX_RETRY {
            let Some(tab) = browser_tab_summary(&tab_id) else {
                // Tab was closed while waiting.
                return;
            };
            let webtag = WebTag::new(
                lingxia_browser::BUILTIN_BROWSER_APPID,
                &tab.path,
                Some(tab.session_id),
            );
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
    }));
}

/// Opens the browser page behind a sidebar header action (settings /
/// downloads) as a presented browser tab.
#[cfg(feature = "browser-runtime")]
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

#[cfg(not(feature = "browser-runtime"))]
fn handle_sidebar_action(_appid: &str, _session_id: u64, _action_id: &str) {}

/// Presents `url` as a browser page: when a tab already shows it, that tab
/// is activated and presented; otherwise a new tab opens at `url` (same
/// flow as the sidebar "New Tab" row, just with a target URL).
#[cfg(feature = "browser-runtime")]
fn open_or_present_browser_page(appid: &str, session_id: u64, url: &str) {
    let existing = browser_tabs()
        .into_iter()
        .find(|tab| tab.current_url.as_deref() == Some(url));
    if let Some(existing) = existing {
        handle_browser_tab_click(appid, &existing.tab_id);
        return;
    }
    match lingxia_browser::open_for_app(appid, session_id, url, None) {
        Ok(tab_id) => present_browser_tab_when_ready(appid, tab_id),
        Err(err) => log::error!("failed to open browser page {url} for {appid}: {err}"),
    }
}

/// Opens the app-menu popup under the top-bar app icon. The product shell adds
/// an About entry (app name + version) above Exit; the dev runner ships the
/// shell chrome without a product identity, so it only offers Exit. Keeping
/// About behind `browser-shell` also keeps `TaskDialogIndirect` (comctl32 v6)
/// out of `browser-runtime`-only hosts, which do not embed that manifest.
#[cfg(feature = "shell-chrome")]
fn show_app_menu(appid: &str, app: &LxApp, screen_x: i32, screen_y: i32) {
    let Some(window) = owner_window_handle(appid) else {
        return;
    };

    let exit = || {
        if let Err(err) = lingxia::app::exit() {
            log::warn!("failed to exit from Windows app menu: {err}");
        }
    };

    #[cfg(feature = "browser-shell")]
    {
        // About shows the *product* (app) identity from the app config -
        // productName / productVersion and the launcher icon - NOT the home
        // lxapp's name/version/icon. Falls back to the lxapp's values only when
        // the app config is unavailable.
        let lxapp_info = app.get_lxapp_info();
        let app_name =
            non_empty(lingxia_app_context::product_name()).unwrap_or(lxapp_info.app_name);
        let version =
            non_empty(lingxia_app_context::product_version()).unwrap_or(lxapp_info.version);
        let icon_path = crate::app_icon::current_app_icon_path()
            .map(|path| path.to_string_lossy().into_owned())
            .filter(|path| !path.is_empty())
            .unwrap_or(lxapp_info.icon);
        let about = AboutInfo {
            title: "About".to_string(),
            app_name,
            version_line: format!("Version {version}"),
            icon_path,
        };
        let items = vec!["About".to_string(), "Exit".to_string()];
        super::context_menu::show_context_menu_checked(
            window,
            (screen_x, screen_y),
            items,
            Vec::new(),
            Arc::new(move |index| match index {
                0 => show_about_dialog(window, &about),
                1 => exit(),
                _ => {}
            }),
        );
    }

    #[cfg(not(feature = "browser-shell"))]
    {
        let _ = app;
        let items = vec!["Exit".to_string()];
        super::context_menu::show_context_menu_checked(
            window,
            (screen_x, screen_y),
            items,
            Vec::new(),
            Arc::new(move |index| {
                if index == 0 {
                    exit();
                }
            }),
        );
    }
}

#[cfg(feature = "browser-shell")]
struct AboutInfo {
    title: String,
    app_name: String,
    version_line: String,
    /// Resolved absolute path to the app's declared icon; empty if none.
    icon_path: String,
}

/// Shows the About dialog owned by the shell window, on the window's UI
/// thread (the popup's selection callback runs there). Uses a task dialog
/// carrying the app's own icon; if the task dialog is unavailable it falls
/// back to a plain message box (with no generic system icon).
#[cfg(feature = "browser-shell")]
fn show_about_dialog(window: isize, about: &AboutInfo) {
    use std::path::Path;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Controls::{
        TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
        TDF_POSITION_RELATIVE_TO_WINDOW, TDF_USE_HICON_MAIN, TaskDialogIndirect,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
    use windows::core::PCWSTR;

    let hwnd = HWND(window as *mut core::ffi::c_void);

    // Prefer the app's declared (clean) icon, loaded fresh; fall back to the
    // shared process icon handle. `owns_icon` tracks which to destroy.
    let from_path = (!about.icon_path.trim().is_empty())
        .then(|| crate::app_icon::create_icon_handle_from_path(Path::new(&about.icon_path), 64))
        .flatten();
    let (icon_handle, owns_icon) = match from_path {
        Some(handle) => (handle, true),
        None => (
            crate::app_icon::current_large_icon_handle().unwrap_or(0),
            false,
        ),
    };

    let title = to_wide(&about.title);
    let instruction = to_wide(&about.app_name);
    let content = to_wide(&about.version_line);

    // TASKDIALOGCONFIG is packed, so the whole struct is built as one literal
    // (mutating a field in place would take an unaligned reference).
    let mut flags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
    let main_icon = if icon_handle != 0 {
        flags |= TDF_USE_HICON_MAIN;
        TASKDIALOGCONFIG_0 {
            hMainIcon: HICON(icon_handle as *mut core::ffi::c_void),
        }
    } else {
        TASKDIALOGCONFIG_0::default()
    };
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: hwnd,
        dwFlags: flags,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(instruction.as_ptr()),
        pszContent: PCWSTR(content.as_ptr()),
        Anonymous1: main_icon,
        ..Default::default()
    };

    let shown = unsafe { TaskDialogIndirect(&config, None, None, None) }.is_ok();

    if owns_icon && icon_handle != 0 {
        unsafe {
            let _ = DestroyIcon(HICON(icon_handle as *mut core::ffi::c_void));
        }
    }

    if !shown {
        show_about_message_box(hwnd, &about.title, &about.app_name, &about.version_line);
    }
}

/// Plain message box fallback. Deliberately uses no `MB_ICON*` flag so it
/// carries no generic system icon.
#[cfg(feature = "browser-shell")]
fn show_about_message_box(
    hwnd: windows::Win32::Foundation::HWND,
    title: &str,
    app_name: &str,
    version_line: &str,
) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};
    use windows::core::PCWSTR;

    let body = to_wide(&format!("{app_name}\n{version_line}"));
    let title = to_wide(title);
    unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK,
        );
    }
}

#[cfg(feature = "browser-shell")]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Trims `value` and returns it owned only when non-empty.
#[cfg(feature = "shell-chrome")]
fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(feature = "shell-chrome")]
pub(super) fn owner_window_handle(appid: &str) -> Option<isize> {
    let app = lxapp::try_get(appid)?;
    let path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    let webtag = WebTag::new(&app.appid, &path, Some(app.session_id()));
    let snapshot = Some(lingxia_windows_host::webview_window_snapshot(&webtag));
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

/// Converts a screen point to `appid`'s shell-window client coordinates,
/// matching the coordinate space the chrome paints panels in (used to
/// focus the terminal pane under the cursor on right-click). `None` when
/// the window handle is unavailable or the point is off-window.
#[cfg(feature = "shell-chrome")]
pub(super) fn screen_to_panel_client(
    appid: &str,
    screen_x: i32,
    screen_y: i32,
) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    let hwnd = owner_window_handle(appid)?;
    let mut point = POINT {
        x: screen_x,
        y: screen_y,
    };
    let ok = unsafe {
        ScreenToClient(
            windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void),
            &mut point,
        )
    };
    ok.as_bool().then_some((point.x, point.y))
}

#[cfg(feature = "browser-runtime")]
fn begin_presented_tab_address_edit(app: &LxApp) {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    // The capsule was painted by the shell-owner window's chrome; its host
    // window handle comes from the owner webtag's window snapshot.
    let Some(window) = owner_window_handle(&app.appid) else {
        return;
    };

    let owner_appid = app.appid.clone();
    let initial = tab.current_url.clone().unwrap_or_default();
    super::begin_address_edit(
        window,
        &initial,
        Arc::new(move |text: String| {
            commit_address_input(&owner_appid, &tab_id, &text);
        }),
    );
}

#[cfg(not(feature = "browser-runtime"))]
fn begin_presented_tab_address_edit(_app: &LxApp) {
    // Without the shell chrome no address bar is drawn (plain OS frame),
    // so there is nothing to edit.
}

/// Starts an inline URL edit over a browser aside's address capsule, prefilled
/// with the tab's current URL. The commit reuses `commit_address_input`, so the
/// aside navigates through the same address-input engine as the main bar.
#[cfg(feature = "browser-runtime")]
fn begin_browser_panel_address_edit(appid: &str, webtag_key: &str, tab_id: &str) {
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    let initial = browser_tab_summary(tab_id)
        .and_then(|tab| tab.current_url.clone())
        .unwrap_or_default();
    let owner_appid = appid.to_string();
    let tab_id = tab_id.to_string();
    super::begin_panel_address_edit(
        window,
        webtag_key,
        &initial,
        Arc::new(move |text: String| {
            commit_address_input(&owner_appid, &tab_id, &text);
        }),
    );
}

/// Resolves a committed address input and navigates the presented tab.
/// Runs on the host window's UI thread (inline-edit commit); the actual
/// navigation hops onto the executor so webview work never blocks that
/// thread.
#[cfg(feature = "browser-runtime")]
fn commit_address_input(appid: &str, tab_id: &str, raw_input: &str) {
    if raw_input.trim().is_empty() {
        return;
    }
    let response = resolve_input(BrowserAddressInputRequest {
        raw_input: raw_input.to_string(),
        trigger: BrowserAddressInputTrigger::Submit,
        context: BrowserAddressInputContext::default(),
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
    std::mem::drop(lingxia::task::spawn(async move {
        if let Err(err) = navigate_browser_tab(&tab_id, &navigation.url) {
            log::error!("failed to navigate browser tab {tab_id}: {err}");
        }
        // The tabs-changed observer re-syncs as well; sync directly so the
        // capsule reflects the committed URL even without an observer.
        sync_shell_layout(&appid);
    }));
}

fn set_managed_surface_visible(panel_id: &str, visible: bool) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    if panel_target_for_id(panel_id).is_none() {
        return false;
    }
    if is_panel_visible(panel_id) == visible {
        sync_shell_layout(&owner_appid);
        return true;
    }
    handle_panel_activator(&owner_appid, panel_id.to_string());
    true
}

fn toggle_managed_surface(panel_id: &str) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    if panel_target_for_id(panel_id).is_none() {
        return false;
    }
    handle_panel_activator(&owner_appid, panel_id.to_string());
    true
}

#[cfg(feature = "browser-shell")]
pub(crate) fn handle_menu_bar_surface_action(surface_id: &str, action_kind: &str) -> bool {
    if panel_target_for_id(surface_id).is_some() {
        return match action_kind {
            "openSurface" | "focusSurface" => set_managed_surface_visible(surface_id, true),
            "closeSurface" => set_managed_surface_visible(surface_id, false),
            _ => toggle_managed_surface(surface_id),
        };
    }

    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    if surface_id != owner_appid {
        return false;
    }

    if action_kind == "closeSurface" {
        if let Some(window) = owner_window_handle(&owner_appid) {
            return crate::window_host::hide_host_window(window);
        }
        return false;
    }
    if action_kind == "focusSurface" {
        return owner_window_handle(&owner_appid)
            .is_some_and(crate::window_host::restore_and_focus_host_window);
    }
    if action_kind == "toggleSurface"
        && let Some(window) = owner_window_handle(&owner_appid)
        && crate::window_host::host_window_is_visible(window)
    {
        return crate::window_host::hide_host_window(window);
    }

    let opened = open_home_app(&owner_appid).is_ok();
    if let Some(window) = owner_window_handle(&owner_appid) {
        return crate::window_host::restore_and_focus_host_window(window) || opened;
    }
    opened
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
        unregister_managed_aside(appid, &panel_id);
        lxapp::mark_lxapp_active(appid);
        sync_shell_layout(appid);
        return;
    }

    if !pending_panel_opens().insert(panel_id.clone()) {
        return;
    }

    let owner_appid = appid.to_string();
    register_managed_aside(&owner_appid, &panel_id);
    std::mem::drop(lingxia::task::spawn(async move {
        let result = open_panel_lxapp(&panel_id, &panel_appid, &path).await;
        pending_panel_opens().remove(&panel_id);
        if let Err(err) = result {
            log::error!("failed to open Windows panel lxapp {panel_appid}: {err}");
            unregister_managed_aside(&owner_appid, &panel_id);
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

fn panel_item_for_lxapp(appid: &str) -> Option<(String, String)> {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| {
            panels.items.into_iter().find_map(|item| {
                (item.content.kind.is_lxapp() && item.content.app_id == appid)
                    .then_some((item.id, item.content.path.unwrap_or_default()))
            })
        })
}

fn handle_terminal_panel_activator(appid: &str, request: TerminalPanelRequest) {
    let position = panel_position(request.position);
    if is_panel_visible(&request.panel_id) {
        if let Err(err) = hide_host_panel(&request.panel_id) {
            log::warn!(
                "failed to hide Windows terminal panel {}: {}",
                request.panel_id,
                err
            );
        }
        unregister_managed_aside(appid, &request.panel_id);
        sync_shell_layout(appid);
        return;
    }

    let title = if request.label.trim().is_empty() {
        "Terminal"
    } else {
        request.label.trim()
    };
    if let Ok(true) = super::terminal_panel::show_existing_windows_terminal_panel(
        &request.panel_id,
        title,
        position,
    ) {
        register_managed_aside(appid, &request.panel_id);
        sync_shell_layout(appid);
        return;
    }
    if let Err(err) =
        super::terminal_panel::open_windows_terminal_panel(&request.panel_id, title, position)
    {
        log::warn!(
            "failed to show Windows terminal panel {}: {}",
            request.panel_id,
            err
        );
        return;
    }
    register_managed_aside(appid, &request.panel_id);
    sync_shell_layout(appid);
}

fn register_managed_aside(appid: &str, panel_id: &str) {
    let Some(edge) = panel_edge_for_id(panel_id) else {
        return;
    };
    if let Some(app) = lxapp::try_get(appid) {
        app.register_host_aside(panel_id, edge);
    }
}

fn unregister_managed_aside(appid: &str, panel_id: &str) {
    if let Some(app) = lxapp::try_get(appid) {
        app.unregister_host_aside(panel_id);
    }
}

fn panel_edge_for_id(panel_id: &str) -> Option<&'static str> {
    let item = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id))?;
    Some(match item.position {
        lingxia_app_context::PanelPosition::Left => "left",
        lingxia_app_context::PanelPosition::Right => "right",
        lingxia_app_context::PanelPosition::Top => "top",
        lingxia_app_context::PanelPosition::Bottom => "bottom",
    })
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
        lingxia_app_context::PanelPosition::Top => WindowsPanelPosition::Top,
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
    if value.is_empty() || is_transparent_css_color(value) {
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

fn is_transparent_css_color(raw: &str) -> bool {
    raw.trim().eq_ignore_ascii_case("transparent")
}
