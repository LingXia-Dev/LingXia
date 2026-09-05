use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
// Atomics here back browser tab-sync debounce/presentation generations and
// shell-created terminal workspace identities.
#[cfg(feature = "browser-runtime")]
use std::sync::atomic::AtomicBool;
#[cfg(any(feature = "browser-runtime", feature = "terminal-runtime"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellFooterActionLayout,
    WindowsShellHeaderActionLayout, WindowsShellNavigationBarLayout,
    WindowsShellSidebarActionSource, WindowsShellTabBarItemLayout, WindowsShellTabBarLayout,
    WindowsShellTabBarPosition, WindowsShellWindowLayout,
};
#[cfg(feature = "browser-runtime")]
use lingxia_browser::BrowserTabInfo;
#[cfg(feature = "browser-runtime")]
use lingxia_browser_shell::{
    BrowserAddressInputContext, BrowserAddressInputRequest, BrowserAddressInputTrigger,
    resolve_input,
};
use lingxia_platform::error::PlatformError;
use lingxia_platform::traits::app_runtime::{
    AppRuntime, LxAppOpenMode, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_platform::traits::ui::{ManagedSurfaceCompletion, ManagedSurfaceFuture};
use lingxia_shell::{
    ResolvedShellSidebarAction, ShellPin, ShellPinTarget, SidebarActionIntent,
    SidebarActionPlacement,
};
use lingxia_surface::{
    Edge, LayoutPresentationPlan, SizeClass, SlotKind, SurfaceIcon, SurfaceSwitcherItem,
    SurfaceSwitcherSnapshot, SwitcherContentKind,
};
use lingxia_webview::WebTag;
#[cfg(feature = "browser-runtime")]
use lingxia_webview::platform::windows::find_webview_handler;
#[cfg(feature = "browser-runtime")]
use lingxia_windows_contract::current_window_layout;
use lingxia_windows_contract::{
    WindowsAsidePanelEvent, WindowsChromeCommand, WindowsHostWindow, WindowsPanelPosition,
    WindowsWindowLayout, active_host_window_webtag_key, dispatch_windows_aside_panel_event,
    hide_host_panel, is_panel_visible, present_webview_in_active_group,
    restore_presented_group_main, set_webview_chrome_event_handler, set_webview_window_layout,
};
use lxapp::{LxApp, LxAppDelegate, LxAppStartupOptions, LxAppUiEventType};
#[cfg(feature = "browser-runtime")]
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

const DEFAULT_NAV_BAR_HEIGHT: i32 = 38;
const MIN_SIDEBAR_WIDTH: i32 = 184;
const MAX_SIDEBAR_WIDTH: i32 = 400;
/// Bottom tab bar height (icons + labels). The strip sits just above the
/// content area's bottom, which is already inset for the home-indicator safe
/// area, so no extra height is reserved here.
const BOTTOM_TABBAR_CONTENT_HEIGHT: i32 = 49;

/// How many times to retry presenting a freshly opened browser tab whose
/// WebView creation is still in flight, and the delay between attempts.
#[cfg(feature = "browser-runtime")]
const PRESENT_BROWSER_TAB_MAX_RETRY: u32 = 30;
#[cfg(feature = "browser-runtime")]
const PRESENT_BROWSER_TAB_RETRY_DELAY_MS: u64 = 100;
#[cfg(feature = "browser-runtime")]
const BROWSER_TAB_SYNC_DEBOUNCE_MS: u64 = 180;
#[cfg(feature = "browser-runtime")]
const BROWSER_FIRST_FRAME_GUARD_MS: u64 = 75;
#[cfg(feature = "browser-runtime")]
const BROWSER_TAB_MEMORY_SHARE: u64 = 4;
#[cfg(feature = "browser-runtime")]
const ESTIMATED_BROWSER_TAB_BYTES: u64 = 256 * 1024 * 1024;
#[cfg(feature = "browser-runtime")]
const MIN_LIVE_BROWSER_TABS: usize = 4;
#[cfg(feature = "browser-runtime")]
const MAX_LIVE_BROWSER_TABS: usize = 16;
#[cfg(feature = "browser-runtime")]
const DEFAULT_LIVE_BROWSER_TABS: usize = 8;

struct PresentationCompletion(Option<ManagedSurfaceCompletion>);

impl PresentationCompletion {
    fn finish(&mut self, result: Result<(), PlatformError>) {
        if let Some(completion) = self.0.take() {
            completion(result);
        }
    }
}

impl Drop for PresentationCompletion {
    fn drop(&mut self) {
        self.finish(Err(PlatformError::CallbackDropped));
    }
}

fn accept_managed_request(
    completion: ManagedSurfaceCompletion,
    start: impl FnOnce(ManagedSurfaceCompletion) -> bool,
    rejected: PlatformError,
) -> bool {
    let completion = Arc::new(Mutex::new(PresentationCompletion(Some(completion))));
    let completion_for_result = completion.clone();
    let accepted = start(Box::new(move |result| {
        if let Ok(mut completion) = completion_for_result.lock() {
            completion.finish(result);
        }
    }));
    if !accepted && let Ok(mut completion) = completion.lock() {
        completion.finish(Err(rejected));
    }
    true
}

/// Panel ids whose lxapp open is still in flight, used to ignore repeated
/// footer action clicks until the open completes.
static PENDING_PANEL_OPENS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static PENDING_PANEL_COMPLETIONS: OnceLock<Mutex<HashMap<String, Vec<ManagedSurfaceCompletion>>>> =
    OnceLock::new();

/// The lxapp that owns the main shell window (set when the home app opens
/// and refreshed on every chrome event); browser tab-change notifications
/// re-sync this app's layout.
static SHELL_OWNER_APPID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
/// `None` means the runtime writer has not declared a list yet. `Some([])`
/// intentionally clears both sidebar action regions.
static RUNTIME_SIDEBAR_ACTIONS: OnceLock<Mutex<Option<Vec<ResolvedShellSidebarAction>>>> =
    OnceLock::new();
static RUNTIME_SHELL_PINS: OnceLock<Mutex<Vec<ShellPin>>> = OnceLock::new();

/// Browser tab currently presented over the main content card, if any.
static PRESENTED_BROWSER_TAB: OnceLock<Mutex<Option<String>>> = OnceLock::new();
/// One-shot intent published before an explicit lxapp main activation. The
/// layout plan itself intentionally does not encode browser-cover state, so
/// this distinguishes a user/API switch from unrelated adaptive commits.
static REQUESTED_LXAPP_MAIN_ACTIVATION: OnceLock<Mutex<Option<String>>> = OnceLock::new();
/// LxApp group that was expanded under the presented browser tab. Browser
/// selection must not silently replace/collapse that navigation group.
static PRESENTED_BROWSER_GROUP_APPID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
#[cfg(feature = "terminal-runtime")]
static PRESENTED_NATIVE_MAIN: OnceLock<Mutex<Option<WebTag>>> = OnceLock::new();
#[cfg(feature = "terminal-runtime")]
static NEXT_SHELL_TERMINAL_WORKSPACE_KEY: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "browser-runtime")]
static SUPPRESSED_BROWSER_TAB_SYNCS: OnceLock<Mutex<u32>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static BROWSER_TAB_SYNC_EPOCH: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "browser-runtime")]
static BROWSER_PRESENT_EPOCH: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "browser-runtime")]
static SELF_BROWSER_HOST: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "browser-runtime")]
static SELF_BROWSER_ROOT_TAB: OnceLock<Mutex<Option<String>>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static DECLARED_BROWSER_TABS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static BROWSER_TAB_MEMORY_STATE: OnceLock<Mutex<BrowserTabMemoryState>> = OnceLock::new();
#[cfg(feature = "browser-runtime")]
static LIVE_BROWSER_TAB_LIMIT: OnceLock<usize> = OnceLock::new();
static DEFAULT_TABBAR_POSITION: OnceLock<Mutex<WindowsShellTabBarPosition>> = OnceLock::new();
static TABBAR_POSITION_OVERRIDES: OnceLock<Mutex<HashMap<String, WindowsShellTabBarPosition>>> =
    OnceLock::new();
/// Stable shared order of main lxapp and browser tabs. The currently selected
/// lxapp expands in place instead of jumping above every web tab.
static MAIN_TAB_ORDER: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

#[cfg(feature = "browser-runtime")]
#[derive(Default)]
struct BrowserTabMemoryState {
    /// Activation order, least-recently-used first.
    recency: Vec<String>,
    /// Tabs whose WebView has been destroyed while their sidebar row remains.
    discarded: HashSet<String>,
}

const AUX_LXAPP_PREFIX: &str = "lxapp:";
const AUX_PINNED_LXAPP_PREFIX: &str = "pin:lxapp:";
const AUX_BOOKMARK_PREFIX: &str = "bookmark:";
const AUX_SURFACE_PREFIX: &str = "surface:";
const SHELL_TERMINAL_SURFACE_ID: &str = "shell:terminal";

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
    can_go_back: bool,
    can_go_forward: bool,
}

fn browser_runtime_enabled() -> bool {
    cfg!(feature = "browser-runtime")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MainWorkspaceAddTarget {
    Browser,
    Terminal { declaration_id: String },
}

fn main_workspace_add_target(switcher: &SurfaceSwitcherSnapshot) -> Option<MainWorkspaceAddTarget> {
    main_workspace_add_target_for_capabilities(
        switcher,
        browser_runtime_enabled(),
        cfg!(feature = "terminal-runtime") && lingxia_app_context::terminal_enabled(),
    )
}

fn main_workspace_add_target_for_capabilities(
    switcher: &SurfaceSwitcherSnapshot,
    browser_enabled: bool,
    terminal_enabled: bool,
) -> Option<MainWorkspaceAddTarget> {
    let active = switcher.active_surface_id.as_deref().and_then(|active_id| {
        switcher
            .items
            .iter()
            .find(|item| item.surface_id == active_id)
    });
    if terminal_enabled
        && active.is_some_and(|item| {
            matches!(
                &item.content,
                SwitcherContentKind::Native { capability } if capability == "terminal"
            )
        })
        && let Some(declaration_id) = switcher.root_surface_id.as_deref().filter(|root_id| {
            switcher.items.iter().any(|item| {
                item.surface_id == *root_id
                    && matches!(
                        &item.content,
                        SwitcherContentKind::Native { capability } if capability == "terminal"
                    )
            })
        })
    {
        return Some(MainWorkspaceAddTarget::Terminal {
            declaration_id: declaration_id.to_string(),
        });
    }
    browser_enabled.then_some(MainWorkspaceAddTarget::Browser)
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
        can_go_back: info.can_go_back,
        can_go_forward: info.can_go_forward,
    }
}

#[cfg(feature = "browser-runtime")]
fn browser_tabs() -> Vec<BrowserTabSummary> {
    lingxia_browser::tabs()
        .into_iter()
        // Standalone tabs (docked aside panels) are independent of the main
        // tab model; the sidebar lists only main-area browser tabs.
        .filter(|tab| !lingxia_browser::tab_is_standalone(&tab.tab_id))
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

#[cfg(feature = "browser-runtime")]
fn live_browser_tab_limit_for_memory(total_physical_bytes: u64) -> usize {
    ((total_physical_bytes / BROWSER_TAB_MEMORY_SHARE) / ESTIMATED_BROWSER_TAB_BYTES)
        .try_into()
        .unwrap_or(usize::MAX)
        .clamp(MIN_LIVE_BROWSER_TABS, MAX_LIVE_BROWSER_TABS)
}

#[cfg(feature = "browser-runtime")]
fn live_browser_tab_limit() -> usize {
    *LIVE_BROWSER_TAB_LIMIT.get_or_init(|| {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `status` has the required size in `dwLength` and remains
        // valid for the duration of the synchronous Win32 call.
        unsafe { GlobalMemoryStatusEx(&mut status) }
            .map(|()| live_browser_tab_limit_for_memory(status.ullTotalPhys))
            .unwrap_or(DEFAULT_LIVE_BROWSER_TABS)
    })
}

#[cfg(feature = "browser-runtime")]
fn touch_browser_tab_recency(recency: &mut Vec<String>, tab_id: &str) {
    recency.retain(|candidate| candidate != tab_id);
    recency.push(tab_id.to_string());
}

#[cfg(feature = "browser-runtime")]
fn record_browser_tab_recency(tab_id: &str) {
    if let Ok(mut state) = BROWSER_TAB_MEMORY_STATE
        .get_or_init(|| Mutex::new(BrowserTabMemoryState::default()))
        .lock()
    {
        touch_browser_tab_recency(&mut state.recency, tab_id);
    }
}

#[cfg(feature = "browser-runtime")]
fn browser_tab_discard_candidates(
    live_tab_ids: &HashSet<String>,
    recency: &[String],
    discarded: &HashSet<String>,
    protected: &HashSet<String>,
    limit: usize,
) -> Vec<String> {
    let live_count = live_tab_ids
        .iter()
        .filter(|tab_id| !discarded.contains(*tab_id))
        .count();
    let excess = live_count.saturating_sub(limit);
    recency
        .iter()
        .filter(|tab_id| {
            live_tab_ids.contains(*tab_id)
                && !discarded.contains(*tab_id)
                && !protected.contains(*tab_id)
        })
        .take(excess)
        .cloned()
        .collect()
}

/// Keeps only a memory-scaled number of browser WebViews alive. Older
/// background tabs retain their metadata/sidebar entry and are recreated when
/// presented again.
#[cfg(feature = "browser-runtime")]
fn enforce_browser_tab_memory_limit(recent_tab_id: Option<&str>) {
    let tabs = browser_tabs();
    let ordered_tab_ids: Vec<String> = tabs.into_iter().map(|tab| tab.tab_id).collect();
    let live_tab_ids: HashSet<String> = ordered_tab_ids.iter().cloned().collect();
    let mut protected = HashSet::new();
    if let Some(tab_id) = lingxia_browser::current_tab().map(|tab| tab.tab_id) {
        protected.insert(tab_id);
    }
    if let Some(tab_id) = presented_browser_tab() {
        protected.insert(tab_id);
    }
    if let Some(tab_id) = recent_tab_id {
        protected.insert(tab_id.to_string());
    }

    let candidates = {
        let state =
            BROWSER_TAB_MEMORY_STATE.get_or_init(|| Mutex::new(BrowserTabMemoryState::default()));
        let Ok(mut state) = state.lock() else {
            return;
        };
        state.recency.retain(|tab_id| live_tab_ids.contains(tab_id));
        state
            .discarded
            .retain(|tab_id| live_tab_ids.contains(tab_id));
        // Normally every tab was already observed by the raw tabs-changed
        // callback. The ordered snapshot is only a deterministic bootstrap
        // fallback for tabs that predated handler installation.
        for tab_id in &ordered_tab_ids {
            if !state.recency.contains(tab_id) {
                state.recency.push(tab_id.clone());
            }
        }
        if let Some(tab_id) = recent_tab_id.filter(|tab_id| live_tab_ids.contains(*tab_id)) {
            touch_browser_tab_recency(&mut state.recency, tab_id);
        }
        browser_tab_discard_candidates(
            &live_tab_ids,
            &state.recency,
            &state.discarded,
            &protected,
            live_browser_tab_limit(),
        )
    };

    for tab_id in candidates {
        match lingxia_browser::discard(&tab_id) {
            Ok(()) => {
                if let Ok(mut state) = BROWSER_TAB_MEMORY_STATE
                    .get_or_init(|| Mutex::new(BrowserTabMemoryState::default()))
                    .lock()
                {
                    state.discarded.insert(tab_id);
                }
            }
            Err(err) => log::warn!("failed to discard background browser tab {tab_id}: {err}"),
        }
    }
}

#[cfg(feature = "browser-runtime")]
fn reactivate_browser_tab_if_needed(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    lingxia_browser::reactivate(tab_id)?;
    if let Ok(mut state) = BROWSER_TAB_MEMORY_STATE
        .get_or_init(|| Mutex::new(BrowserTabMemoryState::default()))
        .lock()
    {
        state.discarded.remove(tab_id);
    }
    enforce_browser_tab_memory_limit(Some(tab_id));
    Ok(())
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
    pub(super) const TAB_BAR_MORE_CLICK: &str = "tabbar.more.click";
    pub(super) const FOOTER_ACTION_CLICK: &str = "sidebar-footer-action.click";
    pub(super) const STATIC_SETTINGS_CLICK: &str = "static-settings.click";
    pub(super) const NAVIGATION_BACK: &str = "navigation.back";
    pub(super) const NAVIGATION_HOME: &str = "navigation.home";
    pub(super) const BROWSER_NEW_TAB: &str = "browser.new-tab";
    pub(super) const MAIN_WORKSPACE_ADD: &str = "main-workspace.add";
    pub(super) const BROWSER_TAB_CLICK: &str = "browser.tab.click";
    pub(super) const BROWSER_TAB_CLOSE: &str = "browser.tab.close";
    pub(super) const SIDEBAR_AUXILIARY_CONTEXT_MENU: &str = "sidebar.auxiliary.context-menu";
    pub(super) const BROWSER_PANEL_CLOSE: &str = "browser-panel.close";
    pub(super) const BROWSER_PANEL_NAV_BACK: &str = "browser-panel.nav.back";
    pub(super) const BROWSER_PANEL_NAV_FORWARD: &str = "browser-panel.nav.forward";
    pub(super) const BROWSER_PANEL_NAV_RELOAD: &str = "browser-panel.nav.reload";
    pub(super) const ASIDE_PANEL_TAB_CLICK: &str = "aside-panel.tab.click";
    pub(super) const ASIDE_PANEL_TAB_CLOSE: &str = "aside-panel.tab.close";
    pub(super) const ASIDE_PANEL_COLLAPSE: &str = "aside-panel.collapse";
    pub(super) const ASIDE_PANEL_NAV_BACK: &str = "aside-panel.nav.back";
    pub(super) const ASIDE_PANEL_NAV_FORWARD: &str = "aside-panel.nav.forward";
    pub(super) const ASIDE_PANEL_NAV_RELOAD: &str = "aside-panel.nav.reload";
    pub(super) const NATIVE_PANEL_TAB_CLICK: &str = "native-panel.tab.click";
    pub(super) const NATIVE_PANEL_TAB_CLOSE: &str = "native-panel.tab.close";
    pub(super) const NATIVE_PANEL_NEW_TAB: &str = "native-panel.new-tab";
    pub(super) const NATIVE_PANEL_MAXIMIZE: &str = "native-panel.maximize";
    pub(super) const NATIVE_PANEL_TAB_RENAME: &str = "native-panel.tab.rename";
    pub(super) const NATIVE_PANEL_RIGHT_CLICK: &str = "native-panel.right-click";
    pub(super) const NATIVE_PANEL_PANE_FOCUS: &str = "native-panel.pane-focus";
    pub(super) const BROWSER_TABS_CYCLE: &str = "browser.tabs.cycle";
    pub(super) const BROWSER_NAV_BACK: &str = "browser.nav.back";
    pub(super) const BROWSER_NAV_FORWARD: &str = "browser.nav.forward";
    pub(super) const BROWSER_NAV_RELOAD: &str = "browser.nav.reload";
    pub(super) const BROWSER_ADDRESS_BAR: &str = "browser.address-bar";
    pub(super) const BROWSER_BOOKMARK_TOGGLE: &str = "browser.bookmark.toggle";
    pub(super) const BROWSER_PIN_TOGGLE: &str = "browser.pin.toggle";
    pub(super) const BROWSER_PAGE_MENU: &str = "browser.page-menu";
    pub(super) const BROWSER_CLOSE: &str = "browser.close";
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
    pub(super) const SIDEBAR_SCROLL: &str = "sidebar.scroll";
    pub(super) const FOOTER_ACTION_SCROLL: &str = "sidebar-footer-action.scroll";
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
    /// Explicit reveal while the adaptive medium projection would otherwise
    /// cap the sidebar at an icon rail. Reset on the next size-class crossing.
    medium_expanded: bool,
    items_collapsed: bool,
    main_scroll_offset: i32,
    footer_action_scroll_row: usize,
    /// Whether `icon_rail` has been filled in from the persisted choice yet.
    /// Startup touches this state before the shell store exists, so the entry
    /// can outlive its own seeding and must say so rather than look settled.
    seeded: bool,
}

static SIDEBAR_UI_STATE: OnceLock<Mutex<HashMap<String, SidebarUiState>>> = OnceLock::new();

/// The group's live state, seeded from what the user last settled on.
///
/// Only their own rail choice is restored; `medium_expanded` and the scroll
/// offsets are per-session, and the adaptive rail is re-derived from the window
/// every launch. Startup writes to this state before the shell store is open —
/// the size-class reset among others — so an entry can exist before it can be
/// seeded, and seeding is retried until the store answers rather than keyed on
/// the entry merely existing.
fn seeded_sidebar_ui_state<'a>(
    state: &'a mut HashMap<String, SidebarUiState>,
    group: &str,
) -> &'a mut SidebarUiState {
    let entry = state.entry(group.to_string()).or_default();
    if !entry.seeded
        && let Ok(manager) = lingxia_shell::manager()
    {
        entry.icon_rail = manager.sidebar_chrome().rail();
        entry.seeded = true;
    }
    entry
}

fn sidebar_ui_state(group: &str) -> SidebarUiState {
    let state = SIDEBAR_UI_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut state) = state.lock() else {
        return SidebarUiState::default();
    };
    *seeded_sidebar_ui_state(&mut state, group)
}

/// Write down the user's own sidebar choice, never the adaptive projection.
fn persist_sidebar_chrome(rail: bool) {
    let expanded_width = lingxia_shell::sidebar_chrome().expanded_width;
    if let Err(error) = lingxia_shell::set_sidebar_chrome(
        lingxia_shell::SidebarChrome::with_expanded(!rail, expanded_width),
    ) {
        log::warn!("could not persist the sidebar mode: {error}");
    }
}

fn persisted_expanded_sidebar_width() -> i32 {
    lingxia_shell::sidebar_chrome()
        .expanded_width
        .round()
        .clamp(f64::from(MIN_SIDEBAR_WIDTH), f64::from(MAX_SIDEBAR_WIDTH)) as i32
}

fn update_sidebar_ui_state(group: &str, update: impl FnOnce(&mut SidebarUiState)) {
    let state = SIDEBAR_UI_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        update(seeded_sidebar_ui_state(&mut state, group));
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

pub(crate) fn set_shell_owner_app_id(appid: &str) {
    set_shell_owner_appid(appid);
}

fn is_shell_owner_appid(appid: &str) -> bool {
    shell_owner_appid()
        .as_deref()
        .map(|owner| owner == appid)
        .unwrap_or(false)
}

pub(crate) fn open_home_app_with_target(
    appid: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
) -> Result<(), String> {
    set_shell_owner_appid(appid);
    #[cfg(feature = "terminal-runtime")]
    if lingxia_app_context::terminal_enabled() {
        super::terminal_panel::ensure_configuration_loaded();
    }
    let options = LxAppStartupOptions::for_page(page, query)?;
    let app = lxapp::open_lxapp(appid, options).map_err(|err| err.to_string())?;
    // A restarted lxapp cannot reuse browser WebViews attached to its previous
    // session. Persistent pins remain in the bookmark store and reopen cleanly.
    // If the presented tab is among the pruned, the tabs-changed handler drops
    // the stale presentation and restores the main webview.
    #[cfg(feature = "browser-runtime")]
    let _ = lingxia_browser::prune_stale_owner_tabs(&app.appid, app.session_id());
    #[cfg(not(feature = "browser-runtime"))]
    let _ = app;
    Ok(())
}

/// Opens the browser as the host's primary self-managed content. Unlike a URL
/// surface, this uses the existing browser tab model and its editable chrome.
#[cfg(feature = "browser-runtime")]
pub(crate) fn open_self_browser(url: &str) -> Result<(), String> {
    SELF_BROWSER_HOST.store(true, Ordering::Release);
    let tab_id = match lingxia_browser::open(url, None) {
        Ok(tab_id) => tab_id,
        Err(error) => {
            SELF_BROWSER_HOST.store(false, Ordering::Release);
            return Err(error.to_string());
        }
    };
    let Some(browser) = lxapp::try_get(lingxia_browser::BUILTIN_BROWSER_APPID) else {
        SELF_BROWSER_HOST.store(false, Ordering::Release);
        return Err("built-in browser runtime is not ready".to_string());
    };
    if let Ok(mut root) = SELF_BROWSER_ROOT_TAB
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *root = Some(tab_id.clone());
    }
    set_shell_owner_appid(&browser.appid);
    present_browser_tab_when_ready(&browser.appid, tab_id);
    Ok(())
}

#[cfg(not(feature = "browser-runtime"))]
pub(crate) fn open_self_browser(_url: &str) -> Result<(), String> {
    Err("managed self browser is not enabled in this host".to_string())
}

/// Opens a declared browser main while the home lxapp remains the shell owner.
#[cfg(feature = "browser-runtime")]
pub(crate) fn open_declared_browser(
    owner_appid: &str,
    surface_id: &str,
    url: &str,
    completion: Option<ManagedSurfaceCompletion>,
) -> Result<(), String> {
    let mut completion = completion;
    SELF_BROWSER_HOST.store(false, Ordering::Release);
    set_shell_owner_appid(owner_appid);
    let owner = lxapp::try_get(owner_appid)
        .ok_or_else(|| format!("home control lxapp is not active: {owner_appid}"))?;
    let existing = DECLARED_BROWSER_TABS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|tabs| tabs.get(surface_id).cloned())
        .filter(|tab_id| browser_tab_summary(tab_id).is_some());
    if let Some(tab_id) = existing {
        let _ = lingxia_browser::activate(&tab_id);
        present_declared_browser_tab_when_ready(
            owner_appid,
            surface_id,
            tab_id,
            false,
            completion.take(),
        );
        return Ok(());
    }
    let tab_id = lingxia_browser::open_for_app(owner_appid, owner.session_id(), url, None)
        .map_err(|error| error.to_string())?;
    if let Ok(mut tabs) = DECLARED_BROWSER_TABS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        tabs.insert(surface_id.to_string(), tab_id.clone());
    }
    present_declared_browser_tab_when_ready(
        owner_appid,
        surface_id,
        tab_id,
        url == "about:blank",
        completion.take(),
    );
    Ok(())
}

#[cfg(feature = "terminal-runtime")]
pub(crate) fn open_declared_terminal(owner_appid: &str, surface_id: &str) -> Result<(), String> {
    set_shell_owner_appid(owner_appid);
    let owner = lxapp::try_get(owner_appid)
        .ok_or_else(|| format!("home control lxapp is not active: {owner_appid}"))?;
    let layout = content_agnostic_window_layout(&owner);
    let webtag = WebTag::new(owner_appid, &format!("__native_main/{surface_id}"), None);
    // Seed the complete terminal layout before revealing the native-only host.
    // Showing the HWND first exposes an empty workspace until the panel is
    // registered and maximized on the following statements.
    super::terminal_panel::open_windows_terminal_panel(
        surface_id,
        &lingxia_logic::i18n::t(lingxia_logic::I18nKey::TerminalTitle),
        WindowsPanelPosition::Bottom,
    )?;
    crate::window_host::set_host_panel_zoom_control_visible(surface_id, false);
    if !lingxia_windows_contract::set_host_panel_maximized(surface_id, true) {
        super::terminal_panel::destroy_windows_terminal_panel(surface_id);
        return Err(format!(
            "failed to maximize native terminal surface '{surface_id}'"
        ));
    }
    install_shell_chrome_event_handler(&webtag, &owner.appid);
    if let Err(error) = crate::window_host::show_native_main_window(&webtag, layout) {
        super::terminal_panel::destroy_windows_terminal_panel(surface_id);
        return Err(error);
    }
    if let Ok(mut presented) = PRESENTED_NATIVE_MAIN
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *presented = Some(webtag);
    }
    Ok(())
}

fn shell_owner_appid() -> Option<String> {
    SHELL_OWNER_APPID
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

fn resolved_shell_size_class(fallback: Option<&LxApp>) -> SizeClass {
    shell_owner_appid()
        .and_then(|appid| lxapp::try_get(&appid))
        .and_then(|owner| owner.surface_derived_layout())
        .or_else(|| fallback.and_then(LxApp::surface_derived_layout))
        .map(|plan| plan.size_class)
        .unwrap_or(SizeClass::Expanded)
}

/// Push the host window's logical (DIP) content width into the shell-owner
/// app's adaptive surface graph so the size class - and therefore the aside
/// projection (Compact overlay / Medium 1 / Expanded 3) - tracks the real window. Without
/// this the graph stays at its seed width (permanently Medium), so a second
/// aside evicts the first even on a wide window. Called from the host's
/// `WM_SIZE`.
pub(crate) fn update_surface_width(logical_width: f64) {
    if logical_width <= 0.0 {
        return;
    }
    if let Some(appid) = shell_owner_appid() {
        let group_appid = preferred_sidebar_group_appid(
            shell_owner_appid(),
            presented_browser_group_appid(),
            active_main_lxapp_id(),
        )
        .unwrap_or_else(|| appid.clone());
        let previous_size_class = lxapp::try_get(&appid)
            .and_then(|app| app.surface_derived_layout())
            .map(|plan| plan.size_class);
        let sidebar_width = current_sidebar_width(&group_appid);
        let size_class_changed =
            lingxia::windows::set_surface_layout_metrics(&appid, logical_width, sidebar_width);
        let resolved_size_class = lxapp::try_get(&appid)
            .and_then(|app| app.surface_derived_layout())
            .map(|plan| plan.size_class);
        if previous_size_class != resolved_size_class {
            update_sidebar_ui_state(&group_appid, |state| {
                state.medium_expanded = false;
            });
        }

        // The width update can cross a size-class boundary, changing the
        // sidebar projection itself (full -> rail -> hidden). Feed that newly
        // resolved width back into admission so the same resize converges
        // without waiting for another WM_SIZE.
        let resolved_sidebar_width = current_sidebar_width(&group_appid);
        if (resolved_sidebar_width - sidebar_width).abs() > f64::EPSILON {
            lingxia::windows::set_surface_sidebar_width(&appid, resolved_sidebar_width);
        }

        // WM_SIZE immediately lays out the native host after this returns. If
        // the width crossed a breakpoint, rebuild its cached chrome first;
        // otherwise that pass combines the new surface projection with the
        // previous size class's sidebar/phone bar until another shell event.
        if size_class_changed {
            sync_shell_layout(&appid);
        }
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
    // Other apps' layouts embed this app's tab bar (a presented lxapp's
    // window layout carries the shell owner's sidebar), so re-sync the owner
    // and the current app too — not just `appid` — or a device switch that
    // updates positions app-by-app leaves the visible layout built against
    // a stale position.
    sync_related_shell_layouts(appid);
}

#[cfg(feature = "device-frame")]
pub(crate) fn set_tabbar_position_on_window_thread(
    appid: &str,
    position: WindowsShellTabBarPosition,
) {
    let overrides = TABBAR_POSITION_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut overrides) = overrides.lock() {
        overrides.insert(appid.to_string(), position);
    }
    sync_related_shell_layouts(appid);
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

#[cfg(feature = "terminal-runtime")]
pub(super) fn terminal_surface_is_protected_root(panel_id: &str) -> bool {
    shell_owner_appid()
        .and_then(|appid| lxapp::try_get(&appid))
        .and_then(|owner| owner.surface_switcher_snapshot().root_surface_id)
        .as_deref()
        == Some(panel_id)
}

#[cfg(feature = "terminal-runtime")]
pub(super) fn terminal_surface_presentation(panel_id: &str) -> &'static str {
    let is_main = shell_owner_appid()
        .and_then(|appid| lxapp::try_get(&appid))
        .is_some_and(|owner| owner.main_surface_content(panel_id).is_some());
    if is_main { "main" } else { "aside" }
}

/// Completes the graph/provider transaction after the final PTY in a
/// non-root terminal surface closes. Main workspaces select and present their
/// successor; asides are removed from the graph so a later layout commit
/// cannot resurrect the dead provider.
#[cfg(feature = "terminal-runtime")]
pub(super) fn close_exhausted_terminal_surface(panel_id: &str) -> bool {
    let Some(appid) = shell_owner_appid() else {
        return false;
    };
    let Some(owner) = lxapp::try_get(&appid) else {
        return false;
    };
    if owner.main_surface_content(panel_id).is_some() {
        return close_main_surface_and_present(&owner, panel_id, "user");
    }
    super::terminal_panel::destroy_windows_terminal_panel(panel_id);
    unregister_managed_aside(&appid, panel_id);
    sync_shell_layout(&appid);
    true
}

fn presented_browser_tab() -> Option<String> {
    PRESENTED_BROWSER_TAB
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

fn set_presented_browser_tab(tab_id: Option<String>) {
    if tab_id.is_none()
        && let Ok(mut group) = PRESENTED_BROWSER_GROUP_APPID
            .get_or_init(|| Mutex::new(None))
            .lock()
    {
        *group = None;
    }
    let slot = PRESENTED_BROWSER_TAB.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = tab_id;
    }
}

fn request_lxapp_main_activation(appid: &str) {
    if let Ok(mut requested) = REQUESTED_LXAPP_MAIN_ACTIVATION
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *requested = Some(appid.to_string());
    }
}

fn take_lxapp_main_activation(appid: &str) -> bool {
    let Some(mut requested) = REQUESTED_LXAPP_MAIN_ACTIVATION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
    else {
        return false;
    };
    if requested.as_deref() != Some(appid) {
        return false;
    }
    requested.take();
    true
}

fn presented_browser_group_appid() -> Option<String> {
    PRESENTED_BROWSER_GROUP_APPID
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

fn set_presented_browser_group_appid(appid: Option<String>) {
    if let Ok(mut slot) = PRESENTED_BROWSER_GROUP_APPID
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *slot = appid;
    }
}

fn preferred_sidebar_group_appid(
    owner: Option<String>,
    presented_group: Option<String>,
    active_main: Option<String>,
) -> Option<String> {
    owner.or(presented_group).or(active_main)
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
    LxApp {
        appid: String,
        path: String,
        page: Option<String>,
        query: Option<serde_json::Value>,
        position: lingxia_app_context::PanelPosition,
    },
    Terminal(TerminalPanelRequest),
}

pub(super) fn install() {
    lingxia_platform::set_windows_ui_update_handler(Arc::new(|appid| {
        sync_related_shell_layouts(&appid);
    }));
    // Awaited `lx.tabBar.update()` calls: run the layout sync off
    // the caller's thread and complete the callback once it has applied.
    lingxia_platform::set_windows_ui_update_async_handler(Arc::new(|appid, done| {
        std::mem::drop(lingxia::task::spawn(async move {
            sync_related_shell_layouts(&appid);
            done(true);
        }));
    }));
    // Page Chrome capsule measurement: a framed simulated phone answers with
    // its floating pill's page-space rect, so a custom-navigation page lays
    // out around the capsule the way it does on iOS. Hosts without a device
    // frame keep answering None.
    #[cfg(feature = "device-frame")]
    lingxia_platform::set_windows_capsule_rect_provider(Arc::new(|_appid| {
        crate::device_frame::device_frame_capsule_page_rect()
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
        // Record every raw event before the layout debounce collapses bursts;
        // otherwise multiple background opens would be initialized from an
        // unordered snapshot and the first eviction would not be true LRU.
        if let Some(tab) = lingxia_browser::current_tab() {
            record_browser_tab_recency(&tab.tab_id);
        }
        schedule_browser_tabs_changed_sync();
    }));
    #[cfg(feature = "browser-shell")]
    lingxia_browser_shell::set_bookmarks_change_listener(Box::new(|| {
        if let Some(appid) = shell_owner_appid() {
            sync_shell_layout(&appid);
        }
    }));
    // Re-render chrome labels when the user changes the display language:
    // `lingxia_logic::i18n::t` resolves through `lxapp::display_language`,
    // so a layout re-sync is all a language switch needs.
    lxapp::add_display_language_effective_listener(Box::new(|_| {
        if let Some(appid) = shell_owner_appid() {
            sync_shell_layout(&appid);
        }
    }));
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::set_tab_present_handler(Arc::new(|tab_id| {
        let Some(owner_appid) = shell_owner_appid() else {
            return;
        };
        present_browser_tab_when_ready(&owner_appid, tab_id.to_string());
    }));
    // Keep in-app open-url targets (new-window requests from browser tabs,
    // lxapp openURL with self/new_browser_tab) inside the app as browser
    // tabs; unhandled requests fall back to the OS shell handler.
    lingxia_platform::set_windows_open_url_handler(Arc::new(handle_open_url_request));
    lingxia_platform::set_windows_close_browser_tab_handler(Arc::new(handle_close_browser_tab));
    lingxia_platform::set_windows_activate_browser_tab_handler(Arc::new(
        handle_activate_browser_tab,
    ));
    lingxia_platform::set_windows_managed_surface_visible_handler(Arc::new(
        set_managed_surface_visible_for_api,
    ));
    lingxia_platform::set_windows_managed_surface_close_handler(Arc::new(
        close_managed_surface_for_api,
    ));
    lingxia_platform::set_windows_managed_native_surface_open_handler(Arc::new(
        open_managed_native_surface_for_api,
    ));
    lingxia_platform::set_windows_sidebar_actions_handler(Arc::new(set_runtime_sidebar_actions));
    lingxia_platform::set_windows_builtin_browser_downloads_handler(Arc::new(
        open_builtin_browser_downloads,
    ));
    lingxia_platform::set_windows_shell_pins_handler(Arc::new(set_runtime_shell_pins));
    lingxia_platform::set_windows_lxapp_main_activation_handler(Arc::new(
        request_lxapp_main_activation,
    ));
    lingxia_platform::set_windows_layout_plan_handler(Arc::new(apply_windows_layout_plan));
    lingxia_platform::set_windows_managed_aside_event_handler(Arc::new(handle_managed_aside_event));
    if lingxia_shell::manager().is_ok_and(|manager| manager.snapshot().sidebar_actions.declared()) {
        let _ = lingxia_shell::apply_current_sidebar_actions();
    }
    let _ = lingxia_shell::apply_current_pins();

    // Deliver lx.surface closes (user window-close or programmatic) back to the
    // logic layer, mirroring the apple/android/harmony FFI bridges: drop the
    // graph node first (recommitting the layout plan), then resolve the JS
    // Surface handle so onClose fires. Without the forget, the closed aside
    // stays in the graph and its next open merges into a zombie node.
    lingxia_platform::set_windows_surface_closed_handler(Arc::new(|id, reason| {
        let (appid, _, _) = lxapp::get_current_lxapp();
        if let Some(app) = lxapp::try_get(&appid) {
            let _ = app.forget_surface(id);
        }
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

fn sync_related_shell_layouts(appid: &str) {
    let mut appids = Vec::from([appid.to_string()]);
    if let Some(owner_appid) = shell_owner_appid()
        && !appids.iter().any(|appid| appid == &owner_appid)
    {
        appids.push(owner_appid);
    }
    let current_appid = lxapp::get_current_lxapp().0;
    if !current_appid.is_empty() && !appids.iter().any(|appid| appid == &current_appid) {
        appids.push(current_appid);
    }
    for appid in appids {
        sync_app_shell_layout(&appid);
    }
}

/// Shell chrome state (sidebar rows and collapse, footer actions, the
/// presented browser tab) is shared by every webtag that renders the shell,
/// and the visible layout may belong to a presented non-owner lxapp. Any
/// state change therefore re-syncs the whole related set — the app, the
/// shell owner, and the current app — never just one layout, or the change
/// only shows up after the next unrelated sync.
fn sync_shell_layout(appid: &str) {
    if let Some(owner_appid) = shell_owner_appid() {
        let group_appid = preferred_sidebar_group_appid(
            shell_owner_appid(),
            presented_browser_group_appid(),
            active_main_lxapp_id(),
        )
        .unwrap_or_else(|| appid.to_string());
        let sidebar_width = current_sidebar_width(&group_appid);
        if let Some(logical_width) = crate::window_host::primary_shell_logical_client_width() {
            lingxia::windows::set_surface_layout_metrics(
                &owner_appid,
                logical_width,
                sidebar_width,
            );
        } else {
            lingxia::windows::set_surface_sidebar_width(&owner_appid, sidebar_width);
        }
    }
    sync_related_shell_layouts(appid);
}

fn current_sidebar_width(group_appid: &str) -> f64 {
    let app = lxapp::try_get(group_appid)
        .or_else(|| shell_owner_appid().and_then(|appid| lxapp::try_get(&appid)));
    let Some(app) = app else {
        return 0.0;
    };
    let owner = shell_owner_app_for(&app);
    let shell_app = owner.as_deref().unwrap_or(&app);
    let footer_actions = build_footer_actions(shell_app);
    build_tab_bar_layout(&app, &footer_actions)
        .filter(|tabbar| {
            matches!(
                tabbar.position,
                WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
            )
        })
        .map(|tabbar| super::chrome::sidebar_column_width(&tabbar) as f64)
        .unwrap_or(0.0)
}

/// Routes `open_url` requests with in-app targets into the internal
/// browser. Returns `None` (let the platform open the system handler)
/// for explicit external targets or when no shell/browser is available.
fn handle_open_url_request(
    req: &OpenUrlRequest,
) -> Result<Option<lingxia_platform::traits::app_runtime::OpenUrlResult>, PlatformError> {
    match req.target {
        OpenUrlTarget::External => Ok(None),
        // In-app targets are routed into the internal browser; without the
        // browser engine there is nowhere in-app to open them, so defer to the
        // OS handler.
        #[cfg(not(feature = "browser-runtime"))]
        OpenUrlTarget::SelfTarget | OpenUrlTarget::NewBrowserTab | OpenUrlTarget::AsideBrowser => {
            Ok(None)
        }
        #[cfg(feature = "browser-runtime")]
        OpenUrlTarget::SelfTarget | OpenUrlTarget::NewBrowserTab | OpenUrlTarget::AsideBrowser => {
            let Some(owner_appid) = shell_owner_appid() else {
                return Ok(None);
            };
            // Presentation policy: requests from the presented browser tab
            // (or from a non-browser surface such as an lxapp page) present
            // the new tab; background browser tabs only add a sidebar row.
            let from_browser_tab = req.owner_appid == lingxia_browser::BUILTIN_BROWSER_APPID;
            let present = !from_browser_tab || presented_browser_tab().is_some();
            // A compact-degraded URL aside keeps aside chrome in the shared
            // in-app browser (no address bar while the tab is active).
            let aside = req.target == OpenUrlTarget::AsideBrowser;
            let url = req.url.clone();
            if req.want_tab_id {
                let owner_session_id = lxapp::try_get(&owner_appid)
                    .map(|owner| owner.session_id())
                    .ok_or_else(|| {
                        PlatformError::Platform(format!(
                            "shell owner app is not active: {owner_appid}"
                        ))
                    })?;
                // JS is waiting for an identity; create on this thread (the
                // Logic executor, not a WebView UI thread) so close() can
                // address the tab immediately.
                let opened = if aside {
                    lingxia_browser::open_aside_for_app(&owner_appid, owner_session_id, &url, None)
                } else {
                    lingxia_browser::open_for_app(&owner_appid, owner_session_id, &url, None)
                };
                return match opened {
                    Ok(tab_id) => {
                        if present {
                            present_browser_tab_when_ready(&owner_appid, tab_id.clone());
                        } else {
                            sync_shell_layout(&owner_appid);
                        }
                        Ok(Some(lingxia_platform::traits::app_runtime::OpenUrlResult {
                            tab_id: Some(tab_id),
                        }))
                    }
                    Err(err) => Err(PlatformError::Platform(format!(
                        "failed to open browser tab for {url}: {err}"
                    ))),
                };
            }
            // May be called on a webview UI thread (NewWindowRequested);
            // hop onto the executor before touching tab/window state.
            std::mem::drop(lingxia::task::spawn(async move {
                open_browser_tab_for_open_url(&owner_appid, &url, present, aside);
            }));
            Ok(Some(lingxia_platform::traits::app_runtime::OpenUrlResult {
                tab_id: None,
            }))
        }
    }
}

fn handle_close_browser_tab(tab_id: &str) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    #[cfg(feature = "browser-runtime")]
    {
        if lingxia_browser::tab_is_aside(tab_id) {
            handle_compact_browser_tab_close(&owner_appid, tab_id);
        } else {
            handle_browser_tab_close(&owner_appid, tab_id);
        }
        true
    }
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (owner_appid, tab_id);
        false
    }
}

fn handle_activate_browser_tab(tab_id: String) -> ManagedSurfaceFuture {
    Box::pin(async move {
        let owner_appid = shell_owner_appid()
            .ok_or_else(|| PlatformError::NotSupported("browser tab".to_string()))?;
        #[cfg(feature = "browser-runtime")]
        {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let completion = Box::new(move |result| {
                let _ = sender.send(result);
            });
            present_browser_tab_when_ready_inner(
                &owner_appid,
                tab_id,
                false,
                None,
                Some(completion),
            );
            receiver.await.map_err(|_| {
                PlatformError::Platform("browser presentation was cancelled".to_string())
            })?
        }
        #[cfg(not(feature = "browser-runtime"))]
        {
            let _ = (owner_appid, tab_id);
            Err(PlatformError::NotSupported("browser tab".to_string()))
        }
    })
}

/// Opens `url` as a new in-app browser tab owned by the shell app and, when
/// `present` is set, shows it over the main content card (same flow as the
/// sidebar rows). The tabs-changed observer keeps the sidebar in sync.
#[cfg(feature = "browser-runtime")]
fn open_browser_tab_for_open_url(owner_appid: &str, url: &str, present: bool, aside: bool) {
    let Some(app) = lxapp::try_get(owner_appid) else {
        log::warn!("no shell owner app for in-app open-url of {url}");
        return;
    };
    let opened = if aside {
        lingxia_browser::open_aside_for_app(owner_appid, app.session_id(), url, None)
    } else {
        lingxia_browser::open_for_app(owner_appid, app.session_id(), url, None)
    };
    match opened {
        Ok(tab_id) if present => present_browser_tab_when_ready(owner_appid, tab_id),
        Ok(_) => sync_shell_layout(owner_appid),
        Err(err) => log::error!("failed to open browser tab for {url}: {err}"),
    }
}

#[cfg(feature = "browser-runtime")]
fn schedule_browser_tabs_changed_sync() {
    if let Some(owner) = presented_browser_window_handle()
        .or_else(crate::window_host::primary_host_window_handle)
        .or_else(|| shell_owner_appid().and_then(|appid| owner_window_handle(&appid)))
    {
        // Snapshot-dismiss an already open switcher at mutation time. Doing
        // this in the debounced observer can close a newer switcher that the
        // user opened after the mutation but before the observer ran.
        crate::window_host::dismiss_phone_tab_switcher(owner);
    }
    let epoch = BROWSER_TAB_SYNC_EPOCH.fetch_add(1, Ordering::Relaxed) + 1;
    if !has_browser_main_tabs() {
        // The generic automation close route bypasses the shell command
        // handler. Do not let unrelated metadata notifications keep pushing
        // the last-main restoration behind the normal debounce window.
        std::mem::drop(lingxia::task::spawn(async move {
            if BROWSER_TAB_SYNC_EPOCH.load(Ordering::Relaxed) == epoch {
                on_browser_tabs_changed();
            }
        }));
        return;
    }
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
    let recent_tab = lingxia_browser::current_tab().map(|tab| tab.tab_id);
    enforce_browser_tab_memory_limit(recent_tab.as_deref());
    reconcile_declared_browser_surfaces();
    if !SELF_BROWSER_HOST.load(Ordering::Acquire) && !has_browser_main_tabs() {
        if clear_browser_presentation() {
            restore_lxapp_main_after_browser(shell_owner_appid().as_deref());
        }
    } else if let Some(presented) = presented_browser_tab()
        && browser_tab_summary(&presented).is_none()
    {
        // Devtools can close a tab while an activate/present task is still
        // waiting for its WebView's first frame. Invalidate that task before
        // restoring the lxapp; otherwise it can recommit the already-closed
        // controller over the restored root after this observer returns.
        clear_browser_presentation();
        if SELF_BROWSER_HOST.load(Ordering::Acquire) {
            if let Some(successor) = lingxia_browser::current_tab()
                .map(|tab| tab.tab_id)
                .or_else(|| {
                    self_browser_root_tab().filter(|root| browser_tab_summary(root).is_some())
                })
            {
                // Devtools can close the visible tab without going through the
                // shell command handler. Rebind the surviving root immediately
                // so its row and WebView replace the destroyed controller.
                set_presented_browser_tab(Some(successor.clone()));
                present_browser_tab_when_ready(lingxia_browser::BUILTIN_BROWSER_APPID, successor);
            }
        } else {
            restore_lxapp_main_after_browser(shell_owner_appid().as_deref());
        }
    }
    refresh_aside_panel_nav_state();
    if consume_suppressed_browser_tab_sync() {
        return;
    }
    if let Some(appid) = shell_owner_appid() {
        sync_shell_layout(&appid);
    }
}

#[cfg(feature = "browser-runtime")]
fn has_browser_main_tabs() -> bool {
    browser_tabs().iter().any(|tab| {
        !lingxia_browser::tab_is_aside(&tab.tab_id)
            && !lingxia_browser::tab_is_standalone(&tab.tab_id)
    })
}

#[cfg(feature = "browser-runtime")]
fn reconcile_declared_browser_surfaces() {
    let entries = DECLARED_BROWSER_TABS
        .get()
        .and_then(|tabs| tabs.lock().ok())
        .map(|tabs| {
            tabs.iter()
                .map(|(surface_id, tab_id)| (surface_id.clone(), tab_id.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if entries.is_empty() {
        return;
    }
    let Some(owner) = shell_owner_appid().and_then(|appid| lxapp::try_get(&appid)) else {
        return;
    };
    for (surface_id, tab_id) in entries {
        if let Some(tab) = browser_tab_summary(&tab_id) {
            let _ = owner.update_shell_surface_automatic_title(
                &surface_id,
                Some(&browser_tab_display_title(&tab)),
            );
            continue;
        }
        if let Some(tabs) = DECLARED_BROWSER_TABS.get()
            && let Ok(mut tabs) = tabs.lock()
        {
            tabs.remove(&surface_id);
        }
        let outcome = owner.close_main_surface_deferred(&surface_id, "user");
        match outcome {
            lingxia_surface::CloseOutcome::RejectedRoot { .. } => {
                if let Err(error) = present_main_surface(&owner, &surface_id) {
                    log::warn!(
                        "failed to restore closed Windows browser root {surface_id}: {error}"
                    );
                }
            }
            lingxia_surface::CloseOutcome::Closed { .. } => {
                if let Some(active) = owner.surface_switcher_snapshot().active_surface_id
                    && let Err(error) = present_successor_main(&owner, &active)
                {
                    log::warn!("failed to present successor after browser close {active}: {error}");
                }
            }
            lingxia_surface::CloseOutcome::NotFound => {}
        }
    }
}

fn sync_app_shell_layout(appid: &str) {
    #[cfg(feature = "browser-runtime")]
    if SELF_BROWSER_HOST.load(Ordering::Acquire) && appid == lingxia_browser::BUILTIN_BROWSER_APPID
    {
        sync_self_browser_layout();
        return;
    }
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };
    if let Some(path) = app.peek_current_page().filter(|path| !path.is_empty())
        && let Some(webtag) = app.get_page(&path).map(|page| page.webtag())
    {
        let is_active_content = active_host_window_webtag_key().as_deref() == Some(webtag.key());
        let layout = build_window_layout(&app, &path);
        #[cfg(feature = "browser-runtime")]
        let dismiss_compact_switcher = is_active_content && !layout.compact_browser_chrome;
        install_shell_chrome_event_handler(&webtag, &app.appid);

        // Drive the device frame's status bar to match the active page: a visible
        // navigation bar extends its color up over the status bar (with its text
        // color); a plain page keeps the chrome-colored strip with contrasting text.
        if is_active_content && let Some(window) = owner_window_handle(appid) {
            let navbar = app.get_navbar_state(&path);
            let immersive = navbar.is_custom_navigation();
            let (foreground, background) = if immersive {
                let foreground = app
                    .resolved_navigation_bar_style(&path)
                    .foreground_color
                    .rgba()
                    >> 8;
                (foreground, 0)
            } else {
                match layout.navigation_bar.as_ref().filter(|nav| nav.visible) {
                    Some(nav) => (nav.text_color, nav.background_color),
                    None => {
                        let chrome = super::style::shell_palette().window_background;
                        (contrasting_text_color(chrome), chrome)
                    }
                }
            };
            set_device_frame_status_bar_style(window, foreground, background, immersive);
        }

        if let Err(err) = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout)) {
            log::warn!(
                "failed to sync Windows shell layout for {}:{}: {}",
                appid,
                path,
                err
            );
        }
        #[cfg(feature = "browser-runtime")]
        if dismiss_compact_switcher && let Some(owner) = owner_window_handle(appid) {
            crate::window_host::dismiss_phone_tab_switcher_if_not_compact(owner);
        }
    }

    #[cfg(feature = "terminal-runtime")]
    if let Some(native_webtag) = PRESENTED_NATIVE_MAIN
        .get()
        .and_then(|presented| presented.lock().ok())
        .and_then(|presented| presented.clone())
    {
        install_shell_chrome_event_handler(&native_webtag, &app.appid);
        if let Err(err) = set_webview_window_layout(
            &native_webtag,
            WindowsWindowLayout::new(content_agnostic_window_layout(&app)),
        ) {
            log::warn!("failed to sync Windows native main shell layout: {err}");
        }
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
        let layout = match app.peek_current_page() {
            Some(path) if !path.is_empty() => build_window_layout(&app, &path),
            _ => content_agnostic_window_layout(&app),
        };
        let dismiss_compact_switcher = !layout.compact_browser_chrome;
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
        if dismiss_compact_switcher
            && let Ok(snapshot) = lingxia_windows_contract::webview_window_snapshot(&browser_webtag)
        {
            crate::window_host::dismiss_phone_tab_switcher_if_not_compact(
                snapshot.window_id as isize,
            );
        }
    }
}

/// Installs an lxapp page's final shell geometry before its WebView is first
/// revealed. Startup presentation otherwise falls back to full-client bounds
/// until the later surface/UI sync catches up.
pub(crate) fn prime_lxapp_shell_layout(webtag: &WebTag, logical_width: f64) -> bool {
    let (appid, path) = webtag.extract_parts();
    if appid.is_empty() || path.is_empty() {
        return false;
    }
    let Some(app) = lxapp::try_get(&appid) else {
        return false;
    };
    // The initial host stays hidden until the home page is ready, so its
    // WM_SIZE path deliberately does not publish a potentially provisional
    // width. Presentation has selected and sized the real host by this point;
    // seed that width now so the first layout does not inherit the graph's
    // Medium fallback and paint an icon rail before expanding one frame later.
    update_surface_width(logical_width);
    install_shell_chrome_event_handler(webtag, &app.appid);
    match set_webview_window_layout(
        webtag,
        WindowsWindowLayout::new(build_window_layout(&app, &path)),
    ) {
        Ok(()) => {
            log::debug!(
                "primed Windows lxapp shell layout before presentation appid={} path={} webtag={}",
                appid,
                path,
                webtag.key()
            );
            true
        }
        Err(error) => {
            log::warn!(
                "failed to prime Windows lxapp shell layout for {}:{}: {}",
                appid,
                path,
                error
            );
            false
        }
    }
}

#[cfg(feature = "terminal-runtime")]
pub(super) fn on_terminal_panel_active_title_changed(surface_id: &str, title: &str) {
    let Some(owner) = shell_owner_appid().and_then(|appid| lxapp::try_get(&appid)) else {
        return;
    };
    if owner.update_shell_surface_automatic_title(surface_id, Some(title)) {
        sync_shell_layout(&owner.appid);
    }
}

#[cfg(feature = "browser-runtime")]
fn sync_self_browser_layout() {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    let webtag = WebTag::new(
        lingxia_browser::BUILTIN_BROWSER_APPID,
        &tab.path,
        Some(tab.session_id),
    );
    install_shell_chrome_event_handler(&webtag, lingxia_browser::BUILTIN_BROWSER_APPID);
    let layout = build_self_browser_window_layout(&webtag);
    let dismiss_compact_switcher = layout.tab_bar.is_some();
    if let Err(error) = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout)) {
        log::warn!("failed to sync Windows self-browser layout: {error}");
    }
    if dismiss_compact_switcher
        && let Ok(snapshot) = lingxia_windows_contract::webview_window_snapshot(&webtag)
    {
        crate::window_host::dismiss_phone_tab_switcher_if_not_compact(snapshot.window_id as isize);
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
    let owner_app = shell_owner_app_for(app);
    let shell_app = owner_app.as_deref().unwrap_or(app);
    let footer_actions = build_footer_actions(shell_app);
    // A simulator frame whose toolbar carries the close/minimize dots owns the
    // window controls, so the shell drops its own caption there. A framed
    // simulated desktop keeps the standard Windows caption buttons.
    let owner_window = owner_window_handle(&shell_app.appid);
    let suppress_window_controls = owner_window
        .map(device_frame_owns_window_controls)
        .unwrap_or(false);
    // Reserve the device frame's status-bar strip so the nav bar + content stack
    // below it (the status bar overlay owns the top strip), matching the macOS
    // runner's status-bar + nav-bar layout. An immersive (custom navigation-
    // style) page draws its own header and bleeds content up under the status
    // bar, so it reserves no top inset — the transparent status-bar overlay just
    // floats the clock/indicators over the page.
    // A presented browser tab is never immersive: its address row must sit
    // below the status-bar strip, not under the floating clock/cutout.
    let immersive = address_bar.is_none() && app.get_navbar_state(path).is_custom_navigation();
    let top_inset = if immersive {
        0
    } else {
        owner_window
            .map(device_frame_status_bar_height)
            .unwrap_or(0)
    };
    // A presented browser tab covers the phone tab bar, matching the macOS
    // runner's full-screen browser surface; side tab bars (sidebar) stay.
    let active_main_app = preferred_sidebar_group_appid(
        shell_owner_appid(),
        presented_browser_group_appid(),
        active_main_lxapp_id(),
    )
    .and_then(|appid| lxapp::try_get(&appid));
    let tab_bar_app =
        tab_bar_owner_for_layout(app, owner_app.as_deref(), active_main_app.as_deref());
    let tab_bar = build_tab_bar_layout(tab_bar_app, &footer_actions).filter(|tabbar| {
        address_bar.is_none() || !matches!(tabbar.position, WindowsShellTabBarPosition::Bottom)
    });
    let compact_browser_chrome = address_bar.is_some()
        && suppress_window_controls
        && resolved_shell_size_class(Some(shell_app)) == SizeClass::Compact;
    WindowsShellWindowLayout {
        navigation_bar,
        address_bar,
        tab_bar,
        footer_actions,
        compact_browser_chrome,
        top_inset,
        suppress_window_controls,
    }
}

/// Shell chrome for a browser/native main owned by a control lxapp without a
/// mounted page. Keep host actions and the desktop switcher, but omit lxapp
/// navigation and page tabs.
fn content_agnostic_window_layout(app: &LxApp) -> WindowsShellWindowLayout {
    let mut layout = build_window_layout(app, &app.initial_route());
    layout.navigation_bar = None;
    if let Some(tab_bar) = layout.tab_bar.as_mut() {
        tab_bar.items.clear();
        tab_bar.selected_index = -1;
    }
    layout
}

#[cfg(feature = "browser-runtime")]
fn build_self_browser_window_layout(webtag: &WebTag) -> WindowsShellWindowLayout {
    let window = lingxia_windows_contract::webview_window_snapshot(webtag)
        .ok()
        .map(|snapshot| snapshot.window_id as isize);
    let mut address_bar = build_address_bar_layout();
    if let Some(address_bar) = address_bar.as_mut() {
        address_bar.dismissible = false;
        address_bar.show_bookmark = false;
        address_bar.show_pin = false;
        address_bar.show_page_menu = false;
    }
    let suppress_window_controls = window
        .map(device_frame_owns_window_controls)
        .unwrap_or(false);
    WindowsShellWindowLayout {
        address_bar,
        tab_bar: build_self_browser_tab_bar_layout(),
        compact_browser_chrome: suppress_window_controls
            && resolved_shell_size_class(None) == SizeClass::Compact,
        suppress_window_controls,
        top_inset: window.map(device_frame_status_bar_height).unwrap_or(0),
        ..Default::default()
    }
}

#[cfg(feature = "browser-runtime")]
fn build_self_browser_tab_bar_layout() -> Option<WindowsShellTabBarLayout> {
    if presented_browser_tab()
        .as_deref()
        .is_some_and(lingxia_browser::tab_is_aside)
    {
        return None;
    }
    let position = tabbar_position(lingxia_browser::BUILTIN_BROWSER_APPID);
    if position == WindowsShellTabBarPosition::Bottom {
        return None;
    }
    let ui_state = sidebar_ui_state(lingxia_browser::BUILTIN_BROWSER_APPID);
    let tabs = browser_tabs()
        .into_iter()
        .filter(|tab| !lingxia_browser::tab_is_aside(&tab.tab_id))
        .collect::<Vec<_>>();
    let root_id = self_browser_root_tab();
    let root = root_id
        .as_deref()
        .and_then(|root_id| tabs.iter().find(|tab| tab.tab_id == root_id));
    Some(WindowsShellTabBarLayout {
        visible: true,
        position,
        dimension: MIN_SIDEBAR_WIDTH,
        app_name: root
            .map(browser_tab_display_title)
            .unwrap_or_else(|| "New Tab".to_string()),
        app_icon_path: String::new(),
        group_id: lingxia_browser::BUILTIN_BROWSER_APPID.to_string(),
        group_target_id: format!(
            "{AUX_LXAPP_PREFIX}{}",
            lingxia_browser::BUILTIN_BROWSER_APPID
        ),
        group_active: true,
        group_closable: false,
        group_order_index: 0,
        color: 0x666666,
        selected_color: 0x1677ff,
        background_color: 0xffffff,
        background_transparent: true,
        border_color: 0xf0f0f0,
        selected_index: -1,
        items: Vec::new(),
        overflow_start_index: -1,
        collapsed: ui_state.collapsed,
        icon_rail: ui_state.icon_rail,
        items_api_hidden: false,
        items_collapsed: false,
        footer_action_height: 0,
        main_scroll_offset: ui_state.main_scroll_offset,
        footer_action_scroll_row: 0,
        auxiliary_items: build_browser_tab_items(
            tabs.into_iter()
                .filter(|tab| root_id.as_deref() != Some(tab.tab_id.as_str()))
                .collect(),
        ),
        show_auxiliary_add: true,
        header_actions: Vec::new(),
    })
}

#[cfg(feature = "browser-runtime")]
fn self_browser_root_tab() -> Option<String> {
    SELF_BROWSER_ROOT_TAB
        .get()
        .and_then(|root| root.lock().ok())
        .and_then(|root| root.clone())
}

fn shell_owner_app_for(active: &LxApp) -> Option<Arc<LxApp>> {
    let owner_appid = shell_owner_appid()?;
    if owner_appid == active.appid {
        return None;
    }
    lxapp::try_get(&owner_appid)
}

fn tab_bar_owner_for_layout<'a>(
    active: &'a LxApp,
    owner: Option<&'a LxApp>,
    active_main: Option<&'a LxApp>,
) -> &'a LxApp {
    if let Some(owner) = owner
        && matches!(
            tabbar_position(&owner.appid),
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        )
    {
        return owner;
    }
    if matches!(
        tabbar_position(&active.appid),
        WindowsShellTabBarPosition::Bottom
    ) {
        active
    } else {
        owner.or(active_main).unwrap_or(active)
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
        .items
        .get(selected_index as usize)
        .map(|item| item.page_path.clone());
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
        // Page webtags are per-instance; resolve the live instance instead of
        // reconstructing a tag from the path.
        let Some(webtag) = app.get_page(&path).map(|page| page.webtag()) else {
            continue;
        };
        set_webview_chrome_event_handler(&webtag, handler.clone());
        let _ = set_webview_window_layout(&webtag, WindowsWindowLayout::new(layout));
    }
}

/// Address-bar layout for the presented browser tab, or `None` while the
/// main surface shows an lxapp webview.
fn build_address_bar_layout() -> Option<WindowsShellAddressBarLayout> {
    let presented = presented_browser_tab()?;
    let tab = browser_tab_summary(&presented)?;
    #[cfg(feature = "browser-runtime")]
    let aside = lingxia_browser::tab_is_aside(&presented);
    #[cfg(not(feature = "browser-runtime"))]
    let aside = false;
    #[cfg(feature = "browser-shell")]
    let bookmarked = tab
        .current_url
        .as_deref()
        .is_some_and(lingxia_browser_shell::is_bookmarked);
    #[cfg(not(feature = "browser-shell"))]
    let bookmarked = false;
    #[cfg(feature = "browser-shell")]
    let pinned = tab
        .current_url
        .as_deref()
        .and_then(pinned_bookmark_for_url)
        .is_some();
    #[cfg(not(feature = "browser-shell"))]
    let pinned = false;
    let web = tab
        .current_url
        .as_deref()
        .is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"));
    #[cfg(feature = "browser-runtime")]
    let tab_count = browser_tabs()
        .into_iter()
        .filter(|tab| lingxia_browser::tab_is_aside(&tab.tab_id) == aside)
        .count();
    #[cfg(not(feature = "browser-runtime"))]
    let tab_count = browser_tabs().len();
    Some(WindowsShellAddressBarLayout {
        visible: true,
        dismissible: true,
        url_text: browser_tab_display_url(&tab),
        aside,
        can_go_back: tab.can_go_back,
        can_go_forward: tab.can_go_forward,
        bookmarked,
        pinned,
        show_bookmark: cfg!(feature = "browser-shell"),
        show_pin: cfg!(feature = "browser-shell"),
        show_page_menu: cfg!(feature = "browser-shell"),
        web,
        tab_count,
    })
}

/// Session-history availability of the aside panel's visible tab, refreshed
/// on every browser tabs-changed pass; the aside toolbar dims back/forward
/// from this.
static ASIDE_PANEL_NAV_STATE: OnceLock<Mutex<(bool, bool)>> = OnceLock::new();

pub(super) fn aside_panel_nav_state() -> (bool, bool) {
    // Enabled until reported otherwise: hosts without the browser engine
    // (plain WebView2 asides) never report, so their buttons stay active.
    ASIDE_PANEL_NAV_STATE
        .get()
        .and_then(|slot| slot.lock().ok())
        .map(|state| *state)
        .unwrap_or((true, true))
}

/// Mirrors the visible aside tab's nav state and repaints the aside toolbar
/// when it changed.
#[cfg(feature = "browser-runtime")]
fn refresh_aside_panel_nav_state() {
    let state = lingxia_browser::tabs()
        .into_iter()
        .filter(|tab| lingxia_browser::tab_is_aside(&tab.tab_id))
        .find(|tab| {
            let webtag = WebTag::new(
                lingxia_browser::BUILTIN_BROWSER_APPID,
                &tab.path,
                Some(tab.session_id),
            );
            crate::window_host::webtag_is_visible(webtag.key())
        })
        .map(|tab| (tab.can_go_back, tab.can_go_forward))
        .unwrap_or((false, false));
    let slot = ASIDE_PANEL_NAV_STATE.get_or_init(|| Mutex::new((false, false)));
    let changed = slot.lock().map(|mut current| {
        let changed = *current != state;
        *current = state;
        changed
    });
    if changed.unwrap_or(false) {
        lingxia_windows_contract::refresh_aside_panel(
            lingxia_windows_contract::ASIDE_BROWSER_PANEL_ID,
        );
    }
}

/// Capsule text of the presented tab: its current URL, else its title
/// (matching the sidebar row fallback). A blank new tab reads as empty,
/// like a fresh address input.
fn browser_tab_display_url(tab: &BrowserTabSummary) -> String {
    let url = tab
        .current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| browser_tab_display_title(tab));
    if browser_url_is_hidden(&url) {
        return String::new();
    }
    url
}

fn browser_url_is_hidden(url: &str) -> bool {
    matches!(
        url.trim().to_ascii_lowercase().as_str(),
        "about:blank" | "lingxia://newtab" | "lingxia://"
    )
}

fn build_navigation_bar_layout(app: &LxApp, path: &str) -> WindowsShellNavigationBarLayout {
    let navbar = app.get_navbar_state(path);
    let style = app.resolved_navigation_bar_style(path);
    let background_color = style.background_color.rgba() >> 8;
    let text_color = style.foreground_color.rgba() >> 8;
    WindowsShellNavigationBarLayout {
        visible: navbar.show_navbar,
        title: navbar.title().to_string(),
        background_color,
        text_color,
        show_back_button: navbar.show_back_button,
        show_home_button: navbar.home_button_visible(),
        height: DEFAULT_NAV_BAR_HEIGHT,
    }
}

/// Dark or light text for legibility over `background` (0xRRGGBB).
fn contrasting_text_color(background: u32) -> u32 {
    let luminance = (((background >> 16) & 0xff) * 299
        + ((background >> 8) & 0xff) * 587
        + (background & 0xff) * 114)
        / 1000;
    if luminance > 140 { 0x111111 } else { 0xf2f2f7 }
}

/// Strips a leading `/` and a framework file extension so a page route
/// (`pages/home/index.tsx`) compares equal to a tab item's resolved path
/// (`pages/home/index`). The manifest names the page; this is the catalog path.
fn normalize_tab_path(path: &str) -> &str {
    let path = path.strip_prefix('/').unwrap_or(path);
    for ext in [".tsx", ".ts", ".jsx", ".js", ".vue", ".html"] {
        if let Some(stripped) = path.strip_suffix(ext) {
            return stripped;
        }
    }
    path
}

fn build_tab_bar_layout(
    app: &LxApp,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> Option<WindowsShellTabBarLayout> {
    if lxapp::open_region(&app.appid) == Some(lxapp::LxAppOpenRegion::Aside) {
        return None;
    }
    let tabbar = app.get_tabbar();
    // The tab matching the page currently shown, if any. Standard mini-app
    // behavior derives the highlighted tab from the current page (not a stored
    // index) and shows the bottom bar only on tab pages — a navigated-to
    // sub-page is not a tab page.
    let current_path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    let current_tab_index = tabbar.as_ref().and_then(|tabbar| {
        let target = normalize_tab_path(&current_path);
        tabbar
            .items
            .iter()
            .position(|item| normalize_tab_path(&item.page_path) == target)
    });
    let ui_state = sidebar_ui_state(&app.appid);
    let runtime_info = app.runtime_info();
    let switcher_owner = shell_owner_appid().and_then(|appid| lxapp::try_get(&appid));
    let switcher_app = switcher_owner.as_deref().unwrap_or(app);
    let size_class = resolved_shell_size_class(Some(switcher_app));
    let switcher = switcher_app.surface_switcher_snapshot();
    let root = switcher.root_surface_id.as_deref().and_then(|root_id| {
        switcher
            .items
            .iter()
            .find(|item| item.surface_id == root_id)
    });
    let group_active = root.map(|item| item.active).unwrap_or_else(|| {
        presented_browser_tab().is_none() && active_main_lxapp_id().as_deref() == Some(&app.appid)
    });
    let root_owns_lxapp_navigation = root.is_none_or(|item| {
        matches!(
            &item.content,
            SwitcherContentKind::Lxapp { app_id } | SwitcherContentKind::Page { app_id }
                if app_id == &app.appid
        )
    });
    let items = tabbar
        .as_ref()
        .filter(|_| root_owns_lxapp_navigation)
        .map(|tabbar| {
            tabbar
                .visible_items()
                .map(|(index, item)| WindowsShellTabBarItemLayout {
                    index,
                    page_path: item.page_path.clone(),
                    text: item.text.clone().unwrap_or_default(),
                    icon_path: item.icon_path.clone().unwrap_or_default(),
                    badge: item.badge.clone(),
                    has_red_dot: item.has_red_dot,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut browser_tabs = browser_tabs();
    let declared_browser_tabs = declared_browser_tab_ids();
    browser_tabs.retain(|tab| !declared_browser_tabs.contains(&tab.tab_id));
    let mut auxiliary_items = build_pinned_items(&browser_tabs);
    let declared_lxapps = switcher
        .items
        .iter()
        .filter_map(|item| match &item.content {
            SwitcherContentKind::Lxapp { app_id } | SwitcherContentKind::Page { app_id } => {
                Some(app_id.as_str())
            }
            SwitcherContentKind::Browser | SwitcherContentKind::Native { .. } => None,
        })
        .collect::<HashSet<_>>();
    let mut main_rows = build_surface_switcher_items(switcher_app, &switcher);
    main_rows.extend(
        build_open_lxapp_items(&app.appid)
            .into_iter()
            .filter(|row| {
                auxiliary_lxapp_id(&row.id).is_none_or(|appid| !declared_lxapps.contains(appid))
            }),
    );
    main_rows.extend(build_browser_tab_items(browser_tabs));
    let group_target_id = root
        .map(|item| format!("{AUX_SURFACE_PREFIX}{}", item.surface_id))
        .unwrap_or_else(|| format!("{AUX_LXAPP_PREFIX}{}", app.appid));
    let (group_order_index, main_rows) = order_main_tab_rows(&group_target_id, main_rows);
    auxiliary_items.extend(main_rows);
    // The global "+" follows the active main provider: browser opens a tab,
    // while a terminal root opens another main workspace. A device-framed
    // runner hosts a single app and never exposes desktop workspace creation.
    let owner_window = owner_window_handle(&app.appid);
    let device_framed = owner_window.map(window_has_device_frame).unwrap_or(false);
    let frame_status_bar_height = owner_window
        .map(device_frame_status_bar_height)
        .unwrap_or(0);
    let mut show_auxiliary_add = main_workspace_add_target(&switcher).is_some() && !device_framed;
    let mut header_actions = build_sidebar_header_actions(app);
    let sidebar_has_content = sidebar_content_available(
        !items.is_empty(),
        !auxiliary_items.is_empty(),
        !footer_actions.is_empty(),
        !header_actions.is_empty(),
        show_auxiliary_add,
    );
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
    let requested_position = tabbar_position(&app.appid);
    let (position, force_icon_rail, show_shell_entries) = adaptive_tabbar_projection(
        requested_position,
        size_class,
        device_framed && frame_status_bar_height > 0,
        ui_state.medium_expanded,
    );
    if !show_shell_entries {
        // Compact projects only the active lxapp's own tabbar at the bottom.
        // Pins, main-switcher rows and app-owned shell actions have no compact
        // projection and must not leak into that bar.
        auxiliary_items.clear();
        show_auxiliary_add = false;
        header_actions.clear();
    }
    // The bottom bar persists across pages, driven by `is_visible` like the
    // sidebar. Only an item-less bar is dropped on a sub-page, so a stray
    // auxiliary row (open lxapps / browser tabs) can't pop a bottom bar onto
    // one.
    if position == WindowsShellTabBarPosition::Bottom
        && current_tab_index.is_none()
        && items.is_empty()
    {
        return None;
    }
    // `dimension` is the bar's cross-axis size: a sidebar's width, but a bottom
    // bar's *height*. A bottom bar is a compact icon+label strip, so it must not
    // borrow the (much taller) sidebar minimum width. The content area is already
    // inset by the home-indicator safe area, so the strip sits just above it and
    // must not re-reserve that height (which would float it up by that much).
    let dimension = match position {
        WindowsShellTabBarPosition::Bottom => BOTTOM_TABBAR_CONTENT_HEIGHT,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right => {
            persisted_expanded_sidebar_width()
        }
    };
    let desktop_sidebar = matches!(
        position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    );
    // An app without a tabBar declaration inherits the desktop shell surface;
    // an explicit backgroundColor still styles the full sidebar as requested.
    let resolved_style = app.resolved_tabbar_style();
    let tabbar_background_transparent = tabbar.as_ref().is_some_and(|tabbar| {
        tabbar.presentation == lxapp::page_chrome::TabBarPresentation::Immersive
    });
    let items_api_hidden = desktop_sidebar
        && tabbar.as_ref().is_some_and(|tabbar| {
            tabbar.visibility == lxapp::page_chrome::TabBarVisibilityPreference::Hidden
        });
    Some(WindowsShellTabBarLayout {
        // Mobile navigation may flip `is_visible` on detail pages. Desktop
        // sidebar chrome is stable; only an explicit API hide affects its
        // child rows, never the sidebar or the parent lxapp tab itself.
        visible: if desktop_sidebar {
            true
        } else {
            tabbar
                .as_ref()
                .map(|tabbar| tabbar.is_effectively_visible())
                .unwrap_or(true)
        },
        position,
        dimension,
        app_name: root
            .map(surface_switcher_title)
            .unwrap_or(runtime_info.app_name),
        app_icon_path: root
            .map(|item| surface_switcher_icon_path(switcher_app, item))
            .unwrap_or_else(|| app.get_lxapp_info().icon),
        group_id: app.appid.clone(),
        group_target_id,
        group_active,
        group_closable: root
            .map(|item| item.closable)
            .unwrap_or_else(|| main_lxapp_closable(&app.appid)),
        group_order_index,
        collapsed: ui_state.collapsed,
        icon_rail: ui_state.icon_rail || force_icon_rail,
        items_api_hidden,
        items_collapsed: items_api_hidden || ui_state.items_collapsed,
        footer_action_height: if desktop_sidebar {
            super::chrome::panel_footer_action_height(dimension, footer_actions)
        } else {
            0
        },
        main_scroll_offset: ui_state.main_scroll_offset,
        footer_action_scroll_row: ui_state.footer_action_scroll_row,
        color: resolved_style
            .map(|style| style.foreground_color.rgba() >> 8)
            .unwrap_or(0x666666),
        selected_color: resolved_style
            .map(|style| style.selected_foreground_color.rgba() >> 8)
            .unwrap_or(0x1677ff),
        // Transparent bottom bars keep the WebView laid out underneath; a
        // small overlay window draws only the tab items above that content.
        background_color: resolved_style
            .and_then(|style| style.background_color)
            .map(|color| color.rgba() >> 8)
            .unwrap_or(0),
        background_transparent: tabbar_background_transparent,
        border_color: resolved_style
            .and_then(|style| style.divider_color)
            .map(|color| color.rgba() >> 8)
            .unwrap_or(0),
        // A detail page keeps the lxapp group selected but clears every child
        // selection; group and tabbar-item selection are independent levels.
        selected_index: current_tab_index.map(|index| index as i32).unwrap_or(-1),
        overflow_start_index: tabbar
            .as_ref()
            .map(|tabbar| tabbar.compact_overflow_slot_index())
            .unwrap_or(-1),
        items,
        auxiliary_items,
        show_auxiliary_add,
        header_actions,
    })
}

fn sidebar_content_available(
    has_page_items: bool,
    has_workspace_items: bool,
    has_footer_actions: bool,
    has_header_actions: bool,
    can_add_workspace: bool,
) -> bool {
    has_page_items
        || has_workspace_items
        || has_footer_actions
        || has_header_actions
        || can_add_workspace
}

fn adaptive_tabbar_projection(
    requested: WindowsShellTabBarPosition,
    size_class: SizeClass,
    device_framed: bool,
    medium_expanded: bool,
) -> (WindowsShellTabBarPosition, bool, bool) {
    match size_class {
        SizeClass::Compact
            if !device_framed
                && matches!(
                    requested,
                    WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
                ) =>
        {
            (requested, true, true)
        }
        SizeClass::Compact => (WindowsShellTabBarPosition::Bottom, false, false),
        SizeClass::Medium
            if !device_framed
                && !medium_expanded
                && matches!(
                    requested,
                    WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
                ) =>
        {
            (requested, true, true)
        }
        SizeClass::Medium | SizeClass::Expanded => (requested, false, true),
    }
}

fn toggle_sidebar_projection(state: &mut SidebarUiState, size_class: SizeClass) {
    state.collapsed = false;
    if size_class == SizeClass::Medium {
        // Medium is already projected as a rail, so the toggle reveals the full
        // sidebar for this session rather than choosing a width to remember.
        let currently_expanded = state.medium_expanded && !state.icon_rail;
        state.medium_expanded = !currently_expanded;
        state.icon_rail = false;
    } else {
        state.medium_expanded = false;
        state.icon_rail = !state.icon_rail;
        // The user has now chosen, so nothing may seed over it later.
        state.seeded = true;
        persist_sidebar_chrome(state.icon_rail);
    }
}

fn active_main_lxapp_id() -> Option<String> {
    let graph_snapshot = shell_owner_appid()
        .and_then(|owner_appid| lxapp::try_get(&owner_appid))
        .map(|owner| owner.surface_switcher_snapshot());
    if let Some(snapshot) = graph_snapshot
        && let Some(active_id) = snapshot.active_surface_id
    {
        return snapshot
            .items
            .into_iter()
            .find(|item| item.surface_id == active_id)
            .and_then(|item| match item.content {
                SwitcherContentKind::Lxapp { app_id } | SwitcherContentKind::Page { app_id } => {
                    Some(app_id)
                }
                SwitcherContentKind::Browser | SwitcherContentKind::Native { .. } => None,
            })
            .filter(|appid| lxapp::open_region(appid) == Some(lxapp::LxAppOpenRegion::Main));
    }
    let appid = lxapp::get_current_lxapp().0;
    (!appid.is_empty() && lxapp::open_region(&appid) == Some(lxapp::LxAppOpenRegion::Main))
        .then_some(appid)
}

fn main_lxapp_closable(appid: &str) -> bool {
    shell_owner_appid()
        .and_then(|owner_appid| lxapp::try_get(&owner_appid))
        .map(|owner| owner.surface_switcher_snapshot())
        .and_then(|snapshot| {
            snapshot.items.into_iter().find(|item| {
                matches!(
                    &item.content,
                    SwitcherContentKind::Lxapp { app_id } if app_id == appid
                )
            })
        })
        .is_some_and(|item| item.closable)
}

fn build_pinned_items(tabs: &[BrowserTabSummary]) -> Vec<WindowsShellAuxiliaryItemLayout> {
    let active_main = active_main_lxapp_id();
    let active_asides: HashSet<String> = shell_owner_appid()
        .and_then(|owner_appid| lxapp::try_get(&owner_appid))
        .and_then(|owner| owner.surface_derived_layout())
        .map(|plan| {
            plan.aside_slots
                .into_iter()
                .filter(|slot| slot.visible)
                .filter_map(|slot| slot.active_child)
                .collect()
        })
        .unwrap_or_default();
    runtime_shell_pins()
        .into_iter()
        .filter_map(|pin| match pin.0 {
            ShellPinTarget::Lxapp { key: appid } => {
                let info = lxapp::try_get(&appid).map(|app| app.runtime_info());
                let title = info
                    .as_ref()
                    .map(|info| info.app_name.trim())
                    .filter(|title| !title.is_empty())
                    .unwrap_or(&appid)
                    .to_string();
                let surface_id = panel_item_for_lxapp(&appid)
                    .map(|(panel_id, _, _)| panel_id)
                    .unwrap_or_else(|| appid.clone());
                Some(WindowsShellAuxiliaryItemLayout {
                    // A Pin is a launch shortcut, not the workspace row that
                    // appears after launch. Keep their identities distinct so
                    // both remain independently paintable and actionable.
                    id: format!("{AUX_PINNED_LXAPP_PREFIX}{appid}"),
                    title,
                    active: active_asides.contains(&surface_id)
                        || (presented_browser_tab().is_none()
                            && active_main.as_deref() == Some(appid.as_str())),
                    pinned: true,
                    closable: false,
                    icon_png: None,
                    icon_path: lxapp_auxiliary_icon_path(&appid),
                })
            }
            ShellPinTarget::Bookmark { key } => build_pinned_bookmark_item(&key, tabs),
        })
        .collect()
}

fn build_surface_switcher_items(
    owner: &LxApp,
    snapshot: &lingxia_surface::SurfaceSwitcherSnapshot,
) -> Vec<WindowsShellAuxiliaryItemLayout> {
    snapshot
        .items
        .iter()
        .filter(|item| !item.root)
        .map(|item| WindowsShellAuxiliaryItemLayout {
            id: format!("{AUX_SURFACE_PREFIX}{}", item.surface_id),
            title: surface_switcher_title(item),
            active: item.active,
            pinned: false,
            closable: item.closable,
            icon_png: None,
            icon_path: surface_switcher_icon_path(owner, item),
        })
        .collect()
}

fn surface_switcher_title(item: &SurfaceSwitcherItem) -> String {
    if let SwitcherContentKind::Lxapp { app_id } | SwitcherContentKind::Page { app_id } =
        &item.content
        && item.title.as_deref().is_none_or(|title| title == app_id)
        && let Some(title) = lxapp::try_get(app_id)
            .map(|app| app.runtime_info().app_name)
            .filter(|title| !title.trim().is_empty())
    {
        return title;
    }
    item.title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&item.surface_id)
        .to_string()
}

fn surface_switcher_icon_path(owner: &LxApp, item: &SurfaceSwitcherItem) -> String {
    match item.icon.as_ref() {
        Some(SurfaceIcon::Resource { uri }) => {
            let path = Path::new(uri);
            if path.is_absolute() {
                uri.clone()
            } else {
                resolve_asset_path(owner.runtime.asset_dir(), uri)
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }
        }
        Some(SurfaceIcon::ProviderAsset { provider, key }) if provider == "lxapp" => {
            lxapp_auxiliary_icon_path(key)
        }
        Some(SurfaceIcon::BuiltIn { name }) if name == "terminal" => owner
            .runtime
            .asset_dir()
            .join("icons")
            .join("design")
            .join("icon_terminal.png")
            .to_string_lossy()
            .into_owned(),
        Some(SurfaceIcon::BuiltIn { name }) if name == "browser" => owner
            .runtime
            .asset_dir()
            .join("icons")
            .join("design")
            .join("icon_globe.png")
            .to_string_lossy()
            .into_owned(),
        Some(SurfaceIcon::BuiltIn { .. }) | Some(SurfaceIcon::ProviderAsset { .. }) | None => {
            String::new()
        }
    }
}

#[cfg(feature = "browser-runtime")]
fn declared_browser_tab_ids() -> HashSet<String> {
    DECLARED_BROWSER_TABS
        .get()
        .and_then(|tabs| tabs.lock().ok())
        .map(|tabs| tabs.values().cloned().collect())
        .unwrap_or_default()
}

#[cfg(not(feature = "browser-runtime"))]
fn declared_browser_tab_ids() -> HashSet<String> {
    HashSet::new()
}

fn order_main_tab_rows(
    group_id: &str,
    rows: Vec<WindowsShellAuxiliaryItemLayout>,
) -> (usize, Vec<WindowsShellAuxiliaryItemLayout>) {
    let group_id = group_id.to_string();
    let mut live = HashSet::with_capacity(rows.len() + 1);
    live.insert(group_id.clone());
    live.extend(rows.iter().map(|row| row.id.clone()));
    let order = MAIN_TAB_ORDER.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut order) = order.lock() else {
        return (0, rows);
    };
    order.retain(|id| live.contains(id));
    for id in std::iter::once(&group_id).chain(rows.iter().map(|row| &row.id)) {
        if !order.contains(id) {
            order.push(id.clone());
        }
    }
    let positions: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    let group_order_index = positions.get(group_id.as_str()).copied().unwrap_or(0);
    let mut rows = rows;
    rows.sort_by_key(|row| {
        positions
            .get(row.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    (group_order_index, rows)
}

fn build_sidebar_header_actions(app: &LxApp) -> Vec<WindowsShellHeaderActionLayout> {
    if owner_window_handle(&app.appid).is_some_and(window_has_device_frame)
        || tabbar_position(&app.appid) == WindowsShellTabBarPosition::Bottom
    {
        return Vec::new();
    }
    resolved_sidebar_actions_for_placement(app, SidebarActionPlacement::Header)
        .into_iter()
        .map(|item| WindowsShellHeaderActionLayout {
            generation: item.generation,
            id: item.id,
            label: item.label,
            icon_path: resolved_sidebar_action_icon_path(app, item.icon_path.as_deref()),
            disabled: item.disabled,
        })
        .collect()
}

// Browser tab rows stay visible independently of pinned shortcuts (a pinned
// site keeps both its grid tile and its row), matching the macOS sidebar.
fn build_browser_tab_items(tabs: Vec<BrowserTabSummary>) -> Vec<WindowsShellAuxiliaryItemLayout> {
    let presented = presented_browser_tab();
    tabs.into_iter()
        .map(|tab| {
            let active = presented.as_deref() == Some(tab.tab_id.as_str());
            let title = browser_tab_display_title(&tab);
            let icon_png = tab.favicon_png.clone();
            WindowsShellAuxiliaryItemLayout {
                id: tab.tab_id,
                title,
                active,
                pinned: false,
                closable: true,
                icon_png,
                icon_path: String::new(),
            }
        })
        .collect()
}

#[cfg(feature = "browser-shell")]
fn build_pinned_bookmark_item(
    bookmark_id: &str,
    tabs: &[BrowserTabSummary],
) -> Option<WindowsShellAuxiliaryItemLayout> {
    let active_url = presented_browser_tab()
        .and_then(|tab_id| browser_tab_summary(&tab_id))
        .and_then(|tab| tab.current_url)
        .map(|url| lingxia_browser_shell::normalize_bookmark_url(&url));
    let entry = lingxia_browser_shell::bookmarks_snapshot()?
        .entries
        .into_iter()
        .find(|entry| entry.id == bookmark_id)?;
    let normalized = lingxia_browser_shell::normalize_bookmark_url(&entry.url);
    let icon_png = tabs.iter().find_map(|tab| {
        tab.current_url
            .as_deref()
            .is_some_and(|url| lingxia_browser_shell::normalize_bookmark_url(url) == normalized)
            .then(|| tab.favicon_png.clone())
            .flatten()
    });
    let title = if entry.title.trim().is_empty() {
        entry.url.clone()
    } else {
        entry.title
    };
    Some(WindowsShellAuxiliaryItemLayout {
        icon_path: lingxia_browser_shell::bookmark_favicon_path(&entry.url).unwrap_or_default(),
        id: format!("{AUX_BOOKMARK_PREFIX}{}", entry.id),
        title,
        active: active_url.as_deref() == Some(normalized.as_str()),
        pinned: true,
        closable: false,
        icon_png,
    })
}

#[cfg(not(feature = "browser-shell"))]
fn build_pinned_bookmark_item(
    _bookmark_id: &str,
    _tabs: &[BrowserTabSummary],
) -> Option<WindowsShellAuxiliaryItemLayout> {
    None
}

fn build_open_lxapp_items(owner_appid: &str) -> Vec<WindowsShellAuxiliaryItemLayout> {
    let current_appid = active_main_lxapp_id();
    lxapp::list_lxapps()
        .into_iter()
        .filter(|info| info.appid != owner_appid)
        .filter(|info| !is_builtin_browser_appid(&info.appid))
        .filter(|info| matches!(info.status.as_str(), "opening" | "opened"))
        .filter(|info| lxapp::open_region(&info.appid) == Some(lxapp::LxAppOpenRegion::Main))
        // A capsule-closed lxapp keeps its "opened" session (stateful hide)
        // but leaves the navigation stack; the sidebar lists only apps the
        // user still has open.
        .filter(|info| info.in_stack)
        .map(|info| {
            let title = if info.app_name.trim().is_empty() {
                info.appid.clone()
            } else {
                info.app_name
            };
            let icon_path = lxapp_auxiliary_icon_path(&info.appid);
            WindowsShellAuxiliaryItemLayout {
                id: format!("{AUX_LXAPP_PREFIX}{}", info.appid),
                title,
                active: presented_browser_tab().is_none()
                    && current_appid.as_deref() == Some(info.appid.as_str()),
                pinned: false,
                closable: main_lxapp_closable(&info.appid),
                icon_png: None,
                icon_path,
            }
        })
        .collect()
}

/// Sidebar row icon for an open lxapp: the lxapp's own declared icon, else
/// the icon of its configured surface/panel slot (matching the panel
/// footer action), else empty so the row falls back to the LingXia mark.
fn lxapp_auxiliary_icon_path(appid: &str) -> String {
    let own_icon = lxapp::try_get(appid)
        .map(|app| app.get_lxapp_info().icon)
        .filter(|icon| !icon.trim().is_empty());
    if let Some(icon) = own_icon {
        return icon;
    }
    let panel_icon = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| {
            panels.items.into_iter().find_map(|item| {
                (item.content.kind.is_lxapp()
                    && item.content.app_id == appid
                    && !item.icon.trim().is_empty())
                .then_some(item.icon)
            })
        });
    let Some(panel_icon) = panel_icon else {
        return String::new();
    };
    shell_owner_appid()
        .and_then(|owner_appid| lxapp::try_get(&owner_appid))
        .and_then(|owner| resolve_asset_path(owner.runtime.asset_dir(), &panel_icon))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or(panel_icon)
}

fn auxiliary_lxapp_id(raw: &str) -> Option<&str> {
    raw.strip_prefix(AUX_PINNED_LXAPP_PREFIX)
        .or_else(|| raw.strip_prefix(AUX_LXAPP_PREFIX))
        .map(str::trim)
        .filter(|appid| !appid.is_empty())
}

fn auxiliary_surface_id(raw: &str) -> Option<&str> {
    raw.strip_prefix(AUX_SURFACE_PREFIX)
        .map(str::trim)
        .filter(|surface_id| !surface_id.is_empty())
}

fn set_lxapp_pin_with_limit(owner_appid: &str, appid: &str, pinned: bool) -> bool {
    if pinned && is_home_lxapp(appid) {
        log::warn!("ignoring Pin request for home lxapp '{appid}'");
        return false;
    }
    match lingxia_shell::set_pinned(
        ShellPinTarget::Lxapp {
            key: appid.to_string(),
        },
        pinned,
    ) {
        Ok(_) => true,
        Err(lingxia_shell::ShellError::LimitReached { .. }) => {
            show_pin_limit_message(owner_appid);
            false
        }
        Err(error) => {
            log::warn!("failed to update lxapp Pin '{appid}': {error}");
            false
        }
    }
}

fn is_home_lxapp(appid: &str) -> bool {
    lingxia_app_context::home_app_id().is_some_and(|home| home == appid)
        || lxapp::try_get(appid).is_some_and(|app| app.runtime_info().is_home)
}

fn is_lxapp_pinned(appid: &str) -> bool {
    if is_home_lxapp(appid) {
        return false;
    }
    lingxia_shell::is_pinned(&ShellPinTarget::Lxapp {
        key: appid.to_string(),
    })
    .unwrap_or(false)
}

fn show_pin_limit_message(appid: &str) {
    let title = lingxia_logic::i18n::t(lingxia_logic::I18nKey::ShellPinLimitTitle);
    let message = lingxia_logic::i18n::t(lingxia_logic::I18nKey::ShellPinLimitMessage);
    if let Some(window) = owner_window_handle(appid) {
        crate::window_host::show_shell_notice(window, title, message);
    }
}

#[cfg(feature = "browser-shell")]
fn auxiliary_bookmark(raw: &str) -> Option<lingxia_browser_shell::BookmarkEntry> {
    let id = raw
        .strip_prefix(AUX_BOOKMARK_PREFIX)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    lingxia_browser_shell::bookmarks_snapshot()?
        .entries
        .into_iter()
        .find(|entry| entry.id == id)
}

#[cfg(feature = "browser-shell")]
fn pinned_bookmark_for_url(url: &str) -> Option<lingxia_browser_shell::BookmarkEntry> {
    let normalized = lingxia_browser_shell::normalize_bookmark_url(url);
    let pinned_ids = runtime_shell_pins()
        .into_iter()
        .filter_map(|pin| match pin.0 {
            ShellPinTarget::Bookmark { key } => Some(key),
            ShellPinTarget::Lxapp { .. } => None,
        })
        .collect::<HashSet<_>>();
    lingxia_browser_shell::bookmarks_snapshot()?
        .entries
        .into_iter()
        .find(|entry| {
            pinned_ids.contains(&entry.id)
                && lingxia_browser_shell::normalize_bookmark_url(&entry.url) == normalized
        })
}

/// Sidebar row title for a browser tab: page title, else the URL host,
/// else localized "New Tab".
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
    lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserNewTab)
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

fn set_runtime_sidebar_actions(items: &[ResolvedShellSidebarAction]) -> bool {
    let state = RUNTIME_SIDEBAR_ACTIONS.get_or_init(|| Mutex::new(None));
    if let Ok(mut state) = state.lock() {
        *state = Some(items.to_vec());
    } else {
        return false;
    }
    if let Some(owner_appid) = shell_owner_appid() {
        sync_shell_layout(&owner_appid);
    }
    true
}

fn open_builtin_browser_downloads() -> bool {
    #[cfg(not(feature = "browser-runtime"))]
    {
        false
    }
    #[cfg(feature = "browser-runtime")]
    {
        let Some(appid) = shell_owner_appid() else {
            return false;
        };
        let Some(app) = lxapp::try_get(&appid) else {
            return false;
        };
        open_or_present_trusted_browser_page(&appid, app.session_id(), "lingxia://downloads")
    }
}

fn runtime_sidebar_actions() -> Option<Vec<ResolvedShellSidebarAction>> {
    let cached = RUNTIME_SIDEBAR_ACTIONS
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.clone());
    if cached.is_some() {
        lingxia_shell::resolved_sidebar_actions().ok().or(cached)
    } else {
        None
    }
}

fn build_footer_actions(app: &LxApp) -> Vec<WindowsShellFooterActionLayout> {
    // Footer actions are desktop sidebar affordances. A mobile presentation —
    // the device-framed runner or a phone-style bottom tab bar — never
    // shows them (matching the mobile platforms).
    if owner_window_handle(&app.appid).is_some_and(window_has_device_frame)
        || tabbar_position(&app.appid) == WindowsShellTabBarPosition::Bottom
    {
        return Vec::new();
    }
    let mut actions = resolved_sidebar_actions_for_placement(app, SidebarActionPlacement::Footer)
        .into_iter()
        .map(|item| WindowsShellFooterActionLayout {
            generation: item.generation,
            id: item.id,
            label: item.label,
            icon_path: resolved_sidebar_action_icon_path(app, item.icon_path.as_deref()),
            disabled: item.disabled,
            source: WindowsShellSidebarActionSource::Runtime,
        })
        .collect::<Vec<_>>();
    if let Some(source) = static_settings_source() {
        actions.push(WindowsShellFooterActionLayout {
            generation: 0,
            id: crate::static_settings::STATIC_SETTINGS_ACTION_ID.to_string(),
            label: "Settings".to_string(),
            icon_path: app
                .runtime
                .asset_dir()
                .join("icons")
                .join("design")
                .join("icon_browser_settings.png")
                .to_string_lossy()
                .into_owned(),
            disabled: false,
            source: WindowsShellSidebarActionSource::StaticSettings(source.destination_kind),
        });
    }
    actions
}

static STATIC_SETTINGS_SOURCE: OnceLock<
    Mutex<Option<crate::static_settings::WindowsStaticSettingsSource>>,
> = OnceLock::new();
static HOST_RUNTIME: OnceLock<lingxia::RuntimeInfo> = OnceLock::new();

pub(crate) fn configure_static_settings_source(
    destination: Option<&lingxia_app_context::SettingsDestination>,
    runtime: &lingxia::RuntimeInfo,
) {
    let _ = HOST_RUNTIME.set(runtime.clone());
    let source = crate::static_settings::WindowsStaticSettingsSource::from_destination(destination);
    let state = STATIC_SETTINGS_SOURCE.get_or_init(|| Mutex::new(None));
    if let Ok(mut state) = state.lock() {
        *state = source;
    }
    if let Some(owner_appid) = shell_owner_appid() {
        sync_shell_layout(&owner_appid);
    }
}

fn static_settings_source() -> Option<crate::static_settings::WindowsStaticSettingsSource> {
    STATIC_SETTINGS_SOURCE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| *state)
}

fn activate_static_settings(item_id: &str) -> bool {
    static_settings_source().is_some_and(|source| {
        HOST_RUNTIME.get().is_some_and(|runtime| {
            source.activate(
                item_id,
                || runtime.resolve_settings_destination(),
                present_static_settings_resolution,
            )
        })
    })
}

fn present_static_settings_resolution(resolution: lingxia::SettingsDestinationResolution) {
    #[cfg(feature = "browser-runtime")]
    if let lingxia::SettingsDestinationResolution::BrowserControlPage { tab_id, .. } = resolution {
        let owner_appid = shell_owner_appid()
            .unwrap_or_else(|| lingxia_browser::BUILTIN_BROWSER_APPID.to_string());
        present_browser_tab_when_ready(&owner_appid, tab_id);
    }
    #[cfg(not(feature = "browser-runtime"))]
    let _ = resolution;
}

fn resolved_sidebar_actions_for_placement(
    _app: &LxApp,
    placement: SidebarActionPlacement,
) -> Vec<ResolvedShellSidebarAction> {
    runtime_sidebar_actions()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            item.placement == placement
                && crate::static_settings::WindowsStaticSettingsSource::accepts_runtime_action(
                    &item.id,
                )
        })
        .collect()
}

fn resolved_sidebar_action_icon_path(app: &LxApp, icon: Option<&str>) -> String {
    let Some(icon) = icon else {
        return String::new();
    };
    resolve_asset_path(app.runtime.asset_dir(), icon)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| icon.to_string())
}

fn set_runtime_shell_pins(items: &[ShellPin]) -> bool {
    let state = RUNTIME_SHELL_PINS.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut state) = state.lock() else {
        return false;
    };
    *state = items.to_vec();
    drop(state);
    if let Some(owner_appid) = shell_owner_appid() {
        sync_shell_layout(&owner_appid);
    }
    true
}

fn runtime_shell_pins() -> Vec<ShellPin> {
    RUNTIME_SHELL_PINS
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| state.clone())
        .unwrap_or_default()
}

fn shell_surface_is_active(surface_id: &str) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    lxapp::try_get(&owner_appid)
        .and_then(|owner| owner.surface_derived_layout())
        .is_some_and(|plan| {
            plan.aside_slots.iter().any(|slot| {
                slot.visible
                    && slot.active_child.as_deref() == Some(surface_id)
                    && slot.children.iter().any(|child| child == surface_id)
            })
        })
}

fn shell_surface_in_graph(surface_id: &str) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    lxapp::try_get(&owner_appid)
        .and_then(|owner| owner.surface_derived_layout())
        .is_some_and(|plan| plan.asides.iter().any(|aside| aside.id == surface_id))
}

/// Native host panels are not owned by the platform WebView presenter, so the
/// SDK reconciles the native aside slot here. Main providers use an explicit
/// two-phase handoff: the incoming provider becomes paintable before the old
/// native main is hidden. Hiding mains from this plan callback would expose an
/// empty content card while a WebView is still becoming ready.
fn apply_windows_layout_plan(plan: &LayoutPresentationPlan) {
    reconcile_lxapp_main_from_layout(plan);

    let native_slot = plan
        .aside_slots
        .iter()
        .find(|slot| slot.kind == SlotKind::Native);
    let active = native_slot.filter(|slot| slot.visible).and_then(|slot| {
        slot.active_child
            .as_deref()
            .or_else(|| slot.children.last().map(String::as_str))
    });
    let edge = native_slot.and_then(|slot| slot.edge);
    let overlay = native_slot.is_some_and(|slot| slot.visible && slot.overlay);

    let mut native_panels = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .map(|panels| {
            panels
                .items
                .into_iter()
                .filter_map(|item| {
                    (!item.content.kind.is_lxapp()).then_some(TerminalPanelRequest {
                        panel_id: item.id,
                        label: item.label,
                        position: item.position,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if lingxia_app_context::terminal_enabled()
        && !native_panels
            .iter()
            .any(|request| request.panel_id == SHELL_TERMINAL_SURFACE_ID)
    {
        native_panels.push(TerminalPanelRequest {
            panel_id: SHELL_TERMINAL_SURFACE_ID.to_string(),
            label: "Terminal".to_string(),
            position: lingxia_app_context::PanelPosition::Bottom,
        });
    }
    if let Some(slot) = native_slot {
        // Keyed terminal ids are runtime-owned and therefore absent from the
        // YAML panel list; keep them in the renderer's reconciliation set.
        let position = slot
            .edge
            .map(panel_position_from_edge)
            .unwrap_or(lingxia_app_context::PanelPosition::Bottom);
        let label = lingxia_logic::i18n::t(lingxia_logic::I18nKey::TerminalTitle);
        for panel_id in &slot.children {
            if !native_panels
                .iter()
                .any(|request| request.panel_id == *panel_id)
            {
                native_panels.push(TerminalPanelRequest {
                    panel_id: panel_id.clone(),
                    label: label.clone(),
                    position,
                });
            }
        }
    }

    for mut request in native_panels {
        if Some(request.panel_id.as_str()) == active {
            if let Some(edge) = edge {
                request.position = panel_position_from_edge(edge);
            }
            present_terminal_from_layout(request, overlay);
        } else if is_panel_visible(&request.panel_id)
            && let Err(err) = hide_host_panel(&request.panel_id)
        {
            log::warn!(
                "failed to hide non-admitted native aside {}: {err}",
                request.panel_id
            );
        }
    }
    if let Some(owner_appid) = shell_owner_appid() {
        sync_shell_layout(&owner_appid);
    }
}

/// Lxapp mains are runtime-owned, so the generic Windows surface presenter
/// cannot look them up in its page/native provider registry. Reconcile them
/// from the same authoritative plan here, just as native asides are reconciled
/// below; otherwise `Surface.show()` changes the switcher graph while leaving
/// the old controller in the physical workspace.
fn reconcile_lxapp_main_from_layout(plan: &LayoutPresentationPlan) {
    let Some(active_id) = plan.active_main_id.as_deref() else {
        return;
    };
    let Some(app_id) = plan
        .main_switcher
        .items
        .iter()
        .find_map(|item| (item.surface_id == active_id).then_some(&item.content))
        .and_then(|content| match content {
            SwitcherContentKind::Lxapp { app_id } => Some(app_id.as_str()),
            SwitcherContentKind::Page { .. }
            | SwitcherContentKind::Browser
            | SwitcherContentKind::Native { .. } => None,
        })
    else {
        return;
    };
    let explicitly_requested = take_lxapp_main_activation(app_id);
    #[cfg(not(feature = "browser-runtime"))]
    let _ = explicitly_requested;
    let physical_app_id = active_host_window_webtag_key()
        .and_then(|key| key.split_once(':').map(|(appid, _)| appid.to_string()));
    if physical_app_id.as_deref() == Some(app_id) {
        return;
    }
    // A browser tab intentionally covers the graph's lxapp main until its own
    // close/back action restores it. An explicit lxapp activation is the one
    // exception: it replaces that cover; resize/aside commits preserve it.
    #[cfg(feature = "browser-runtime")]
    if presented_browser_tab().is_some() {
        if !explicitly_requested {
            return;
        }
        clear_browser_presentation();
    }
    let Some(app) = lxapp::try_get(app_id) else {
        return;
    };
    if !present_current_lxapp_main(&app) {
        log::warn!("failed to reconcile Windows lxapp main from layout: {app_id}");
    }
}

fn present_terminal_from_layout(request: TerminalPanelRequest, overlay: bool) {
    let position = panel_position(request.position);
    let title = if request.label.trim().is_empty() {
        "Terminal"
    } else {
        request.label.trim()
    };
    match super::terminal_panel::show_existing_windows_terminal_panel(
        &request.panel_id,
        title,
        position,
    ) {
        Ok(true) => {
            super::terminal_panel::set_terminal_panel_maximized(&request.panel_id, overlay);
            return;
        }
        Ok(false) => {}
        Err(err) => {
            log::warn!(
                "failed to restore Windows native aside {}: {err}",
                request.panel_id
            );
            return;
        }
    }
    if let Err(err) =
        super::terminal_panel::open_windows_terminal_panel(&request.panel_id, title, position)
    {
        log::warn!(
            "failed to show Windows native aside {}: {err}",
            request.panel_id
        );
    } else {
        super::terminal_panel::set_terminal_panel_maximized(&request.panel_id, overlay);
    }
}

fn panel_position_from_edge(edge: Edge) -> lingxia_app_context::PanelPosition {
    match edge {
        Edge::Left => lingxia_app_context::PanelPosition::Left,
        Edge::Right => lingxia_app_context::PanelPosition::Right,
        Edge::Top => lingxia_app_context::PanelPosition::Top,
        Edge::Bottom => lingxia_app_context::PanelPosition::Bottom,
    }
}

fn handle_managed_aside_event(event: WindowsAsidePanelEvent) {
    match event {
        WindowsAsidePanelEvent::TabClick { surface_id, .. } => {
            if let Some(owner_appid) = shell_owner_appid()
                && let Some(owner) = lxapp::try_get(&owner_appid)
            {
                owner.focus_shell_surface(&surface_id);
            }
        }
        WindowsAsidePanelEvent::TabClose { surface_id, .. } => {
            close_managed_aside_child(&surface_id);
        }
        WindowsAsidePanelEvent::Collapse { panel_id } => {
            if let Some(owner_appid) = shell_owner_appid()
                && let Some(owner) = lxapp::try_get(&owner_appid)
            {
                owner.set_shell_slot_collapsed(aside_slot_kind(&panel_id), true);
            }
        }
        WindowsAsidePanelEvent::NavBack { .. }
        | WindowsAsidePanelEvent::NavForward { .. }
        | WindowsAsidePanelEvent::NavReload { .. } => {}
    }
}

/// Slot kind behind a well-known aside panel id.
fn aside_slot_kind(panel_id: &str) -> &'static str {
    match panel_id {
        lingxia_windows_contract::ASIDE_BROWSER_PANEL_ID => "browser",
        lingxia_windows_contract::ASIDE_LXAPP_PANEL_ID => "lxapp",
        _ => "native",
    }
}

fn close_managed_aside_child(surface_id: &str) {
    let Some(owner_appid) = shell_owner_appid() else {
        return;
    };
    if let Some(owner) = lxapp::try_get(&owner_appid) {
        owner.unregister_host_aside(surface_id);
    }
    if surface_id == SHELL_TERMINAL_SURFACE_ID {
        if let Err(error) = hide_host_panel(surface_id) {
            log::warn!("failed to close shell terminal: {error}");
        }
        return;
    }
    match panel_target_for_id(surface_id) {
        Some(PanelTarget::LxApp { appid, .. }) => {
            if let Err(err) = lxapp::close_lxapp(&appid) {
                log::warn!("failed to close Windows lxapp aside {appid}: {err}");
            }
        }
        Some(PanelTarget::Terminal(_)) => {
            if let Err(err) = hide_host_panel(surface_id) {
                log::warn!("failed to close Windows native aside {surface_id}: {err}");
            }
        }
        None => {
            // Runtime (undeclared) lxapp asides use appId as their surface id.
            if let Err(err) = lxapp::close_lxapp(surface_id) {
                log::warn!("failed to close Windows runtime aside {surface_id}: {err}");
            }
        }
    }
}

fn dispatch_aside_panel_event(event: WindowsAsidePanelEvent) {
    if !dispatch_windows_aside_panel_event(event) {
        log::warn!("aside panel event dropped: no handler installed");
    }
}

fn handle_chrome_event(appid: &str, event: WindowsChromeCommand) {
    // Chrome events arrive on the active content webview's handler, which may
    // belong to a presented non-owner lxapp (a second lxapp opened over the
    // shell owner's card). The navigation bar is that visible page's, but
    // every other command (sidebar rows, browser tabs, panels, app menu)
    // targets the shell owner's chrome state — route there instead of
    // dropping the event.
    let page_scoped = chrome_command_is_page_scoped(event.id.as_str());
    let tabbar_target = matches!(
        event.id.as_str(),
        chrome_command::TAB_BAR_CLICK | chrome_command::TAB_BAR_MORE_CLICK
    )
    .then(|| payload_string(&event, "group"))
    .flatten();
    let appid = if let Some(target) = tabbar_target {
        target
    } else if page_scoped || is_shell_owner_appid(appid) {
        appid.to_string()
    } else {
        match shell_owner_appid() {
            Some(owner) => owner,
            None => {
                log::debug!("ignoring Windows shell chrome event without a shell owner: {appid}");
                return;
            }
        }
    };
    let appid = appid.as_str();
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };

    let handled = match event.id.as_str() {
        chrome_command::TAB_BAR_CLICK => {
            let Some(index) = payload_usize(&event, "index") else {
                return;
            };
            // Clear browser presentation state without restoring the saved
            // lxapp first. That restore target may be an older tab page; showing
            // it before SwitchTab presents the requested page creates a very
            // visible old-page -> target-page flicker.
            let returning_from_browser = clear_browser_presentation();
            let switching_main = active_main_lxapp_id().as_deref() != Some(appid);
            if switching_main {
                app.set_active_main();
            }
            prime_tabbar_selection(&app, index);
            let _ = app.on_lxapp_event(LxAppUiEventType::TabBarClick, index.to_string());
            if (returning_from_browser || switching_main) && !present_current_lxapp_main(&app) {
                if returning_from_browser {
                    if let Err(err) = restore_presented_group_main() {
                        log::warn!("failed to restore lxapp webview for {appid}: {err}");
                    }
                } else {
                    log::warn!("failed to present tabbar owner lxapp {appid}");
                }
            }
            return;
        }
        chrome_command::TAB_BAR_MORE_CLICK => {
            show_tabbar_overflow(appid);
            return;
        }
        chrome_command::NAVIGATION_BACK => {
            app.on_lxapp_event(LxAppUiEventType::NavigationClick, "back".to_string())
        }
        chrome_command::NAVIGATION_HOME => {
            let returning_from_browser = clear_browser_presentation();
            let handled = app.on_lxapp_event(LxAppUiEventType::NavigationClick, "home".to_string());
            if returning_from_browser
                && !present_current_lxapp_main(&app)
                && let Err(err) = restore_presented_group_main()
            {
                log::warn!("failed to restore lxapp webview for {appid}: {err}");
            }
            handled
        }
        // The device-framed browser's close button: dismiss the presented tab
        // back to the lxapp (tabs stay alive, like the macOS phone browser).
        chrome_command::BROWSER_CLOSE => {
            return_to_lxapp_from_browser(appid);
            sync_shell_layout(appid);
            return;
        }
        chrome_command::FOOTER_ACTION_CLICK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(generation) = payload_u64(&event, "generation") else {
                return;
            };
            if let Err(error) = lingxia_shell::activate_sidebar_action(SidebarActionIntent {
                id: panel_id.clone(),
                generation,
            }) {
                log::warn!("shell sidebar action '{panel_id}' failed: {error}");
            }
            sync_shell_layout(appid);
            return;
        }
        chrome_command::STATIC_SETTINGS_CLICK => {
            let Some(item_id) = payload_string(&event, "panel_id") else {
                return;
            };
            if !activate_static_settings(&item_id) {
                log::warn!("static Settings action '{item_id}' failed");
            }
            sync_shell_layout(appid);
            return;
        }
        chrome_command::BROWSER_TABS_CYCLE => {
            handle_browser_tabs_toggle(appid, payload_isize(&event, "source_window"));
            return;
        }
        chrome_command::BROWSER_NEW_TAB => {
            #[cfg(feature = "browser-runtime")]
            if presented_browser_tab()
                .as_deref()
                .is_some_and(lingxia_browser::tab_is_aside)
            {
                return;
            }
            handle_browser_new_tab(appid, app.session_id());
            return;
        }
        chrome_command::MAIN_WORKSPACE_ADD => {
            handle_main_workspace_add(app.clone());
            return;
        }
        chrome_command::BROWSER_TAB_CLICK => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            if let Some(surface_id) = auxiliary_surface_id(&tab_id) {
                handle_main_surface_click(appid, surface_id);
                return;
            }
            #[cfg(feature = "browser-runtime")]
            if SELF_BROWSER_HOST.load(Ordering::Acquire) && is_browser_root_group_entry(&tab_id) {
                if let Some(active) = self_browser_root_tab()
                    .filter(|root| browser_tab_summary(root).is_some())
                    .or_else(presented_browser_tab)
                    .or_else(|| lingxia_browser::current_tab().map(|tab| tab.tab_id))
                {
                    handle_browser_tab_click(appid, &active);
                }
                return;
            }
            if let Some(target_appid) = auxiliary_lxapp_id(&tab_id) {
                handle_lxapp_auxiliary_click(appid, target_appid);
                return;
            }
            #[cfg(feature = "browser-shell")]
            if let Some(bookmark) = auxiliary_bookmark(&tab_id) {
                open_or_present_browser_page(appid, app.session_id(), &bookmark.url);
                return;
            }
            if payload_bool(&event, "compact_group") {
                handle_compact_browser_tab_click(appid, &tab_id);
            } else {
                handle_browser_tab_click(appid, &tab_id);
            }
            return;
        }
        chrome_command::BROWSER_TAB_CLOSE => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            if let Some(surface_id) = auxiliary_surface_id(&tab_id) {
                handle_main_surface_close(appid, surface_id);
                return;
            }
            if let Some(target_appid) = auxiliary_lxapp_id(&tab_id) {
                handle_lxapp_auxiliary_close(appid, target_appid);
                return;
            }
            #[cfg(feature = "browser-shell")]
            if auxiliary_bookmark(&tab_id).is_some() {
                return;
            }
            if payload_bool(&event, "compact_group") {
                handle_compact_browser_tab_close(appid, &tab_id);
            } else {
                handle_browser_tab_close(appid, &tab_id);
            }
            return;
        }
        chrome_command::SIDEBAR_AUXILIARY_CONTEXT_MENU => {
            let Some(tab_id) = payload_string(&event, "tab_id") else {
                return;
            };
            let screen_x = payload_i32(&event, "screen_x").unwrap_or(0);
            let screen_y = payload_i32(&event, "screen_y").unwrap_or(0);
            if let Some(surface_id) = auxiliary_surface_id(&tab_id) {
                show_main_surface_context_menu(
                    appid,
                    surface_id,
                    payload_isize(&event, "source_window"),
                    screen_x,
                    screen_y,
                );
            } else if let Some(target_appid) = auxiliary_lxapp_id(&tab_id) {
                show_lxapp_auxiliary_context_menu(appid, target_appid, screen_x, screen_y);
            } else if tab_id.starts_with(AUX_BOOKMARK_PREFIX) {
                show_pinned_bookmark_context_menu(appid, &tab_id, screen_x, screen_y);
            } else {
                show_browser_tab_context_menu(appid, &tab_id, screen_x, screen_y);
            }
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
        // Aside browser panel (grouped web asides): routed to the surface
        // layer, which owns the tab group.
        chrome_command::ASIDE_PANEL_TAB_CLICK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(surface_id) = payload_string(&event, "surface_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::TabClick {
                panel_id,
                surface_id,
            });
            return;
        }
        chrome_command::ASIDE_PANEL_TAB_CLOSE => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            let Some(surface_id) = payload_string(&event, "surface_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::TabClose {
                panel_id,
                surface_id,
            });
            return;
        }
        chrome_command::ASIDE_PANEL_COLLAPSE => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::Collapse { panel_id });
            return;
        }
        chrome_command::ASIDE_PANEL_NAV_BACK => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::NavBack { panel_id });
            return;
        }
        chrome_command::ASIDE_PANEL_NAV_FORWARD => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::NavForward { panel_id });
            return;
        }
        chrome_command::ASIDE_PANEL_NAV_RELOAD => {
            let Some(panel_id) = payload_string(&event, "panel_id") else {
                return;
            };
            dispatch_aside_panel_event(WindowsAsidePanelEvent::NavReload { panel_id });
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
        chrome_command::BROWSER_BOOKMARK_TOGGLE => {
            toggle_presented_tab_bookmark(appid);
            return;
        }
        chrome_command::BROWSER_PIN_TOGGLE => {
            toggle_presented_tab_pin(appid);
            return;
        }
        chrome_command::BROWSER_PAGE_MENU => {
            let screen_x = payload_i32(&event, "screen_x").unwrap_or(0);
            let screen_y = payload_i32(&event, "screen_y").unwrap_or(0);
            show_browser_page_menu(appid, screen_x, screen_y);
            return;
        }
        chrome_command::SIDEBAR_TOGGLE => {
            // User toggle is two-state: expanded <-> icon rail. Fully hidden
            // sidebars are controlled by content-driven auto-hide only.
            let size_class = shell_owner_appid()
                .and_then(|owner| lxapp::try_get(&owner))
                .and_then(|app| app.surface_derived_layout())
                .map(|plan| plan.size_class)
                .unwrap_or(SizeClass::Expanded);
            update_sidebar_ui_state(appid, |state| {
                toggle_sidebar_projection(state, size_class);
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
        chrome_command::SIDEBAR_SCROLL => {
            let Some(group) = payload_string(&event, "group") else {
                return;
            };
            let Some(offset) = payload_i32(&event, "offset") else {
                return;
            };
            update_sidebar_ui_state(&group, |state| {
                state.main_scroll_offset = offset.max(0);
            });
            sync_shell_layout(appid);
            return;
        }
        chrome_command::FOOTER_ACTION_SCROLL => {
            let Some(group) = payload_string(&event, "group") else {
                return;
            };
            let Some(row) = payload_usize(&event, "row") else {
                return;
            };
            update_sidebar_ui_state(&group, |state| {
                state.footer_action_scroll_row = row;
            });
            sync_shell_layout(appid);
            return;
        }
        chrome_command::SIDEBAR_ACTION => {
            let Some(action_id) = payload_string(&event, "action_id") else {
                return;
            };
            let Some(generation) = payload_u64(&event, "generation") else {
                return;
            };
            if let Err(error) = lingxia_shell::activate_sidebar_action(SidebarActionIntent {
                id: action_id.clone(),
                generation,
            }) {
                log::warn!("shell sidebar action '{action_id}' failed: {error}");
            }
            sync_shell_layout(appid);
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

#[cfg(feature = "browser-runtime")]
fn is_browser_root_group_entry(tab_id: &str) -> bool {
    tab_id
        .strip_prefix(AUX_LXAPP_PREFIX)
        .is_some_and(|appid| appid == lingxia_browser::BUILTIN_BROWSER_APPID)
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

fn chrome_command_is_page_scoped(command: &str) -> bool {
    matches!(
        command,
        chrome_command::NAVIGATION_BACK
            | chrome_command::NAVIGATION_HOME
            | chrome_command::TAB_BAR_CLICK
            | chrome_command::TAB_BAR_MORE_CLICK
    )
}

/// Presents the compact strip's folded items in an in-frame grid sheet.
fn show_tabbar_overflow(appid: &str) {
    let Some(app) = lxapp::try_get(appid) else {
        return;
    };
    let Some(tabbar) = build_tab_bar_layout(&app, &[]) else {
        return;
    };
    if !matches!(tabbar.position, WindowsShellTabBarPosition::Bottom)
        || tabbar.bottom_overflow_start().is_none()
    {
        return;
    }
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    crate::window_host::toggle_tabbar_overflow(window, tabbar);
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

fn payload_isize(command: &WindowsChromeCommand, field: &str) -> Option<isize> {
    command
        .payload
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| isize::try_from(value).ok())
}

fn payload_bool(command: &WindowsChromeCommand, field: &str) -> bool {
    command
        .payload
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
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
    if !clear_browser_presentation() {
        return;
    }
    restore_lxapp_main_after_browser(Some(appid));
}

/// Restores the graph's current lxapp main before consulting the physical
/// cover stack. Browser tab-close notifications are debounced, so their saved
/// target may have closed or stopped being active before this callback runs.
fn restore_lxapp_main_after_browser(fallback_appid: Option<&str>) {
    let active_appid = active_main_lxapp_id();
    if active_appid
        .as_deref()
        .and_then(lxapp::try_get)
        .as_deref()
        .is_some_and(present_current_lxapp_main)
    {
        return;
    }

    match crate::window_host::try_restore_presented_group_main() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => log::warn!("failed to restore covered main after browser exit: {err}"),
    }

    let fallback = fallback_appid.filter(|appid| Some(*appid) != active_appid.as_deref());
    if fallback
        .and_then(lxapp::try_get)
        .as_deref()
        .is_some_and(present_current_lxapp_main)
    {
        return;
    }

    log::warn!(
        "browser exit had no live lxapp main to restore (active={active_appid:?}, fallback={fallback_appid:?})"
    );
}

/// Clears only the browser chrome-selection state. Callers that immediately
/// navigate/focus another lxapp main can then replace the browser directly,
/// without flashing the stale saved restore target in between.
fn clear_browser_presentation() -> bool {
    cancel_pending_browser_presentation();
    let state_presented = presented_browser_tab().is_some();
    if !state_presented && !active_host_is_browser() {
        return false;
    }
    set_presented_browser_tab(None);
    true
}

#[cfg(feature = "browser-runtime")]
fn cancel_pending_browser_presentation() {
    BROWSER_PRESENT_EPOCH.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "browser-runtime"))]
fn cancel_pending_browser_presentation() {}

#[cfg(feature = "browser-runtime")]
fn active_host_is_browser() -> bool {
    active_host_window_webtag_key().is_some_and(|key| {
        key.strip_prefix(lingxia_browser::BUILTIN_BROWSER_APPID)
            .is_some_and(|suffix| suffix.starts_with(':'))
    })
}

#[cfg(not(feature = "browser-runtime"))]
fn active_host_is_browser() -> bool {
    false
}

fn present_current_lxapp_main(app: &LxApp) -> bool {
    let path = app
        .peek_current_page()
        .unwrap_or_else(|| app.initial_route());
    if path.is_empty() {
        return false;
    }
    // Page webtags are per-instance; resolve the live instance instead of
    // reconstructing a tag from the route.
    let Some(webtag) = app.get_page(&path).map(|page| page.webtag()) else {
        return false;
    };
    log::debug!(
        "presenting current Windows lxapp main appid={} path={} webtag={}",
        app.appid,
        path,
        webtag.key()
    );
    install_shell_chrome_event_handler(&webtag, &app.appid);
    let _ = set_webview_window_layout(
        &webtag,
        WindowsWindowLayout::new(build_window_layout(app, &path)),
    );
    match present_webview_in_active_group(&webtag) {
        Ok(()) => true,
        Err(err) => {
            log::warn!(
                "failed to present current lxapp main {}:{}: {err}",
                app.appid,
                path
            );
            false
        }
    }
}

/// Opens a new browser tab at `lingxia://newtab` owned by the shell app
/// and presents it once its webview is ready.
#[cfg(feature = "browser-runtime")]
fn handle_browser_new_tab(appid: &str, session_id: u64) {
    // Without the browser webui there is no `lingxia://newtab` start page;
    // a new tab is a blank page, like the macOS runner.
    #[cfg(feature = "browser-shell")]
    const NEW_TAB_URL: &str = "lingxia://newtab";
    #[cfg(not(feature = "browser-shell"))]
    const NEW_TAB_URL: &str = "about:blank";
    let trusted_runtime = HOST_RUNTIME.get();
    if SELF_BROWSER_HOST.load(Ordering::Acquire) {
        let opened = if NEW_TAB_URL.starts_with("lingxia://") {
            trusted_runtime
                .ok_or_else(|| {
                    lxapp::LxAppError::UnsupportedOperation(
                        "native browser runtime authority is not initialized".to_string(),
                    )
                })
                .and_then(|runtime| runtime.open_trusted_browser_page(NEW_TAB_URL, None))
        } else {
            lingxia_browser::open(NEW_TAB_URL, None)
        };
        match opened {
            Ok(tab_id) => present_browser_tab_when_ready_with_policy(
                lingxia_browser::BUILTIN_BROWSER_APPID,
                tab_id,
                NEW_TAB_URL == "about:blank",
            ),
            Err(err) => log::error!("failed to open browser-only tab: {err}"),
        }
        return;
    }
    let opened = if NEW_TAB_URL.starts_with("lingxia://") {
        trusted_runtime
            .ok_or_else(|| {
                lxapp::LxAppError::UnsupportedOperation(
                    "native browser runtime authority is not initialized".to_string(),
                )
            })
            .and_then(|runtime| {
                runtime.open_trusted_browser_page_for_app(appid, session_id, NEW_TAB_URL, None)
            })
    } else {
        lingxia_browser::open_for_app(appid, session_id, NEW_TAB_URL, None)
    };
    match opened {
        Ok(tab_id) => {
            present_browser_tab_when_ready_with_policy(appid, tab_id, NEW_TAB_URL == "about:blank")
        }
        Err(err) => log::error!("failed to open new browser tab for {appid}: {err}"),
    }
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_new_tab(_appid: &str, _session_id: u64) {}

fn handle_main_workspace_add(app: Arc<LxApp>) {
    match main_workspace_add_target(&app.surface_switcher_snapshot()) {
        Some(MainWorkspaceAddTarget::Browser) => {
            handle_browser_new_tab(&app.appid, app.session_id());
        }
        Some(MainWorkspaceAddTarget::Terminal { declaration_id }) => {
            handle_terminal_workspace_add(app, declaration_id);
        }
        None => {}
    }
}

#[cfg(feature = "terminal-runtime")]
fn handle_terminal_workspace_add(app: Arc<LxApp>, declaration_id: String) {
    let key = format!(
        "windows-shell-{}",
        NEXT_SHELL_TERMINAL_WORKSPACE_KEY.fetch_add(1, Ordering::Relaxed)
    );
    std::mem::drop(lingxia::task::spawn(async move {
        if let Err(error) = app
            .open_shell_native_surface(
                &declaration_id,
                Some(&key),
                Some(lxapp::SurfaceRole::Main),
                None,
            )
            .await
        {
            log::error!("failed to open terminal workspace from Windows sidebar: {error}");
        }
    }));
}

#[cfg(not(feature = "terminal-runtime"))]
fn handle_terminal_workspace_add(_app: Arc<LxApp>, _declaration_id: String) {}

/// Toggles the phone tab-switcher sheet (the macOS runner's in-frame
/// bottom sheet) listing every open tab.
#[cfg(feature = "browser-runtime")]
fn handle_browser_tabs_toggle(appid: &str, source_window: Option<isize>) {
    let presented = presented_browser_tab();
    let aside = presented
        .as_deref()
        .is_some_and(lingxia_browser::tab_is_aside);
    let tabs: Vec<(String, String, bool)> = browser_tabs()
        .into_iter()
        .filter(|tab| lingxia_browser::tab_is_aside(&tab.tab_id) == aside)
        .map(|tab| {
            let active = presented.as_deref() == Some(tab.tab_id.as_str());
            let title = browser_tab_display_title(&tab);
            (tab.tab_id, title, active)
        })
        .collect();
    if tabs.is_empty() {
        return;
    }
    // Chrome input already carries the HWND that painted and received the
    // click. Registry lookups can lag a rapid page replacement and otherwise
    // mount the popup on a stale, hidden host with obsolete dimensions.
    let visible = |window: &isize| crate::window_host::host_window_is_visible(*window);
    let owner = source_window
        .filter(visible)
        .or_else(|| crate::window_host::primary_host_window_handle().filter(visible))
        .or_else(|| owner_window_handle(appid).filter(visible))
        .or_else(|| {
            presented
                .as_deref()
                .and_then(browser_tab_window_handle)
                .filter(visible)
        });
    let Some(owner) = owner else {
        log::warn!("compact browser tab switcher has no owner window for {appid}");
        return;
    };
    log::debug!(
        "toggling compact browser tab switcher owner={owner} appid={appid} presented={presented:?} tabs={}",
        tabs.len()
    );
    crate::window_host::toggle_phone_tab_switcher(owner, tabs);
}

#[cfg(feature = "browser-runtime")]
fn browser_tab_window_handle(tab_id: &str) -> Option<isize> {
    let tab = browser_tab_summary(tab_id)?;
    let webtag = WebTag::new(
        lingxia_browser::BUILTIN_BROWSER_APPID,
        &tab.path,
        Some(tab.session_id),
    );
    lingxia_windows_contract::webview_window_snapshot(&webtag)
        .ok()
        .map(|snapshot| snapshot.window_id as isize)
}

#[cfg(feature = "browser-runtime")]
fn presented_browser_window_handle() -> Option<isize> {
    presented_browser_tab()
        .as_deref()
        .and_then(browser_tab_window_handle)
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_tabs_toggle(_appid: &str, _source_window: Option<isize>) {}

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
fn handle_compact_browser_tab_click(appid: &str, tab_id: &str) {
    let Some(presented) = presented_browser_tab() else {
        return;
    };
    if browser_tab_summary(tab_id).is_none()
        || lingxia_browser::tab_is_aside(&presented) != lingxia_browser::tab_is_aside(tab_id)
    {
        return;
    }
    handle_browser_tab_click(appid, tab_id);
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_compact_browser_tab_click(_appid: &str, _tab_id: &str) {}

#[cfg(feature = "browser-runtime")]
fn handle_browser_tab_close(appid: &str, tab_id: &str) {
    if reset_browser_only_last_tab(tab_id, false) {
        sync_shell_layout(appid);
        return;
    }
    let was_presented = presented_browser_tab().as_deref() == Some(tab_id);
    let successor = was_presented
        .then(|| adjacent_main_tab(tab_id, &HashSet::from([tab_id])))
        .flatten();
    if let Err(err) = lingxia_browser::close(tab_id) {
        log::error!("failed to close browser tab {tab_id}: {err}");
    }
    if was_presented {
        activate_main_tab(appid, successor.as_deref());
    } else if !has_browser_main_tabs() {
        // Closing tabs in quick succession can remove the successor before
        // its asynchronous presentation updates PRESENTED_BROWSER_TAB. The
        // last close must still restore the lxapp that the physical browser
        // controller covers, even though the state marker names an older tab.
        return_to_lxapp_from_browser(appid);
    }
    // The tabs-changed observer re-syncs as well; sync directly so the row
    // disappears even if no observer is installed.
    sync_shell_layout(appid);
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_tab_close(_appid: &str, _tab_id: &str) {}

#[cfg(feature = "browser-runtime")]
fn handle_compact_browser_tab_close(appid: &str, tab_id: &str) {
    let Some(presented) = presented_browser_tab() else {
        return;
    };
    let aside = lingxia_browser::tab_is_aside(&presented);
    if browser_tab_summary(tab_id).is_none() || lingxia_browser::tab_is_aside(tab_id) != aside {
        return;
    }
    if reset_browser_only_last_tab(tab_id, aside) {
        sync_shell_layout(appid);
        return;
    }
    let was_presented = presented == tab_id;
    let successor = was_presented
        .then(|| adjacent_browser_tab_in_mode(tab_id, aside, &HashSet::from([tab_id])))
        .flatten();
    if let Err(err) = lingxia_browser::close(tab_id) {
        log::error!("failed to close browser tab {tab_id}: {err}");
    }
    if was_presented {
        activate_main_tab(appid, successor.as_deref());
    }
    sync_shell_layout(appid);
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_compact_browser_tab_close(_appid: &str, _tab_id: &str) {}

#[cfg(feature = "browser-runtime")]
fn reset_browser_only_last_tab(tab_id: &str, aside: bool) -> bool {
    if !SELF_BROWSER_HOST.load(Ordering::Acquire) || aside {
        return false;
    }
    let tabs: Vec<_> = browser_tabs()
        .into_iter()
        .filter(|tab| !lingxia_browser::tab_is_aside(&tab.tab_id))
        .collect();
    if tabs.len() != 1 || tabs[0].tab_id != tab_id {
        return false;
    }
    if let Err(error) = lingxia_browser::open("about:blank", Some(tab_id)) {
        log::error!("failed to reset the browser-only root tab: {error}");
    }
    true
}

#[derive(Debug, Clone, Copy)]
enum BrowserTabContextAction {
    #[cfg(feature = "browser-shell")]
    TogglePin,
    Close,
    CloseOtherTabs,
    CloseTabsBelow,
}

fn show_browser_tab_context_menu(appid: &str, tab_id: &str, screen_x: i32, screen_y: i32) {
    let tabs = browser_tabs();
    let Some(index) = tabs.iter().position(|tab| tab.tab_id == tab_id) else {
        return;
    };
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    use super::context_menu::ContextMenuEntry;
    use crate::WindowsDesignIcon;
    use lingxia_logic::I18nKey;
    let mut actions = Vec::new();
    let mut items = Vec::new();
    #[cfg(feature = "browser-shell")]
    if let Some(url) = tabs[index]
        .current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    {
        let pinned = pinned_bookmark_for_url(url).is_some();
        actions.extend([Some(BrowserTabContextAction::TogglePin), None]);
        items.extend([
            ContextMenuEntry::item(
                lingxia_logic::i18n::t(if pinned {
                    I18nKey::BrowserUnpin
                } else {
                    I18nKey::BrowserPinToSidebar
                }),
                true,
                if pinned {
                    WindowsDesignIcon::Unpin
                } else {
                    WindowsDesignIcon::Pin
                },
            ),
            ContextMenuEntry::separator(),
        ]);
    }
    actions.push(Some(BrowserTabContextAction::Close));
    items.push(ContextMenuEntry::item(
        lingxia_logic::i18n::t(I18nKey::CommonClose),
        true,
        WindowsDesignIcon::CloseX,
    ));
    if tabs.len() > 1 {
        actions.push(Some(BrowserTabContextAction::CloseOtherTabs));
        items.push(ContextMenuEntry::item(
            lingxia_logic::i18n::t(I18nKey::BrowserCloseOtherTabs),
            true,
            WindowsDesignIcon::CloseOtherTabs,
        ));
    }
    if index + 1 < tabs.len() {
        actions.push(Some(BrowserTabContextAction::CloseTabsBelow));
        items.push(ContextMenuEntry::item(
            lingxia_logic::i18n::t(I18nKey::BrowserCloseTabsBelow),
            true,
            WindowsDesignIcon::CloseTabsBelow,
        ));
    }
    let appid = appid.to_string();
    let tab_id = tab_id.to_string();
    super::context_menu::show_context_menu_entries(
        window,
        (screen_x, screen_y),
        items,
        Arc::new(move |index| {
            if let Some(action) = actions.get(index).copied().flatten() {
                handle_browser_tab_context_action(&appid, &tab_id, action);
            }
        }),
    );
}

#[cfg(feature = "browser-shell")]
fn show_pinned_bookmark_context_menu(appid: &str, row_id: &str, screen_x: i32, screen_y: i32) {
    let Some(bookmark) = auxiliary_bookmark(row_id) else {
        return;
    };
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    let items = vec![
        lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserUnpin),
        lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserManageBookmarks),
    ];
    let appid = appid.to_string();
    super::context_menu::show_context_menu_checked(
        window,
        (screen_x, screen_y),
        items,
        Vec::new(),
        Arc::new(move |index| match index {
            0 => {
                let command = serde_json::json!({
                    "op": "setPinned",
                    "id": bookmark.id,
                    "pinned": false,
                });
                let _ = lingxia_browser_shell::bookmarks_command_json(&command.to_string());
            }
            1 => {
                if let Some(app) = lxapp::try_get(&appid) {
                    open_or_present_trusted_browser_page(
                        &appid,
                        app.session_id(),
                        "lingxia://bookmarks",
                    );
                }
            }
            _ => {}
        }),
    );
}

#[cfg(not(feature = "browser-shell"))]
fn show_pinned_bookmark_context_menu(_appid: &str, _row_id: &str, _screen_x: i32, _screen_y: i32) {}

#[cfg(feature = "browser-runtime")]
fn handle_browser_tab_context_action(appid: &str, tab_id: &str, action: BrowserTabContextAction) {
    match action {
        #[cfg(feature = "browser-shell")]
        BrowserTabContextAction::TogglePin => {
            if let Some(tab) = browser_tab_summary(tab_id) {
                toggle_browser_tab_pin(appid, &tab);
            }
        }
        BrowserTabContextAction::Close => handle_browser_tab_close(appid, tab_id),
        BrowserTabContextAction::CloseOtherTabs => close_other_browser_tabs(appid, tab_id),
        BrowserTabContextAction::CloseTabsBelow => close_browser_tabs_below(appid, tab_id),
    }
}

#[cfg(not(feature = "browser-runtime"))]
fn handle_browser_tab_context_action(
    _appid: &str,
    _tab_id: &str,
    _action: BrowserTabContextAction,
) {
}

#[cfg(feature = "browser-runtime")]
fn close_other_browser_tabs(appid: &str, keeping_tab_id: &str) {
    let tab_ids: Vec<String> = browser_tabs()
        .into_iter()
        .map(|tab| tab.tab_id)
        .filter(|tab_id| tab_id != keeping_tab_id)
        .collect();
    close_browser_tab_batch(appid, tab_ids);
}

#[cfg(feature = "browser-runtime")]
fn close_browser_tabs_below(appid: &str, tab_id: &str) {
    let tabs = browser_tabs();
    let Some(index) = tabs.iter().position(|tab| tab.tab_id == tab_id) else {
        return;
    };
    let tab_ids = tabs
        .into_iter()
        .skip(index + 1)
        .map(|tab| tab.tab_id)
        .collect();
    close_browser_tab_batch(appid, tab_ids);
}

#[cfg(feature = "browser-runtime")]
fn close_browser_tab_batch(appid: &str, tab_ids: Vec<String>) {
    let presented_closed = presented_browser_tab()
        .map(|presented| tab_ids.iter().any(|tab_id| tab_id == &presented))
        .unwrap_or(false);
    let closing: HashSet<&str> = tab_ids.iter().map(String::as_str).collect();
    let successor = presented_browser_tab()
        .filter(|_| presented_closed)
        .and_then(|presented| adjacent_main_tab(&presented, &closing));
    for tab_id in tab_ids {
        if let Err(err) = lingxia_browser::close(&tab_id) {
            log::error!("failed to close browser tab {tab_id}: {err}");
        }
    }
    if presented_closed {
        activate_main_tab(appid, successor.as_deref());
    }
    sync_shell_layout(appid);
}

fn handle_main_surface_click(owner_appid: &str, surface_id: &str) {
    let Some(owner) = lxapp::try_get(owner_appid) else {
        return;
    };
    let Some(content) = owner.main_surface_content(surface_id) else {
        return;
    };
    let deferred = match &content {
        lingxia_surface::SurfaceContent::Browser { .. } => true,
        lingxia_surface::SurfaceContent::Native { capability, .. } => capability == "browser",
        _ => false,
    };
    if let Err(error) = present_main_surface(&owner, surface_id) {
        log::warn!("failed to present Windows main surface {surface_id}: {error}");
        return;
    }
    if deferred {
        return;
    }
    if !owner.set_active_main_surface(surface_id) {
        log::warn!("failed to activate Windows main surface {surface_id}");
        return;
    }
    sync_shell_layout(owner_appid);
}

fn handle_main_surface_close(owner_appid: &str, surface_id: &str) {
    let Some(owner) = lxapp::try_get(owner_appid) else {
        return;
    };
    let _ = close_main_surface_and_present(&owner, surface_id, "user");
}

fn close_main_surface_and_present(owner: &LxApp, surface_id: &str, reason: &str) -> bool {
    let content = owner.main_surface_content(surface_id);
    let outcome = owner.close_main_surface_deferred(surface_id, reason);
    if outcome.removed().is_empty() {
        return false;
    }
    for removed in outcome.removed() {
        discard_main_surface_provider(owner, removed, content.as_ref());
    }
    if let Some(active) = owner.surface_switcher_snapshot().active_surface_id
        && let Err(error) = present_successor_main(owner, &active)
    {
        log::warn!("failed to present successor main surface {active}: {error}");
    }
    sync_shell_layout(&owner.appid);
    true
}

fn show_main_surface_context_menu(
    owner_appid: &str,
    surface_id: &str,
    source_window: Option<isize>,
    screen_x: i32,
    screen_y: i32,
) {
    let Some(owner) = lxapp::try_get(owner_appid) else {
        return;
    };
    let Some(snapshot) = owner.shell_surface_menu(surface_id) else {
        return;
    };
    let Some(window) = source_window.or_else(|| owner_window_handle(owner_appid)) else {
        log::warn!("no source window for surface context menu {surface_id}");
        return;
    };
    let mut entries = Vec::new();
    let mut actions = Vec::new();
    for section in &snapshot.sections {
        if !entries.is_empty() && !section.items.is_empty() {
            entries.push(super::context_menu::ContextMenuEntry::separator());
            actions.push(None);
        }
        for item in &section.items {
            entries.push(surface_menu_entry(item));
            actions.push(Some(item.action.clone()));
        }
    }
    let title = owner
        .surface_switcher_snapshot()
        .items
        .iter()
        .find(|item| item.surface_id == surface_id)
        .map(surface_switcher_title)
        .unwrap_or_else(|| surface_id.to_string());
    let owner_appid = owner_appid.to_string();
    let surface_id = snapshot.surface_id;
    let revision = snapshot.revision;
    super::context_menu::show_context_menu_entries(
        window,
        (screen_x, screen_y),
        entries,
        Arc::new(move |index| {
            let Some(action) = actions.get(index).cloned().flatten() else {
                return;
            };
            handle_main_surface_menu_action(
                &owner_appid,
                &surface_id,
                revision,
                &title,
                window,
                action,
            );
        }),
    );
}

fn surface_menu_entry(
    item: &lingxia_shell::SurfaceMenuItem,
) -> super::context_menu::ContextMenuEntry {
    use lingxia_logic::I18nKey;
    use lingxia_shell::{LxappSurfaceMenuAction, SurfaceMenuAction, SurfaceMenuBuiltinAction};
    let (label, icon) = match &item.action {
        SurfaceMenuAction::Information {} => (item.label.clone().unwrap_or_default(), None),
        SurfaceMenuAction::External { .. } => (item.label.clone().unwrap_or_default(), None),
        SurfaceMenuAction::Lxapp { action } => match action {
            LxappSurfaceMenuAction::Restart => {
                (lingxia_logic::i18n::t(I18nKey::CapsuleRestart), None)
            }
            LxappSurfaceMenuAction::CleanCacheRestart => {
                (lingxia_logic::i18n::t(I18nKey::CapsuleCleanCache), None)
            }
        },
        SurfaceMenuAction::Switcher { action } => match action {
            SurfaceMenuBuiltinAction::Rename => {
                (lingxia_logic::i18n::t(I18nKey::SurfaceRename), None)
            }
            SurfaceMenuBuiltinAction::ResetTitle => {
                (lingxia_logic::i18n::t(I18nKey::SurfaceResetTitle), None)
            }
            SurfaceMenuBuiltinAction::Close => (
                lingxia_logic::i18n::t(I18nKey::SurfaceClose),
                Some(crate::WindowsDesignIcon::CloseX),
            ),
            SurfaceMenuBuiltinAction::CloseOthers => (
                lingxia_logic::i18n::t(I18nKey::SurfaceCloseOthers),
                Some(crate::WindowsDesignIcon::CloseOtherTabs),
            ),
            SurfaceMenuBuiltinAction::CloseAfter => (
                lingxia_logic::i18n::t(I18nKey::SurfaceCloseAfter),
                Some(crate::WindowsDesignIcon::CloseTabsBelow),
            ),
        },
    };
    super::context_menu::ContextMenuEntry {
        label,
        enabled: item.enabled,
        checked: false,
        separator: false,
        icon,
    }
}

fn handle_main_surface_menu_action(
    owner_appid: &str,
    surface_id: &str,
    revision: u64,
    title: &str,
    window: isize,
    action: lingxia_shell::SurfaceMenuAction,
) {
    use lingxia_shell::{SurfaceMenuAction, SurfaceMenuBuiltinAction, SurfaceMenuIntent};
    match action {
        SurfaceMenuAction::Switcher {
            action: SurfaceMenuBuiltinAction::Rename,
        } => {
            let owner_appid = owner_appid.to_string();
            let surface_id = surface_id.to_string();
            let target_id = format!("{AUX_SURFACE_PREFIX}{surface_id}");
            let started = super::chrome::begin_sidebar_surface_rename(
                window,
                &target_id,
                title,
                Arc::new(move |value| {
                    let value = value.trim().to_string();
                    if value.is_empty() {
                        return;
                    }
                    perform_main_surface_menu_intent(
                        &owner_appid,
                        SurfaceMenuIntent {
                            revision,
                            surface_id: surface_id.clone(),
                            action: SurfaceMenuAction::Switcher {
                                action: SurfaceMenuBuiltinAction::Rename,
                            },
                            value: Some(value),
                        },
                    );
                }),
            );
            if !started {
                log::warn!("sidebar row is unavailable for inline surface rename");
            }
        }
        action => perform_main_surface_menu_intent(
            owner_appid,
            SurfaceMenuIntent {
                revision,
                surface_id: surface_id.to_string(),
                action,
                value: None,
            },
        ),
    }
}

fn perform_main_surface_menu_intent(owner_appid: &str, intent: lingxia_shell::SurfaceMenuIntent) {
    let Some(owner) = lxapp::try_get(owner_appid) else {
        return;
    };
    let contents = owner
        .surface_switcher_snapshot()
        .items
        .into_iter()
        .filter_map(|item| {
            owner
                .main_surface_content(&item.surface_id)
                .map(|content| (item.surface_id, content))
        })
        .collect::<HashMap<_, _>>();
    let execution = owner.perform_shell_surface_menu_intent_deferred(intent);
    if !execution.accepted {
        return;
    }
    for removed in &execution.removed_surface_ids {
        discard_main_surface_provider(&owner, removed, contents.get(removed));
    }
    if execution.removed_surface_ids.is_empty() {
        owner.commit_shell_surface_layout();
    } else if let Some(active) = execution.snapshot.active_surface_id
        && let Err(error) = present_successor_main(&owner, &active)
    {
        log::warn!("failed to present successor main surface {active}: {error}");
    }
    sync_shell_layout(owner_appid);
}

fn present_main_surface(owner: &LxApp, surface_id: &str) -> Result<(), String> {
    present_main_surface_inner(owner, surface_id, None)
}

fn present_successor_main(owner: &LxApp, surface_id: &str) -> Result<(), String> {
    let content = owner
        .main_surface_content(surface_id)
        .ok_or_else(|| format!("unknown main surface: {surface_id}"))?;
    present_main_surface(owner, surface_id)?;
    let deferred = matches!(content, lingxia_surface::SurfaceContent::Browser { .. })
        || matches!(
            content,
            lingxia_surface::SurfaceContent::Native { capability, .. }
                if capability == "browser"
        );
    if !deferred && !owner.set_active_main_surface(surface_id) {
        return Err(format!("failed to activate main surface: {surface_id}"));
    }
    Ok(())
}

fn present_main_surface_inner(
    owner: &LxApp,
    surface_id: &str,
    completion: Option<ManagedSurfaceCompletion>,
) -> Result<(), String> {
    #[cfg(feature = "browser-runtime")]
    let mut completion = completion;
    let content = owner
        .main_surface_content(surface_id)
        .ok_or_else(|| format!("unknown main surface: {surface_id}"))?;
    let result = crate::window_host::with_host_layout_batch(|| match content {
        lingxia_surface::SurfaceContent::Lxapp { app_id, path } => {
            clear_browser_presentation();
            // A main switch is a focus operation for a live lxapp. Reopening it
            // with its declaration path creates an initial-route WebView beside
            // the retained navigation stack and can leave that stale generation
            // intercepting the shell after the outgoing workspace closes.
            let app = match lxapp::try_get(&app_id) {
                Some(app) => app,
                None => lxapp::open_lxapp(
                    &app_id,
                    LxAppStartupOptions::new(path.as_deref().unwrap_or_default()),
                )
                .map_err(|error| error.to_string())?,
            };
            if !present_current_lxapp_main(&app) {
                return Err(format!("lxapp main is not ready: {app_id}"));
            }
            hide_inactive_native_main_panels(owner, surface_id);
            Ok(())
        }
        #[cfg(feature = "browser-runtime")]
        lingxia_surface::SurfaceContent::Browser { initial_url, .. } => {
            open_declared_browser(&owner.appid, surface_id, &initial_url, completion.take())
        }
        #[cfg(not(feature = "browser-runtime"))]
        lingxia_surface::SurfaceContent::Browser { .. } => {
            Err("browser runtime is unavailable".to_string())
        }
        lingxia_surface::SurfaceContent::Native { capability, .. } if capability == "terminal" => {
            clear_browser_presentation();
            if open_managed_native_surface(surface_id, &capability, None, "main", "") {
                hide_inactive_native_main_panels(owner, surface_id);
                Ok(())
            } else {
                Err(format!("native main is unavailable: {capability}"))
            }
        }
        #[cfg(feature = "browser-runtime")]
        lingxia_surface::SurfaceContent::Native { capability, .. } if capability == "browser" => {
            open_declared_browser(&owner.appid, surface_id, "about:blank", completion.take())
        }
        #[cfg(not(feature = "browser-runtime"))]
        lingxia_surface::SurfaceContent::Native { capability, .. } if capability == "browser" => {
            Err("browser runtime is unavailable".to_string())
        }
        other => Err(format!("unsupported Windows main surface: {other:?}")),
    });
    if let Some(completion) = completion {
        let mut completion = PresentationCompletion(Some(completion));
        completion.finish(
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| PlatformError::Platform(error.clone())),
        );
    }
    result
}

fn hide_inactive_native_main_panels(owner: &LxApp, active_surface_id: &str) {
    for item in owner.surface_switcher_snapshot().items {
        if item.surface_id == active_surface_id
            || !matches!(item.content, SwitcherContentKind::Native { .. })
        {
            continue;
        }
        if is_panel_visible(&item.surface_id)
            && let Err(error) = hide_host_panel(&item.surface_id)
        {
            log::warn!(
                "failed to hide inactive Windows native main {}: {error}",
                item.surface_id
            );
        }
    }
}

fn discard_main_surface_provider(
    owner: &LxApp,
    surface_id: &str,
    content: Option<&lingxia_surface::SurfaceContent>,
) {
    let browser_tab = {
        #[cfg(feature = "browser-runtime")]
        {
            DECLARED_BROWSER_TABS
                .get()
                .and_then(|tabs| tabs.lock().ok())
                .and_then(|mut tabs| tabs.remove(surface_id))
        }
        #[cfg(not(feature = "browser-runtime"))]
        {
            None::<String>
        }
    };
    #[cfg(feature = "browser-runtime")]
    if let Some(tab_id) = browser_tab
        && let Err(error) = lingxia_browser::close(&tab_id)
    {
        log::warn!("failed to close browser provider for {surface_id}: {error}");
    }
    #[cfg(not(feature = "browser-runtime"))]
    let _ = browser_tab;

    #[cfg(feature = "terminal-runtime")]
    let destroyed_terminal = matches!(
        content,
        Some(lingxia_surface::SurfaceContent::Native { capability, .. })
            if capability == "terminal"
    );
    #[cfg(not(feature = "terminal-runtime"))]
    let destroyed_terminal = false;
    #[cfg(feature = "terminal-runtime")]
    if destroyed_terminal {
        super::terminal_panel::destroy_windows_terminal_panel(surface_id);
    }
    if !destroyed_terminal
        && is_panel_visible(surface_id)
        && let Err(error) = hide_host_panel(surface_id)
    {
        log::warn!("failed to close native provider for {surface_id}: {error}");
    }
    if let Some(lingxia_surface::SurfaceContent::Lxapp { app_id, .. }) = content
        && app_id.as_str() != owner.appid.as_str()
        && let Err(error) = lxapp::close_lxapp(app_id)
    {
        log::warn!("failed to close lxapp provider {app_id}: {error}");
    }
}

fn handle_lxapp_auxiliary_click(owner_appid: &str, target_appid: &str) {
    focus_or_open_lxapp(owner_appid, target_appid);
    sync_shell_layout(owner_appid);
    sync_shell_layout(target_appid);
}

fn handle_lxapp_auxiliary_close(owner_appid: &str, target_appid: &str) {
    if !main_lxapp_closable(target_appid) {
        return;
    }
    let was_active = presented_browser_tab().is_none()
        && active_main_lxapp_id().as_deref() == Some(target_appid);
    let row_id = format!("{AUX_LXAPP_PREFIX}{target_appid}");
    let successor = was_active
        .then(|| adjacent_main_tab(&row_id, &HashSet::from([row_id.as_str()])))
        .flatten();
    if let Err(err) = lxapp::close_lxapp(target_appid) {
        log::error!("failed to close sidebar lxapp {target_appid}: {err}");
    }
    if let Some(owner) = lxapp::try_get(owner_appid) {
        owner.forget_surface(target_appid);
    }
    if was_active {
        activate_main_tab(owner_appid, successor.as_deref());
    }
    sync_shell_layout(owner_appid);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LxappShortcutAction {
    Open,
    Focus,
    PromoteAside,
}

fn lxapp_shortcut_action(current: Option<lxapp::LxAppOpenRegion>) -> LxappShortcutAction {
    match current {
        None => LxappShortcutAction::Open,
        Some(lxapp::LxAppOpenRegion::Main) => LxappShortcutAction::Focus,
        Some(lxapp::LxAppOpenRegion::Aside) => LxappShortcutAction::PromoteAside,
    }
}

fn focus_or_open_lxapp(_owner_appid: &str, target_appid: &str) {
    let current_region = lxapp::open_region(target_appid);
    clear_browser_presentation();
    match lxapp_shortcut_action(current_region) {
        LxappShortcutAction::Focus => focus_existing_main_lxapp(target_appid),
        LxappShortcutAction::Open => {
            open_pinned_lxapp_main(target_appid);
        }
        LxappShortcutAction::PromoteAside => {
            let surface_id = panel_item_for_lxapp(target_appid)
                .map(|(panel_id, _, _)| panel_id)
                .unwrap_or_else(|| target_appid.to_string());
            close_managed_aside_child(&surface_id);
            if lxapp::open_region(target_appid).is_some() {
                log::warn!("failed to close pinned lxapp aside before promotion: {target_appid}");
                return;
            }
            open_pinned_lxapp_main(target_appid);
        }
    }
}

fn open_pinned_lxapp_main(target_appid: &str) {
    match lxapp::open_lxapp(
        target_appid,
        LxAppStartupOptions::default().set_release_type(lxapp::host_channel()),
    ) {
        Ok(_) => focus_existing_main_lxapp(target_appid),
        Err(err) => log::warn!("failed to open pinned lxapp {target_appid}: {err}"),
    }
}

/// Focus an already-open main without running its startup path again. Reopening
/// with default options presents the initial route while retaining the live
/// navigation/tab state, producing mismatched content and sidebar selection.
fn focus_existing_main_lxapp(appid: &str) {
    match lxapp::try_get(appid) {
        Some(app) => {
            app.set_active_main();
            if !present_current_lxapp_main(&app) {
                log::warn!("failed to present focused main lxapp {appid}");
            }
        }
        None => log::warn!("failed to focus missing main lxapp {appid}"),
    }
}

fn ordered_main_tabs() -> Vec<String> {
    MAIN_TAB_ORDER
        .get()
        .and_then(|order| order.lock().ok())
        .map(|order| order.clone())
        .unwrap_or_default()
}

fn adjacent_main_tab(current: &str, excluded: &HashSet<&str>) -> Option<String> {
    let order = ordered_main_tabs();
    let index = order.iter().position(|id| id == current)?;
    order
        .iter()
        .skip(index + 1)
        .chain(order[..index].iter().rev())
        .find(|id| !excluded.contains(id.as_str()))
        .cloned()
}

#[cfg(feature = "browser-runtime")]
fn adjacent_browser_tab_in_mode(
    current: &str,
    aside: bool,
    excluded: &HashSet<&str>,
) -> Option<String> {
    let tabs: Vec<String> = browser_tabs()
        .into_iter()
        .filter(|tab| lingxia_browser::tab_is_aside(&tab.tab_id) == aside)
        .map(|tab| tab.tab_id)
        .collect();
    let index = tabs.iter().position(|id| id == current)?;
    tabs.iter()
        .skip(index + 1)
        .chain(tabs[..index].iter().rev())
        .find(|id| !excluded.contains(id.as_str()))
        .cloned()
}

fn activate_main_tab(owner_appid: &str, tab_id: Option<&str>) {
    match tab_id {
        Some(tab_id) if auxiliary_lxapp_id(tab_id).is_some() => {
            focus_or_open_lxapp(owner_appid, auxiliary_lxapp_id(tab_id).unwrap());
        }
        Some(tab_id) => handle_browser_tab_click(owner_appid, tab_id),
        None => return_to_lxapp_from_browser(owner_appid),
    }
}

fn open_lxapp_panel_now(
    target_appid: &str,
    path: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
    panel_id: &str,
) -> Result<(), lxapp::LxAppError> {
    let options = panel_startup_options(target_appid, path, page, query)?;
    lxapp::open_lxapp(
        target_appid,
        options
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(panel_id.to_string()),
    )
    .map(|_| ())
}

fn panel_startup_options(
    target_appid: &str,
    path: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
) -> Result<LxAppStartupOptions, lxapp::LxAppError> {
    if let Some(current) = lxapp::try_get(target_appid).and_then(|panel| panel.peek_current_page())
    {
        return Ok(LxAppStartupOptions::new(&current));
    }
    if page.is_some() || query.is_some() {
        return LxAppStartupOptions::for_page(page, query)
            .map_err(lxapp::LxAppError::InvalidParameter);
    }
    Ok(LxAppStartupOptions::new(path))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LxappContextMenuAction {
    TogglePin,
    Restart,
    CleanCacheRestart,
    More { token: String },
}

fn push_lxapp_context_menu_item(
    items: &mut Vec<String>,
    actions: &mut Vec<Option<LxappContextMenuAction>>,
    label: String,
    action: Option<LxappContextMenuAction>,
) {
    items.push(label);
    actions.push(action);
}

fn build_lxapp_context_menu(
    is_home: bool,
    pinned: bool,
    header_item: String,
) -> (Vec<String>, Vec<Option<LxappContextMenuAction>>) {
    let mut items = Vec::new();
    let mut actions = Vec::new();
    push_lxapp_context_menu_item(&mut items, &mut actions, header_item, None);
    push_lxapp_context_menu_item(&mut items, &mut actions, String::new(), None);
    if !is_home {
        push_lxapp_context_menu_item(
            &mut items,
            &mut actions,
            lingxia_logic::i18n::t(if pinned {
                lingxia_logic::I18nKey::BrowserUnpin
            } else {
                lingxia_logic::I18nKey::BrowserPinToSidebar
            }),
            Some(LxappContextMenuAction::TogglePin),
        );
        push_lxapp_context_menu_item(&mut items, &mut actions, String::new(), None);
    }
    push_lxapp_context_menu_item(
        &mut items,
        &mut actions,
        lingxia_logic::i18n::t(lingxia_logic::I18nKey::CapsuleRestart),
        Some(LxappContextMenuAction::Restart),
    );
    push_lxapp_context_menu_item(
        &mut items,
        &mut actions,
        lingxia_logic::i18n::t(lingxia_logic::I18nKey::CapsuleCleanCache),
        Some(LxappContextMenuAction::CleanCacheRestart),
    );
    (items, actions)
}

fn lxapp_context_menu_header(
    target_appid: &str,
    app_name: Option<&str>,
    version: Option<&str>,
    release_type: Option<&str>,
) -> String {
    let mut header = app_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(target_appid)
        .to_string();
    if let Some(version) = version.map(str::trim).filter(|version| !version.is_empty()) {
        header.push_str(" · ");
        header.push_str(version);
    }
    match release_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "developer" => header.push_str(" [DEV]"),
        "preview" => header.push_str(" [PRE]"),
        _ => {}
    }
    header
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
    let target = lxapp::try_get(target_appid);
    let info = target.as_ref().map(|target| target.get_lxapp_info());
    let is_home = is_home_lxapp(target_appid);
    let pinned = is_lxapp_pinned(target_appid);
    let header_item = lxapp_context_menu_header(
        target_appid,
        info.as_ref().map(|info| info.app_name.as_str()),
        info.as_ref().map(|info| info.version.as_str()),
        info.as_ref().map(|info| info.release_type.as_str()),
    );
    let (mut items, mut actions) = build_lxapp_context_menu(is_home, pinned, header_item);
    if let Some(snapshot) = target.as_ref().map(|target| target.more_actions())
        && !snapshot.items.is_empty()
    {
        let mut custom_items = Vec::with_capacity(snapshot.items.len() + 1);
        let mut custom_actions = Vec::with_capacity(snapshot.items.len() + 1);
        custom_items.push(String::new());
        custom_actions.push(None);
        for (index, item) in snapshot.items.into_iter().enumerate() {
            custom_items.push(item.label);
            custom_actions.push(Some(LxappContextMenuAction::More {
                token: format!("more:{}:{index}", snapshot.generation),
            }));
        }
        items.extend(custom_items);
        actions.extend(custom_actions);
    }
    let entries = items
        .iter()
        .zip(actions.iter())
        .map(|(label, action)| {
            if label.is_empty() {
                super::context_menu::ContextMenuEntry::separator()
            } else {
                super::context_menu::ContextMenuEntry {
                    label: label.clone(),
                    enabled: action.is_some(),
                    checked: false,
                    separator: false,
                    icon: None,
                }
            }
        })
        .collect();
    let owner_appid = owner_appid.to_string();
    let target_appid = target_appid.to_string();
    super::context_menu::show_context_menu_entries(
        window,
        (screen_x, screen_y),
        entries,
        Arc::new(move |index| match actions.get(index).cloned().flatten() {
            Some(LxappContextMenuAction::TogglePin)
                if set_lxapp_pin_with_limit(&owner_appid, &target_appid, !pinned) =>
            {
                sync_shell_layout(&owner_appid);
            }
            Some(LxappContextMenuAction::TogglePin) => {}
            Some(LxappContextMenuAction::Restart) => {
                schedule_lxapp_restart_in_place(target_appid.clone(), false);
            }
            Some(LxappContextMenuAction::CleanCacheRestart) => {
                schedule_lxapp_restart_in_place(target_appid.clone(), true);
            }
            Some(LxappContextMenuAction::More { token }) => {
                if let Some(target) = lxapp::try_get(&target_appid) {
                    let _ = target.on_lxapp_event(LxAppUiEventType::CapsuleClick, token);
                }
            }
            None => {}
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

/// Whether `window`'s device frame owns the window controls (toolbar dots),
/// which is what suppresses the shell's own caption buttons.
#[cfg(feature = "device-frame")]
fn device_frame_owns_window_controls(window: isize) -> bool {
    crate::device_frame::device_frame_owns_window_controls(window)
}

#[cfg(not(feature = "device-frame"))]
fn device_frame_owns_window_controls(_window: isize) -> bool {
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
fn set_device_frame_status_bar_style(
    window: isize,
    foreground: u32,
    background: u32,
    transparent: bool,
) {
    crate::device_frame::set_device_frame_status_bar_style(
        window,
        foreground,
        background,
        transparent,
    );
}

#[cfg(not(feature = "device-frame"))]
fn set_device_frame_status_bar_style(
    _window: isize,
    _foreground: u32,
    _background: u32,
    _transparent: bool,
) {
}

fn schedule_lxapp_restart_in_place(appid: String, clear_cache: bool) {
    // Native context-menu callbacks run on the WebView UI thread. WebView2's
    // synchronous Reload command rejects that thread, after the app service
    // has already restarted, leaving fresh logic behind stale DOM. Run the
    // complete cache/restart/reload transaction on a blocking worker instead.
    std::mem::drop(lingxia::task::spawn_blocking_handle(move || {
        let result = (|| {
            let app =
                lxapp::try_get(&appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
            if clear_cache {
                app.clear_user_cache().map_err(|err| err.to_string())?;
            }
            app.restart_in_place().map_err(|err| err.to_string())
        })();
        if let Err(err) = result {
            let action = if clear_cache {
                "clean cache + restart"
            } else {
                "restart"
            };
            log::warn!("failed to {action} sidebar lxapp {appid}: {err}");
        }
    }));
}

/// Presents `tab_id`'s webview over the main content card, retrying while
/// the tab's WebView creation is still in flight (new tabs create their
/// webview asynchronously).
#[cfg(feature = "browser-runtime")]
fn present_browser_tab_when_ready(appid: &str, tab_id: String) {
    present_browser_tab_when_ready_with_policy(appid, tab_id, false);
}

#[cfg(feature = "browser-runtime")]
fn present_browser_tab_when_ready_with_policy(
    appid: &str,
    tab_id: String,
    allow_intentional_blank: bool,
) {
    present_browser_tab_when_ready_inner(appid, tab_id, allow_intentional_blank, None, None);
}

#[cfg(feature = "browser-runtime")]
fn present_declared_browser_tab_when_ready(
    appid: &str,
    surface_id: &str,
    tab_id: String,
    allow_intentional_blank: bool,
    completion: Option<ManagedSurfaceCompletion>,
) {
    present_browser_tab_when_ready_inner(
        appid,
        tab_id,
        allow_intentional_blank,
        Some(surface_id.to_string()),
        completion,
    );
}

#[cfg(feature = "browser-runtime")]
fn present_browser_tab_when_ready_inner(
    appid: &str,
    tab_id: String,
    allow_intentional_blank: bool,
    activate_surface_id: Option<String>,
    completion: Option<ManagedSurfaceCompletion>,
) {
    let mut completion = PresentationCompletion(completion);
    if let Err(err) = reactivate_browser_tab_if_needed(&tab_id) {
        log::warn!("failed to reactivate browser tab {tab_id}: {err}");
        completion.finish(Err(PlatformError::Platform(err.to_string())));
        return;
    }
    let owner_appid = appid.to_string();
    let epoch = BROWSER_PRESENT_EPOCH.fetch_add(1, Ordering::Relaxed) + 1;
    std::mem::drop(lingxia::task::spawn(async move {
        let mut completion = completion;
        let previous_tab = presented_browser_tab();
        let previous_group = presented_browser_group_appid();
        for attempt in 0..PRESENT_BROWSER_TAB_MAX_RETRY {
            if BROWSER_PRESENT_EPOCH.load(Ordering::Relaxed) != epoch {
                completion.finish(Err(PlatformError::Platform(
                    "browser presentation was superseded".to_string(),
                )));
                return;
            }
            let Some(tab) = browser_tab_summary(&tab_id) else {
                // Tab was closed while waiting.
                completion.finish(Err(PlatformError::AssetNotFound(format!(
                    "browser tab closed before presentation: {tab_id}"
                ))));
                return;
            };
            let webtag = WebTag::new(
                lingxia_browser::BUILTIN_BROWSER_APPID,
                &tab.path,
                Some(tab.session_id),
            );
            let first_presentation = current_window_layout(webtag.key()).is_empty();

            // Do not use `present_webview_in_active_group` as the readiness
            // probe: once a handler exists that call changes the active host
            // immediately. Prime the incoming WebView's FINAL shell layout
            // first, so neither main nor attached panels ever paint an empty
            // or inherited intermediate frame.
            if find_webview_handler(&webtag).is_none() {
                if attempt + 1 == PRESENT_BROWSER_TAB_MAX_RETRY {
                    log::error!("failed to present browser tab {tab_id}: WebView not ready");
                    completion.finish(Err(PlatformError::Platform(format!(
                        "browser WebView was not ready: {tab_id}"
                    ))));
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(
                    PRESENT_BROWSER_TAB_RETRY_DELAY_MS,
                ))
                .await;
                continue;
            }

            // A WebView2 controller exists before its first document frame is
            // ready. Keep the old main until the target document is
            // interactive and has real body content. The Runner's explicit
            // blank new tab is the exception: about:blank is its final content,
            // so waiting for a child element would add the full retry delay.
            let wait_for_first_content =
                first_presentation && !SELF_BROWSER_HOST.load(Ordering::Acquire);
            let content_ready = if wait_for_first_content {
                browser_tab_first_content_ready(&tab_id, allow_intentional_blank).await
            } else {
                true
            };
            if BROWSER_PRESENT_EPOCH.load(Ordering::Relaxed) != epoch {
                completion.finish(Err(PlatformError::Platform(
                    "browser presentation was superseded".to_string(),
                )));
                return;
            }
            if !content_ready && attempt + 1 < PRESENT_BROWSER_TAB_MAX_RETRY {
                tokio::time::sleep(std::time::Duration::from_millis(
                    PRESENT_BROWSER_TAB_RETRY_DELAY_MS,
                ))
                .await;
                continue;
            }
            if !content_ready {
                // Never strand a slow/unusual page in the background forever;
                // after the normal readiness window, present its loading view.
                log::warn!("browser tab {tab_id} had no first-content signal before presentation");
            }

            let group_appid = previous_group
                .clone()
                .or_else(active_main_lxapp_id)
                .unwrap_or_else(|| owner_appid.clone());
            set_presented_browser_group_appid(Some(group_appid));
            set_presented_browser_tab(Some(tab_id.clone()));
            if !browser_tab_shell_layout_is_current(&owner_appid, &webtag)
                && !prime_browser_tab_shell_layout(&owner_appid, &webtag)
            {
                set_presented_browser_tab(previous_tab.clone());
                set_presented_browser_group_appid(previous_group.clone());
                log::error!("failed to prime shell layout for browser tab {tab_id}");
                completion.finish(Err(PlatformError::Platform(format!(
                    "failed to prime browser shell layout: {tab_id}"
                ))));
                return;
            }

            let result = if first_presentation {
                crate::window_host::present_webview_over_active_group_with_snapshot_guard(
                    &webtag,
                    BROWSER_FIRST_FRAME_GUARD_MS,
                )
            } else {
                crate::window_host::present_webview_over_active_group(&webtag)
            };
            match result {
                Ok(()) => {
                    if let Some(surface_id) = activate_surface_id.as_deref() {
                        let Some(owner) = lxapp::try_get(&owner_appid) else {
                            completion.finish(Err(PlatformError::AssetNotFound(format!(
                                "surface owner closed before presentation: {owner_appid}"
                            ))));
                            return;
                        };
                        if !owner.set_active_main_surface(surface_id) {
                            set_presented_browser_tab(previous_tab.clone());
                            set_presented_browser_group_appid(previous_group.clone());
                            if let Some(previous_tab) = previous_tab.clone() {
                                present_browser_tab_when_ready(&owner_appid, previous_tab);
                            } else if let Err(error) = restore_presented_group_main() {
                                log::warn!(
                                    "failed to restore previous main after browser activation race: {error}"
                                );
                            }
                            completion.finish(Err(PlatformError::InvalidParameter(format!(
                                "unknown declared browser surface: {surface_id}"
                            ))));
                            return;
                        }
                        hide_inactive_native_main_panels(&owner, surface_id);
                    }
                    // The target is visible with the final layout now; this
                    // pass only mirrors that state to hidden lxapp webtags and
                    // refreshes chrome data that changed while it was loading.
                    sync_shell_layout(&owner_appid);
                    completion.finish(Ok(()));
                    return;
                }
                Err(err) => {
                    set_presented_browser_tab(previous_tab.clone());
                    set_presented_browser_group_appid(previous_group.clone());
                    sync_shell_layout(&owner_appid);
                    if attempt + 1 == PRESENT_BROWSER_TAB_MAX_RETRY {
                        log::error!("failed to present browser tab {tab_id}: {err}");
                        completion.finish(Err(PlatformError::Platform(format!(
                            "failed to present browser tab {tab_id}: {err}"
                        ))));
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

#[cfg(feature = "browser-runtime")]
fn browser_tab_shell_layout_is_current(owner_appid: &str, webtag: &WebTag) -> bool {
    let current = current_window_layout(webtag.key());
    let desired = if SELF_BROWSER_HOST.load(Ordering::Acquire) {
        build_self_browser_window_layout(webtag)
    } else {
        let Some(app) = lxapp::try_get(owner_appid) else {
            return false;
        };
        match app.peek_current_page() {
            Some(path) if !path.is_empty() => build_window_layout(&app, &path),
            _ => content_agnostic_window_layout(&app),
        }
    };
    current.downcast_ref::<WindowsShellWindowLayout>() == Some(&desired)
}

/// Installs the incoming browser WebView's final chrome/layout without
/// touching the still-visible outgoing main WebView. The target is still on
/// its temporary helper window here, whose layout backend intentionally skips
/// host synchronization while the primary shell host is active.
#[cfg(feature = "browser-runtime")]
fn prime_browser_tab_shell_layout(owner_appid: &str, webtag: &WebTag) -> bool {
    if SELF_BROWSER_HOST.load(Ordering::Acquire) {
        install_shell_chrome_event_handler(webtag, lingxia_browser::BUILTIN_BROWSER_APPID);
        return set_webview_window_layout(
            webtag,
            WindowsWindowLayout::new(build_self_browser_window_layout(webtag)),
        )
        .is_ok();
    }
    let Some(app) = lxapp::try_get(owner_appid) else {
        return false;
    };
    install_shell_chrome_event_handler(webtag, &app.appid);
    let layout = match app.peek_current_page() {
        Some(path) if !path.is_empty() => build_window_layout(&app, &path),
        _ => content_agnostic_window_layout(&app),
    };
    set_webview_window_layout(webtag, WindowsWindowLayout::new(layout)).is_ok()
}

#[cfg(feature = "browser-runtime")]
async fn browser_tab_first_content_ready(tab_id: &str, allow_intentional_blank: bool) -> bool {
    const FIRST_CONTENT_SCRIPT: &str = r#"
        (() => document.readyState !== "loading"
            && !!document.body
            && location.href !== "about:blank"
            && document.body.childElementCount > 0)()
    "#;
    const FIRST_CONTENT_OR_BLANK_SCRIPT: &str = r#"
        (() => document.readyState !== "loading"
            && !!document.body
            && (location.href === "about:blank"
                || document.body.childElementCount > 0))()
    "#;
    let script = if allow_intentional_blank {
        FIRST_CONTENT_OR_BLANK_SCRIPT
    } else {
        FIRST_CONTENT_SCRIPT
    };
    lingxia_browser::evaluate_javascript(tab_id, script)
        .await
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[cfg(feature = "browser-shell")]
fn browser_page_urls_match(current: &str, target: &str) -> bool {
    if let (Some(current_route), Some(target_route)) = (
        browser_internal_page_key(current),
        browser_internal_page_key(target),
    ) {
        return current_route == target_route;
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        lingxia_browser_shell::normalize_bookmark_url(current)
            == lingxia_browser_shell::normalize_bookmark_url(target)
    } else {
        current == target
    }
}

#[cfg(any(feature = "browser-shell", feature = "browser-runtime", test))]
fn browser_internal_page_key(url: &str) -> Option<&'static str> {
    let normalized = url.trim().to_ascii_lowercase();
    let route = normalized
        .strip_prefix("lingxia://")?
        .split(['/', '?', '#'])
        .next()?;
    match route {
        "settings" => Some("settings"),
        "bookmarks" => Some("bookmarks"),
        "history" => Some("history"),
        "downloads" => Some("downloads"),
        _ => None,
    }
}

#[cfg(not(feature = "browser-shell"))]
fn browser_page_urls_match(current: &str, target: &str) -> bool {
    current == target
}

/// Presents `url` as a browser page: when a tab already shows it, that tab
/// is activated and presented; otherwise a new tab opens at `url` (same
/// flow as the sidebar "New Tab" row, just with a target URL).
///
/// Internal `lingxia://` pages match by route, so re-opening Settings finds
/// the existing tab whatever query/fragment it carries. A bare re-open only
/// presents it (keeping scroll and dialog state); a deep link (query or
/// fragment on the target) navigates the tab so hash routing fires.
#[cfg(feature = "browser-runtime")]
fn open_or_present_browser_page(appid: &str, session_id: u64, url: &str) -> bool {
    open_or_present_browser_page_with_authority(
        appid,
        session_id,
        url,
        BrowserPageOpenAuthority::AppSession,
    )
}

#[cfg(feature = "browser-runtime")]
fn open_or_present_trusted_browser_page(appid: &str, session_id: u64, url: &str) -> bool {
    open_or_present_browser_page_with_authority(
        appid,
        session_id,
        url,
        BrowserPageOpenAuthority::NativeControl,
    )
}

#[cfg(feature = "browser-runtime")]
#[derive(Clone, Copy)]
enum BrowserPageOpenAuthority {
    AppSession,
    NativeControl,
}

#[cfg(feature = "browser-runtime")]
#[derive(Debug, PartialEq, Eq)]
enum NativeControlTabTarget<'a> {
    ExistingRuntimeId(&'a str),
    NewOwnerScoped,
}

#[cfg(feature = "browser-runtime")]
fn native_control_tab_target(tab_id: Option<&str>) -> NativeControlTabTarget<'_> {
    match tab_id {
        Some(tab_id) => NativeControlTabTarget::ExistingRuntimeId(tab_id),
        None => NativeControlTabTarget::NewOwnerScoped,
    }
}

#[cfg(feature = "browser-runtime")]
fn open_browser_page_with_authority(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
    authority: BrowserPageOpenAuthority,
) -> Result<String, lxapp::LxAppError> {
    match authority {
        BrowserPageOpenAuthority::AppSession => {
            lingxia_browser::open_for_app(appid, session_id, url, tab_id)
        }
        BrowserPageOpenAuthority::NativeControl => {
            let runtime = HOST_RUNTIME.get().ok_or_else(|| {
                lxapp::LxAppError::UnsupportedOperation(
                    "native browser runtime authority is not initialized".to_string(),
                )
            })?;
            match native_control_tab_target(tab_id) {
                // `browser_tabs()` returns runtime ids, not owner-stable keys.
                // Navigating it through `open_*_for_app` would scope it again
                // and create a hidden sibling instead of updating this tab.
                NativeControlTabTarget::ExistingRuntimeId(tab_id) => {
                    runtime.open_trusted_browser_page(url, Some(tab_id))
                }
                NativeControlTabTarget::NewOwnerScoped
                    if !SELF_BROWSER_HOST.load(Ordering::Acquire) =>
                {
                    runtime.open_trusted_browser_page_for_app(appid, session_id, url, None)
                }
                NativeControlTabTarget::NewOwnerScoped => {
                    runtime.open_trusted_browser_page(url, None)
                }
            }
        }
    }
}

#[cfg(feature = "browser-runtime")]
fn open_or_present_browser_page_with_authority(
    appid: &str,
    session_id: u64,
    url: &str,
    authority: BrowserPageOpenAuthority,
) -> bool {
    let existing = browser_tabs().into_iter().find(|tab| {
        tab.current_url
            .as_deref()
            .is_some_and(|current| browser_page_urls_match(current, url))
    });
    if let Some(existing) = existing {
        if browser_internal_page_deep_link(url)
            && let Err(err) = open_browser_page_with_authority(
                appid,
                session_id,
                url,
                Some(&existing.tab_id),
                authority,
            )
        {
            log::error!("failed to navigate browser page {url}: {err}");
        }
        handle_browser_tab_click(appid, &existing.tab_id);
        return true;
    }
    match open_browser_page_with_authority(appid, session_id, url, None, authority) {
        Ok(tab_id) => {
            present_browser_tab_when_ready(appid, tab_id);
            true
        }
        Err(err) => {
            log::error!("failed to open browser page {url} for {appid}: {err}");
            false
        }
    }
}

#[cfg(feature = "browser-shell")]
fn open_or_present_browser_local_page(
    appid: &str,
    session_id: u64,
    navigation: crate::browser_local_navigation::BrowserLocalNavigation<'_>,
) -> bool {
    open_or_present_trusted_browser_page(appid, session_id, &navigation.url())
}

/// An internal page target that carries a query or fragment, e.g.
/// `lingxia://settings#clear-site-data?tabId=t1`.
#[cfg(any(feature = "browser-shell", feature = "browser-runtime", test))]
fn browser_internal_page_deep_link(url: &str) -> bool {
    browser_internal_page_key(url).is_some() && url.contains(['?', '#'])
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
        let about_label = lingxia_logic::i18n::t(lingxia_logic::I18nKey::CommonAbout);
        let exit_label = lingxia_logic::i18n::t(lingxia_logic::I18nKey::CommonExit);
        let version_label = lingxia_logic::i18n::t(lingxia_logic::I18nKey::CommonVersion);
        let about = AboutInfo {
            title: about_label.clone(),
            app_name,
            version_line: format!("{version_label} {version}"),
            icon_path,
        };
        let items = vec![about_label, exit_label];
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
        let items = vec![lingxia_logic::i18n::t(lingxia_logic::I18nKey::CommonExit)];
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
#[cfg(feature = "browser-shell")]
fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(feature = "shell-chrome")]
pub(super) fn owner_window_handle(appid: &str) -> Option<isize> {
    let app = lxapp::try_get(appid)?;
    // Page webtags are per-instance; resolve the live instance instead of
    // reconstructing a tag from the route.
    let webtag = app
        .current_page()
        .ok()
        .map(|page| page.webtag())
        .or_else(|| app.get_page(&app.initial_route()).map(|page| page.webtag()))?;
    match lingxia_windows_contract::webview_window_snapshot(&webtag) {
        Ok(snapshot) => Some(snapshot.window_id as isize),
        Err(err) => {
            let fallback = crate::window_host::primary_host_window_handle();
            if fallback.is_none() {
                if appid == lxapp::HOST_SURFACE_OWNER_APP_ID {
                    log::debug!("native host shell window is not ready: {err}");
                } else {
                    log::warn!("no shell window handle for {appid}: {err}");
                }
            }
            fallback
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
    if lingxia_browser::tab_is_aside(&tab_id) {
        return;
    }
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    // The capsule was painted by the shell-owner window's chrome; its host
    // window handle comes from the owner webtag's window snapshot.
    let Some(window) = owner_window_handle(&app.appid) else {
        return;
    };

    let owner_appid = app.appid.clone();
    // Keep internal blank-page URLs out of the native edit control too. The
    // painted capsule already presents them as an empty, fresh address field.
    let initial = tab
        .current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !browser_url_is_hidden(url))
        .unwrap_or_default();
    super::begin_address_edit(
        window,
        initial,
        Arc::new(move |text: String| {
            commit_address_input(&owner_appid, &tab_id, &text);
        }),
    );
}

#[cfg(feature = "browser-shell")]
fn toggle_presented_tab_bookmark(appid: &str) {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    let Some(url) = tab
        .current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    else {
        return;
    };
    let title = browser_tab_display_title(&tab);
    let _ = lingxia_browser_shell::toggle_bookmark(url, &title);
    sync_shell_layout(appid);
}

#[cfg(feature = "browser-shell")]
fn toggle_presented_tab_pin(appid: &str) {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    toggle_browser_tab_pin(appid, &tab);
}

#[cfg(feature = "browser-shell")]
fn toggle_browser_tab_pin(appid: &str, tab: &BrowserTabSummary) {
    let Some(url) = tab
        .current_url
        .as_deref()
        .map(str::trim)
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    else {
        return;
    };
    if let Some(entry) = pinned_bookmark_for_url(url) {
        let command = serde_json::json!({
            "op": "setPinned",
            "id": entry.id,
            "pinned": false,
        });
        let _ = lingxia_browser_shell::bookmarks_command_json(&command.to_string());
    } else {
        let title = browser_tab_display_title(tab);
        let pinned = lingxia_browser_shell::pin_bookmark_url_with_favicon(
            url,
            &title,
            tab.favicon_png.as_deref().map(Vec::as_slice),
        );
        if !pinned
            && lingxia_shell::pins().is_ok_and(|pins| pins.len() >= lingxia_shell::MAX_SHELL_PINS)
        {
            show_pin_limit_message(appid);
        }
    }
    sync_shell_layout(appid);
}

#[cfg(not(feature = "browser-shell"))]
fn toggle_presented_tab_pin(_appid: &str) {}

#[cfg(not(feature = "browser-shell"))]
fn toggle_presented_tab_bookmark(_appid: &str) {}

#[cfg(feature = "browser-shell")]
fn show_browser_page_menu(appid: &str, screen_x: i32, screen_y: i32) {
    let Some(tab_id) = presented_browser_tab() else {
        return;
    };
    let Some(tab) = browser_tab_summary(&tab_id) else {
        return;
    };
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    let url = tab.current_url.clone().unwrap_or_default();
    let is_web_url = url.starts_with("http://") || url.starts_with("https://");
    let bookmarked = is_web_url && lingxia_browser_shell::is_bookmarked(&url);
    let pinned_id = pinned_bookmark_for_url(&url).map(|entry| entry.id);
    use super::context_menu::ContextMenuEntry;
    use crate::WindowsDesignIcon;
    let page_actionable = !url.trim().is_empty() && !lingxia_browser_shell::should_hide_url(&url);
    let items = vec![
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(if bookmarked {
                lingxia_logic::I18nKey::BrowserRemoveBookmark
            } else {
                lingxia_logic::I18nKey::BrowserAddBookmark
            }),
            is_web_url,
            if bookmarked {
                WindowsDesignIcon::BookmarkFilled
            } else {
                WindowsDesignIcon::Bookmark
            },
        ),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(if pinned_id.is_some() {
                lingxia_logic::I18nKey::BrowserUnpin
            } else {
                lingxia_logic::I18nKey::BrowserPinToSidebar
            }),
            is_web_url,
            if pinned_id.is_some() {
                WindowsDesignIcon::Unpin
            } else {
                WindowsDesignIcon::Pin
            },
        ),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserCopyLink),
            page_actionable,
            WindowsDesignIcon::Link,
        ),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserOpenInSystemBrowser),
            is_web_url,
            WindowsDesignIcon::External,
        ),
        ContextMenuEntry::separator(),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserManageBookmarks),
            true,
            WindowsDesignIcon::Bookmarks,
        ),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserHistory),
            true,
            WindowsDesignIcon::History,
        ),
        ContextMenuEntry::item(
            "Settings".to_string(),
            true,
            WindowsDesignIcon::BrowserSettings,
        ),
        ContextMenuEntry::separator(),
        ContextMenuEntry::item(
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::BrowserClearSiteData),
            is_web_url,
            WindowsDesignIcon::ClearData,
        ),
    ];
    let appid = appid.to_string();
    let title = browser_tab_display_title(&tab);
    super::context_menu::show_context_menu_entries(
        window,
        (screen_x, screen_y),
        items,
        Arc::new(move |index| match index {
            0 if is_web_url => {
                let _ = lingxia_browser_shell::toggle_bookmark(&url, &title);
            }
            1 if is_web_url => {
                toggle_browser_tab_pin(&appid, &tab);
            }
            2 if page_actionable => {
                let _ = super::clipboard::set_clipboard_text(&url);
            }
            3 if is_web_url => {
                if let Some(app) = lxapp::try_get(&appid) {
                    let _ = app.runtime.open_url(OpenUrlRequest {
                        owner_appid: appid.clone(),
                        owner_session_id: app.session_id(),
                        url: url.clone(),
                        target: OpenUrlTarget::External,
                        want_tab_id: false,
                    });
                }
            }
            5 => {
                if let Some(app) = lxapp::try_get(&appid) {
                    open_or_present_trusted_browser_page(
                        &appid,
                        app.session_id(),
                        "lingxia://bookmarks",
                    );
                }
            }
            6 => {
                if let Some(app) = lxapp::try_get(&appid) {
                    open_or_present_trusted_browser_page(
                        &appid,
                        app.session_id(),
                        "lingxia://history",
                    );
                }
            }
            7 => {
                if let Some(app) = lxapp::try_get(&appid) {
                    open_or_present_browser_local_page(
                        &appid,
                        app.session_id(),
                        crate::browser_local_navigation::BrowserLocalNavigation::Settings,
                    );
                }
            }
            9 if is_web_url => {
                if let Some(app) = lxapp::try_get(&appid) {
                    open_or_present_browser_local_page(
                        &appid,
                        app.session_id(),
                        crate::browser_local_navigation::BrowserLocalNavigation::ClearSiteData {
                            tab_id: &tab_id,
                        },
                    );
                }
            }
            _ => {}
        }),
    );
}

#[cfg(not(feature = "browser-shell"))]
fn show_browser_page_menu(_appid: &str, _screen_x: i32, _screen_y: i32) {}

#[cfg(not(feature = "browser-runtime"))]
fn begin_presented_tab_address_edit(_app: &LxApp) {
    // Without the shell chrome no address bar is drawn (plain OS frame),
    // so there is nothing to edit.
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

fn set_managed_surface_visible_for_api(
    panel_id: &str,
    visible: bool,
    role: &str,
    edge: &str,
    completion: ManagedSurfaceCompletion,
) -> bool {
    accept_managed_request(
        completion,
        |completion| {
            set_managed_surface_visible_inner(panel_id, visible, role, edge, Some(completion))
        },
        PlatformError::AssetNotFound(format!("managed surface request rejected: {panel_id}")),
    )
}

#[cfg(feature = "browser-shell")]
fn set_managed_surface_visible(panel_id: &str, visible: bool, role: &str, edge: &str) -> bool {
    set_managed_surface_visible_inner(panel_id, visible, role, edge, None)
}

fn close_managed_surface_for_api(panel_id: &str, capability: &str, role: &str) -> bool {
    if !role.is_empty() && !matches!(role, "main" | "aside" | "float") {
        return false;
    }
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    let target = if capability == "terminal" {
        PanelTarget::Terminal(TerminalPanelRequest {
            panel_id: panel_id.to_string(),
            label: String::new(),
            position: lingxia_app_context::PanelPosition::Bottom,
        })
    } else if capability.is_empty() {
        let Some(target) = panel_target_for_id(panel_id) else {
            return false;
        };
        target
    } else {
        return false;
    };
    let target_was_lxapp_main = matches!(
        &target,
        PanelTarget::LxApp { appid, .. }
            if lxapp::open_region(appid) == Some(lxapp::LxAppOpenRegion::Main)
    );
    let closing_main = role == "main" || (role.is_empty() && target_was_lxapp_main);
    let result = match target {
        PanelTarget::LxApp { appid, .. } => {
            lxapp::close_lxapp(&appid).map_err(|error| error.to_string())
        }
        PanelTarget::Terminal(_) => {
            #[cfg(feature = "terminal-runtime")]
            {
                super::terminal_panel::destroy_windows_terminal_panel(panel_id);
                Ok(())
            }
            #[cfg(not(feature = "terminal-runtime"))]
            {
                hide_host_panel(panel_id).map_err(|error| error.to_string())
            }
        }
    };
    if let Err(error) = &result {
        log::warn!("failed to destroy Windows managed surface {panel_id}: {error}");
    }
    crate::window_host::set_panel_position_override(panel_id, None);
    if result.is_ok()
        && closing_main
        && let Some(owner) = lxapp::try_get(&owner_appid)
        && let Some(active) = owner.surface_switcher_snapshot().active_surface_id
        && let Err(error) = present_successor_main(&owner, &active)
    {
        log::warn!("failed to present successor after managed main close {active}: {error}");
    }
    sync_shell_layout(&owner_appid);
    result.is_ok()
}

fn set_managed_surface_visible_inner(
    panel_id: &str,
    visible: bool,
    role: &str,
    edge: &str,
    completion: Option<ManagedSurfaceCompletion>,
) -> bool {
    if role == "main" {
        let Some(owner_appid) = shell_owner_appid() else {
            return false;
        };
        let Some(owner) = lxapp::try_get(&owner_appid) else {
            return false;
        };
        return if visible {
            present_main_surface_inner(&owner, panel_id, completion).is_ok()
        } else {
            let closed = close_main_surface_and_present(&owner, panel_id, "programmatic");
            let mut completion = PresentationCompletion(completion);
            completion.finish(if closed {
                Ok(())
            } else {
                Err(PlatformError::Platform(format!(
                    "failed to close managed main surface '{panel_id}'"
                )))
            });
            closed
        };
    }
    if !role.is_empty() && role != "aside" {
        log::warn!("Windows managed surfaces do not support role override: {role}");
        return false;
    }
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    let position_override = match parse_panel_position_override(edge) {
        Ok(position) => position,
        Err(()) => {
            log::warn!("invalid Windows managed surface edge override: {edge}");
            return false;
        }
    };
    let Some(target) = panel_target_for_id(panel_id) else {
        return false;
    };
    if !visible {
        let result = if shell_surface_in_graph(panel_id) {
            hide_panel_target(&owner_appid, panel_id, target)
        } else {
            sync_shell_layout(&owner_appid);
            Ok(())
        };
        let mut completion = PresentationCompletion(completion);
        completion.finish(
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| PlatformError::Platform(error.clone())),
        );
        return result.is_ok();
    }
    if shell_surface_is_active(panel_id) && position_override.is_none() {
        sync_shell_layout(&owner_appid);
        let mut completion = PresentationCompletion(completion);
        completion.finish(Ok(()));
        return true;
    }
    if shell_surface_in_graph(panel_id) && position_override.is_none() {
        if let Some(owner) = lxapp::try_get(&owner_appid) {
            owner.focus_shell_surface(panel_id);
        }
        sync_shell_layout(&owner_appid);
        let mut completion = PresentationCompletion(completion);
        completion.finish(Ok(()));
        return true;
    }
    show_panel_target_inner(
        &owner_appid,
        panel_id,
        target,
        position_override,
        completion,
    )
}

fn open_managed_native_surface_for_api(
    surface_id: &str,
    capability: &str,
    instance_key: Option<&str>,
    role: &str,
    edge: &str,
    completion: ManagedSurfaceCompletion,
) -> bool {
    accept_managed_request(
        completion,
        |completion| {
            let opened =
                open_managed_native_surface(surface_id, capability, instance_key, role, edge);
            if opened {
                completion(Ok(()));
            }
            opened
        },
        PlatformError::AssetNotFound(format!("unsupported managed native surface: {capability}")),
    )
}

#[cfg(feature = "terminal-runtime")]
fn open_managed_native_surface(
    surface_id: &str,
    capability: &str,
    _instance_key: Option<&str>,
    role: &str,
    edge: &str,
) -> bool {
    if capability != "terminal" || !matches!(role, "main" | "aside") {
        return false;
    }
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    let position = match parse_panel_position_override(edge) {
        Ok(position) => position.unwrap_or(lingxia_app_context::PanelPosition::Bottom),
        Err(()) => return false,
    };
    if role == "main" && crate::window_host::primary_host_window_handle().is_none() {
        return open_declared_terminal(&owner_appid, surface_id).is_ok();
    }
    let title = lingxia_logic::i18n::t(lingxia_logic::I18nKey::TerminalTitle);
    let position = panel_position(position);
    let Some(host_window) = crate::window_host::primary_host_window_handle() else {
        return false;
    };
    let surface_id = surface_id.to_string();
    let role = role.to_string();
    match crate::window_host::run_on_window_thread_sync(host_window, move || {
        commit_terminal_surface_handoff(&owner_appid, &surface_id, &title, position, &role)
    }) {
        Ok(opened) => opened,
        Err(error) => {
            log::warn!("failed to commit Windows terminal surface handoff: {error}");
            false
        }
    }
}

#[cfg(feature = "terminal-runtime")]
fn commit_terminal_surface_handoff(
    owner_appid: &str,
    surface_id: &str,
    title: &str,
    position: WindowsPanelPosition,
    role: &str,
) -> bool {
    crate::window_host::with_host_layout_batch(|| {
        if role == "main" {
            cancel_pending_browser_presentation();
        }
        let opened = match super::terminal_panel::show_existing_windows_terminal_panel(
            surface_id, title, position,
        ) {
            Ok(true) => true,
            Ok(false) => {
                super::terminal_panel::open_windows_terminal_panel(surface_id, title, position)
                    .is_ok()
            }
            Err(error) => {
                log::warn!("failed to restore Windows terminal surface {surface_id}: {error}");
                false
            }
        };
        if opened {
            if role == "main" {
                clear_browser_presentation();
            }
            crate::window_host::set_host_panel_zoom_control_visible(surface_id, role != "main");
            super::terminal_panel::set_terminal_panel_maximized(surface_id, role == "main");
            if role == "main"
                && let Some(owner) = lxapp::try_get(owner_appid)
            {
                hide_inactive_native_main_panels(&owner, surface_id);
            }
        }
        opened
    })
}

#[cfg(not(feature = "terminal-runtime"))]
fn open_managed_native_surface(
    _surface_id: &str,
    _capability: &str,
    _instance_key: Option<&str>,
    _role: &str,
    _edge: &str,
) -> bool {
    false
}

#[cfg(feature = "browser-shell")]
fn toggle_managed_surface(panel_id: &str) -> bool {
    let Some(owner_appid) = shell_owner_appid() else {
        return false;
    };
    if panel_target_for_id(panel_id).is_none() {
        return false;
    }
    handle_footer_action(&owner_appid, panel_id.to_string());
    true
}

#[cfg(feature = "browser-shell")]
pub(crate) fn handle_menu_bar_surface_action(
    surface_id: &str,
    action_kind: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
) -> bool {
    if panel_target_for_id(surface_id).is_some() {
        return match action_kind {
            "openSurface" | "focusSurface" => set_managed_surface_visible(surface_id, true, "", ""),
            "closeSurface" => set_managed_surface_visible(surface_id, false, "", ""),
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
        return crate::window_host::primary_host_window_handle()
            .or_else(|| owner_window_handle(&owner_appid))
            .is_some_and(crate::window_host::restore_and_focus_host_window);
    }
    if action_kind == "toggleSurface"
        && let Some(window) = owner_window_handle(&owner_appid)
        && crate::window_host::host_window_is_visible(window)
    {
        return crate::window_host::hide_host_window(window);
    }

    if let Some(window) = crate::window_host::primary_host_window_handle()
        && crate::window_host::restore_and_focus_host_window(window)
    {
        return true;
    }
    let opened = open_home_app_with_target(&owner_appid, page, query).is_ok();
    if let Some(window) = crate::window_host::primary_host_window_handle()
        .or_else(|| owner_window_handle(&owner_appid))
    {
        return crate::window_host::restore_and_focus_host_window(window) || opened;
    }
    opened
}

#[cfg(feature = "browser-shell")]
fn handle_footer_action(appid: &str, panel_id: String) {
    let Some(target) = panel_target_for_id(&panel_id) else {
        log::error!("Windows sidebar footer action was not found: {panel_id}");
        return;
    };

    if shell_surface_is_active(&panel_id) {
        if hide_panel_target(appid, &panel_id, target).is_ok()
            && let Some(owner) = lxapp::try_get(appid)
        {
            owner.mark_shell_surface_hidden(&panel_id);
        }
        return;
    }
    if shell_surface_in_graph(&panel_id) {
        if let Some(owner) = lxapp::try_get(appid) {
            owner.focus_shell_surface(&panel_id);
        }
        sync_shell_layout(appid);
        return;
    }

    show_panel_target(appid, &panel_id, target, None);
}

#[cfg(feature = "browser-shell")]
fn show_panel_target(
    appid: &str,
    panel_id: &str,
    target: PanelTarget,
    position_override: Option<lingxia_app_context::PanelPosition>,
) {
    let _ = show_panel_target_inner(appid, panel_id, target, position_override, None);
}

fn show_panel_target_inner(
    appid: &str,
    panel_id: &str,
    target: PanelTarget,
    position_override: Option<lingxia_app_context::PanelPosition>,
    completion: Option<ManagedSurfaceCompletion>,
) -> bool {
    match target {
        PanelTarget::LxApp {
            appid: panel_appid,
            path,
            page,
            query,
            position,
        } => show_lxapp_panel(
            appid,
            panel_id,
            &panel_appid,
            &path,
            page.as_deref(),
            query.as_ref(),
            position_override.unwrap_or(position),
            position_override.is_some(),
            completion,
        ),
        PanelTarget::Terminal(mut request) => {
            if let Some(position) = position_override {
                request.position = position;
            }
            let result = show_terminal_panel(appid, request);
            if let Some(completion) = completion {
                completion(
                    result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| PlatformError::Platform(error.clone())),
                );
            }
            result.is_ok()
        }
    }
}

fn hide_panel_target(appid: &str, panel_id: &str, target: PanelTarget) -> Result<(), String> {
    let restore_lxapp_active = matches!(&target, PanelTarget::LxApp { .. });
    let mut failure = None;
    match target {
        PanelTarget::LxApp {
            appid: panel_appid, ..
        } => {
            if let Some(panel) = lxapp::try_get(&panel_appid)
                && let Err(err) = panel
                    .runtime
                    .hide_lxapp(panel_appid.clone(), panel.session_id())
            {
                log::error!("failed to close Windows panel lxapp {panel_appid}: {err}");
                failure = Some(err.to_string());
            }
        }
        PanelTarget::Terminal(_) => {}
    }
    if let Err(err) = hide_host_panel(panel_id) {
        log::warn!("failed to hide Windows panel {panel_id}: {err}");
        if failure.is_none() {
            failure = Some(err.to_string());
        }
    }
    crate::window_host::set_panel_position_override(panel_id, None);
    if restore_lxapp_active {
        lxapp::mark_lxapp_active(appid);
    }
    sync_shell_layout(appid);
    failure.map_or(Ok(()), Err)
}

fn show_lxapp_panel(
    owner_appid: &str,
    panel_id: &str,
    panel_appid: &str,
    path: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
    position: lingxia_app_context::PanelPosition,
    has_position_override: bool,
    completion: Option<ManagedSurfaceCompletion>,
) -> bool {
    crate::window_host::set_panel_position_override(
        panel_id,
        has_position_override.then_some(panel_position(position)),
    );
    register_managed_aside(owner_appid, panel_id, position);

    if is_panel_visible(panel_id) {
        sync_shell_layout(owner_appid);
        sync_shell_layout(panel_appid);
        if let Some(completion) = completion {
            completion(Ok(()));
        }
        return true;
    }

    if lxapp::try_get(panel_appid).is_some() {
        if let Err(err) = open_lxapp_panel_now(panel_appid, path, page, query, panel_id) {
            log::error!("failed to show Windows panel lxapp {panel_appid}: {err}");
            crate::window_host::set_panel_position_override(panel_id, None);
            unregister_managed_aside(owner_appid, panel_id);
            drop(completion);
            return false;
        }
        sync_shell_layout(owner_appid);
        sync_shell_layout(panel_appid);
        if let Some(completion) = completion {
            completion(Ok(()));
        }
        return true;
    }

    let panel_id = panel_id.to_string();
    let panel_appid = panel_appid.to_string();
    let path = path.to_string();
    let page = page.map(str::to_string);
    let query = query.cloned();
    if !pending_panel_opens().insert(panel_id.clone()) {
        if let Some(completion) = completion
            && let Ok(mut pending) = PENDING_PANEL_COMPLETIONS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
        {
            pending.entry(panel_id).or_default().push(completion);
        }
        return true;
    }

    if let Some(completion) = completion
        && let Ok(mut pending) = PENDING_PANEL_COMPLETIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
    {
        pending
            .entry(panel_id.clone())
            .or_default()
            .push(completion);
    }

    let owner_appid = owner_appid.to_string();
    std::mem::drop(lingxia::task::spawn(async move {
        let result = open_panel_lxapp(
            &panel_id,
            &panel_appid,
            &path,
            page.as_deref(),
            query.as_ref(),
        )
        .await;
        pending_panel_opens().remove(&panel_id);
        let completions = PENDING_PANEL_COMPLETIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&panel_id))
            .unwrap_or_default();
        for completion in completions {
            completion(
                result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|error| PlatformError::Platform(error.to_string())),
            );
        }
        if let Err(err) = result {
            log::error!("failed to open Windows panel lxapp {panel_appid}: {err}");
            crate::window_host::set_panel_position_override(&panel_id, None);
            unregister_managed_aside(&owner_appid, &panel_id);
            return;
        }
        sync_shell_layout(&owner_appid);
    }));
    true
}

fn panel_target_for_id(panel_id: &str) -> Option<PanelTarget> {
    let configured = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id));

    if let Some(item) = configured {
        return if item.content.kind.is_lxapp() {
            Some(PanelTarget::LxApp {
                appid: item.content.app_id,
                path: item.content.path.unwrap_or_default(),
                page: item.content.page,
                query: item.content.query,
                position: item.position,
            })
        } else {
            Some(PanelTarget::Terminal(TerminalPanelRequest {
                panel_id: item.id,
                label: item.label,
                position: item.position,
            }))
        };
    }

    lxapp::list_lxapps().into_iter().find_map(|info| {
        let app = lxapp::try_get(&info.appid)?;
        (lxapp::open_region(&app.appid) == Some(lxapp::LxAppOpenRegion::Aside)
            && app.open_panel_id().as_deref().unwrap_or(app.appid.as_str()) == panel_id)
            .then(|| PanelTarget::LxApp {
                appid: app.appid.clone(),
                path: String::new(),
                page: None,
                query: None,
                position: lingxia_app_context::PanelPosition::Right,
            })
    })
}

fn panel_item_for_lxapp(
    appid: &str,
) -> Option<(String, String, lingxia_app_context::PanelPosition)> {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| {
            panels.items.into_iter().find_map(|item| {
                (item.content.kind.is_lxapp() && item.content.app_id == appid).then_some((
                    item.id,
                    item.content.path.unwrap_or_default(),
                    item.position,
                ))
            })
        })
}

fn show_terminal_panel(appid: &str, request: TerminalPanelRequest) -> Result<(), String> {
    let position = panel_position(request.position);
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
        register_managed_native_aside(appid, &request.panel_id, request.position);
        sync_shell_layout(appid);
        sync_shell_owner_host_layout(appid);
        return Ok(());
    }
    if let Err(err) =
        super::terminal_panel::open_windows_terminal_panel(&request.panel_id, title, position)
    {
        log::warn!(
            "failed to show Windows terminal panel {}: {}",
            request.panel_id,
            err
        );
        return Err(err);
    }
    register_managed_native_aside(appid, &request.panel_id, request.position);
    sync_shell_layout(appid);
    sync_shell_owner_host_layout(appid);
    Ok(())
}

fn sync_shell_owner_host_layout(appid: &str) {
    let Some(window) = owner_window_handle(appid) else {
        return;
    };
    // A recently hidden aside can remain the process-wide active webtag while
    // its standalone HWND is hidden. Native panels belong to the shell owner,
    // so explicitly refresh that HWND instead of relying on global focus.
    crate::window_host::request_host_window_layout(WindowsHostWindow { window });
    crate::window_host::restore_and_focus_host_window(window);
}

fn register_managed_aside(
    appid: &str,
    panel_id: &str,
    position: lingxia_app_context::PanelPosition,
) {
    if let Some(app) = lxapp::try_get(appid) {
        app.register_host_aside(panel_id, panel_edge(position));
    }
}

fn register_managed_native_aside(
    appid: &str,
    panel_id: &str,
    position: lingxia_app_context::PanelPosition,
) {
    if let Some(app) = lxapp::try_get(appid) {
        app.register_host_aside_content(panel_id, "terminal", panel_edge(position));
    }
}

fn unregister_managed_aside(appid: &str, panel_id: &str) {
    if let Some(app) = lxapp::try_get(appid) {
        app.unregister_host_aside(panel_id);
    }
}

fn parse_panel_position_override(
    edge: &str,
) -> Result<Option<lingxia_app_context::PanelPosition>, ()> {
    let edge = edge.trim();
    if edge.is_empty() {
        return Ok(None);
    }
    match edge {
        "left" => Ok(Some(lingxia_app_context::PanelPosition::Left)),
        "right" => Ok(Some(lingxia_app_context::PanelPosition::Right)),
        "top" => Ok(Some(lingxia_app_context::PanelPosition::Top)),
        "bottom" => Ok(Some(lingxia_app_context::PanelPosition::Bottom)),
        _ => Err(()),
    }
}

fn panel_edge(position: lingxia_app_context::PanelPosition) -> &'static str {
    match position {
        lingxia_app_context::PanelPosition::Left => "left",
        lingxia_app_context::PanelPosition::Right => "right",
        lingxia_app_context::PanelPosition::Top => "top",
        lingxia_app_context::PanelPosition::Bottom => "bottom",
    }
}

async fn open_panel_lxapp(
    panel_id: &str,
    appid: &str,
    path: &str,
    page: Option<&str>,
    query: Option<&serde_json::Value>,
) -> Result<(), lxapp::LxAppError> {
    let channel = lxapp::host_channel();
    lxapp::prepare_lxapp_open(appid, channel).await?;
    let options = panel_startup_options(appid, path, page, query)?;
    let _ = lxapp::open_lxapp(
        appid,
        options
            .set_release_type(channel)
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(panel_id.to_string()),
    )?;
    lxapp::schedule_lxapp_update_check(appid, channel);
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

#[cfg(test)]
mod tests {
    use super::{
        LxappContextMenuAction, LxappShortcutAction, MainWorkspaceAddTarget,
        PresentationCompletion, SidebarUiState, adaptive_tabbar_projection, auxiliary_lxapp_id,
        browser_internal_page_deep_link, browser_internal_page_key, browser_url_is_hidden,
        build_lxapp_context_menu, chrome_command, chrome_command_is_page_scoped,
        lxapp_shortcut_action, main_workspace_add_target_for_capabilities,
        preferred_sidebar_group_appid, sidebar_content_available, toggle_sidebar_projection,
    };
    #[cfg(feature = "browser-runtime")]
    use super::{
        browser_tab_discard_candidates, is_browser_root_group_entry,
        live_browser_tab_limit_for_memory, touch_browser_tab_recency,
    };
    use crate::shell::WindowsShellTabBarPosition;
    use lingxia_surface::{Role, SizeClass, Surface, SurfaceManager};
    #[cfg(feature = "browser-runtime")]
    use std::collections::HashSet;

    #[test]
    fn presentation_completion_finishes_exactly_once() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_for_callback = calls.clone();
        let mut completion = PresentationCompletion(Some(Box::new(move |result| {
            assert!(result.is_ok());
            calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })));
        completion.finish(Ok(()));
        completion.finish(Err(lingxia_platform::error::PlatformError::CallbackDropped));
        drop(completion);
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn dropping_a_pending_presentation_reports_cancellation() {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled_for_callback = cancelled.clone();
        drop(PresentationCompletion(Some(Box::new(move |result| {
            cancelled_for_callback.store(
                matches!(
                    result,
                    Err(lingxia_platform::error::PlatformError::CallbackDropped)
                ),
                std::sync::atomic::Ordering::Relaxed,
            );
        }))));
        assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn tabbar_clicks_stay_scoped_to_the_visible_lxapp() {
        assert!(chrome_command_is_page_scoped(chrome_command::TAB_BAR_CLICK));
        assert!(chrome_command_is_page_scoped(
            chrome_command::NAVIGATION_BACK
        ));
        assert!(!chrome_command_is_page_scoped(
            chrome_command::BROWSER_TAB_CLICK
        ));
    }

    #[test]
    fn desktop_tabbar_projection_tracks_the_core_size_class() {
        use WindowsShellTabBarPosition::{Bottom, Left, Right};

        assert_eq!(
            adaptive_tabbar_projection(Left, SizeClass::Expanded, false, false),
            (Left, false, true)
        );
        assert_eq!(
            adaptive_tabbar_projection(Right, SizeClass::Medium, false, false),
            (Right, true, true)
        );
        assert_eq!(
            adaptive_tabbar_projection(Left, SizeClass::Compact, false, false),
            (Left, true, true)
        );
        assert_eq!(
            adaptive_tabbar_projection(Left, SizeClass::Compact, true, false),
            (Bottom, false, false)
        );
        assert_eq!(
            adaptive_tabbar_projection(Bottom, SizeClass::Medium, true, false),
            (Bottom, false, true)
        );
        assert_eq!(
            adaptive_tabbar_projection(Left, SizeClass::Medium, false, true),
            (Left, false, true)
        );

        let mut state = SidebarUiState::default();
        toggle_sidebar_projection(&mut state, SizeClass::Medium);
        assert!(state.medium_expanded);
        assert!(!state.icon_rail);
        toggle_sidebar_projection(&mut state, SizeClass::Medium);
        assert!(!state.medium_expanded);
        assert!(!state.icon_rail);
    }

    #[test]
    fn terminal_root_exposes_a_terminal_workspace_add_target() {
        let mut manager = SurfaceManager::new(1024.0);
        manager.open(Surface::native("terminal", Role::Main, "terminal"));
        let snapshot = manager.switcher_snapshot();

        let target = main_workspace_add_target_for_capabilities(&snapshot, false, true);
        assert_eq!(
            target,
            Some(MainWorkspaceAddTarget::Terminal {
                declaration_id: "terminal".to_string()
            })
        );
        assert!(sidebar_content_available(
            false,
            false,
            false,
            false,
            target.is_some()
        ));
        assert!(!sidebar_content_available(
            false, false, false, false, false
        ));
        assert_eq!(
            main_workspace_add_target_for_capabilities(&snapshot, true, true),
            Some(MainWorkspaceAddTarget::Terminal {
                declaration_id: "terminal".to_string()
            })
        );
    }

    #[test]
    fn non_terminal_main_falls_back_to_browser_workspace_add() {
        let mut manager = SurfaceManager::new(1024.0);
        manager.open(Surface::lxapp("home", Role::Main, "home"));
        let snapshot = manager.switcher_snapshot();

        assert_eq!(
            main_workspace_add_target_for_capabilities(&snapshot, true, true),
            Some(MainWorkspaceAddTarget::Browser)
        );
        assert_eq!(
            main_workspace_add_target_for_capabilities(&snapshot, false, true),
            None
        );
    }

    #[cfg(feature = "browser-runtime")]
    #[test]
    fn builtin_browser_group_is_not_treated_as_an_lxapp_switch() {
        assert!(is_browser_root_group_entry("lxapp:app.lingxia.browser"));
        assert!(!is_browser_root_group_entry("lxapp:app.example.notes"));
        assert!(!is_browser_root_group_entry("browser-tab-id"));
    }

    #[test]
    fn internal_page_key_ignores_url_decoration() {
        assert_eq!(
            browser_internal_page_key("lingxia://settings"),
            Some("settings")
        );
        assert_eq!(
            browser_internal_page_key("lingxia://settings/#privacy"),
            Some("settings")
        );
        assert_eq!(
            browser_internal_page_key("LINGXIA://BOOKMARKS/?q=rust#top"),
            Some("bookmarks")
        );
        assert_eq!(browser_internal_page_key("lingxia://newtab"), None);
        assert_eq!(browser_internal_page_key("https://example.com"), None);
    }

    #[test]
    fn deep_link_requires_query_or_fragment() {
        assert!(browser_internal_page_deep_link(
            "lingxia://settings#clear-site-data?tabId=t1"
        ));
        assert!(!browser_internal_page_deep_link("lingxia://settings"));
        assert!(!browser_internal_page_deep_link("lingxia://settings/"));
        assert!(!browser_internal_page_deep_link("https://example.com/?q=1"));
    }

    #[cfg(feature = "browser-runtime")]
    #[test]
    fn existing_native_control_tab_ids_are_not_owner_scoped_again() {
        assert_eq!(
            native_control_tab_target(Some("downloads-ownerhash")),
            NativeControlTabTarget::ExistingRuntimeId("downloads-ownerhash")
        );
        assert_eq!(
            native_control_tab_target(None),
            NativeControlTabTarget::NewOwnerScoped
        );
    }

    #[test]
    fn blank_new_tab_urls_stay_out_of_the_address_editor() {
        assert!(browser_url_is_hidden("about:blank"));
        assert!(browser_url_is_hidden(" LINGXIA://NEWTAB "));
        assert!(browser_url_is_hidden("lingxia://"));
        assert!(!browser_url_is_hidden("https://example.com"));
    }

    #[test]
    fn home_group_remains_the_sidebar_owner_across_main_switches() {
        assert_eq!(
            preferred_sidebar_group_appid(
                Some("home".to_string()),
                Some("browser-owner".to_string()),
                Some("app-b".to_string()),
            )
            .as_deref(),
            Some("home")
        );
        assert_eq!(
            preferred_sidebar_group_appid(
                None,
                Some("browser-owner".to_string()),
                Some("app-b".to_string()),
            )
            .as_deref(),
            Some("browser-owner")
        );
        assert_eq!(
            preferred_sidebar_group_appid(None, None, Some("app-b".to_string())).as_deref(),
            Some("app-b")
        );
    }

    #[test]
    fn lxapp_shortcuts_are_workspace_intents() {
        use LxappShortcutAction::{Focus, Open, PromoteAside};
        use lxapp::LxAppOpenRegion::{Aside, Main};

        assert_eq!(lxapp_shortcut_action(None), Open);
        assert_eq!(lxapp_shortcut_action(Some(Main)), Focus);
        assert_eq!(lxapp_shortcut_action(Some(Aside)), PromoteAside);
    }

    #[test]
    fn pin_shortcuts_and_workspace_rows_have_distinct_routable_ids() {
        assert_eq!(auxiliary_lxapp_id("pin:lxapp:chat"), Some("chat"));
        assert_eq!(auxiliary_lxapp_id("lxapp:chat"), Some("chat"));
        assert_ne!("pin:lxapp:chat", "lxapp:chat");
        assert_eq!(auxiliary_lxapp_id("pin:lxapp:"), None);
    }

    #[test]
    fn home_lxapp_context_menu_never_offers_pin() {
        let (_, home_actions) =
            build_lxapp_context_menu(true, false, "Showcase · 1.0.0 [DEV]".to_string());
        assert!(!home_actions.contains(&Some(LxappContextMenuAction::TogglePin)));

        let (_, app_actions) =
            build_lxapp_context_menu(false, false, "Showcase · 1.0.0 [DEV]".to_string());
        assert!(app_actions.contains(&Some(LxappContextMenuAction::TogglePin)));
    }

    #[test]
    fn lxapp_context_menu_header_is_informational() {
        let (items, actions) =
            build_lxapp_context_menu(true, false, "Showcase · 1.0.0 [DEV]".to_string());
        assert_eq!(
            items.first().map(String::as_str),
            Some("Showcase · 1.0.0 [DEV]")
        );
        assert_eq!(actions.first(), Some(&None));
        assert!(items.get(1).is_some_and(String::is_empty));
    }

    #[cfg(feature = "browser-runtime")]
    #[test]
    fn live_browser_tab_limit_scales_with_physical_memory() {
        const GIB: u64 = 1024 * 1024 * 1024;
        assert_eq!(live_browser_tab_limit_for_memory(2 * GIB), 4);
        assert_eq!(live_browser_tab_limit_for_memory(8 * GIB), 8);
        assert_eq!(live_browser_tab_limit_for_memory(16 * GIB), 16);
        assert_eq!(live_browser_tab_limit_for_memory(64 * GIB), 16);
    }

    #[cfg(feature = "browser-runtime")]
    #[test]
    fn browser_tab_discard_candidates_are_lru_and_protect_visible_tabs() {
        let live = ["old", "discarded", "visible", "active", "new"]
            .into_iter()
            .map(str::to_string)
            .collect::<HashSet<_>>();
        let mut recency = Vec::new();
        for tab_id in ["old", "discarded", "visible", "active", "new"] {
            touch_browser_tab_recency(&mut recency, tab_id);
        }
        let discarded = HashSet::from(["discarded".to_string()]);
        let protected = HashSet::from(["visible".to_string(), "active".to_string()]);

        assert_eq!(
            browser_tab_discard_candidates(&live, &recency, &discarded, &protected, 3),
            vec!["old".to_string()]
        );
    }

    #[cfg(feature = "browser-runtime")]
    #[test]
    fn browser_tab_recency_preserves_raw_event_order() {
        let mut recency = Vec::new();
        for tab_id in ["first", "second", "third", "first"] {
            touch_browser_tab_recency(&mut recency, tab_id);
        }
        assert_eq!(recency, ["second", "third", "first"]);
    }
}
