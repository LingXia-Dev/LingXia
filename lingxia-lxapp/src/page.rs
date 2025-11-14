use crate::appservice::bridge::IncomingMessage;
use crate::event::PageLifecycleEvent;
use crate::lxapp::{self, navbar::NavigationBarState};
use crate::startup::parse_query_string;
use crate::{LxApp, LxAppError, error, info};
use http::StatusCode;
use lingxia_platform::{AnimationType, AppRuntime};
use lingxia_webview::{
    LogLevel, WebResourceResponse, WebTag, WebView, WebViewController, WebViewDelegate,
    create_webview,
};

use rong::service_executor;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

/// Inner state of a page that can be shared across threads
#[derive(Clone)]
pub(crate) struct PageInner {
    appid: String,
    path: String,

    // Reference to the WebView (optional, set when WebView is ready)
    webview: Arc<Mutex<Option<Arc<WebView>>>>,

    // Time when this page was last active
    last_active_time: Arc<Mutex<Instant>>,

    // state of Page
    state: Arc<Mutex<PageState>>,

    // One-shot bridge-ready notifier: PageSvc.handle_lxport_ready() will notify
    bridge_ready_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

#[derive(Clone, Debug)]
pub struct PageState {
    // Page(webview) reander status
    render_status: PageRenderStatus,
    // page lifecycle event
    event: PageLifecycleEvent,
    /// Tracks if the UI has requested to show this page. Handles onShow arriving before onLoad.
    show_requested: bool,
    /// Tracks if the onLoad JavaScript event has been fired to prevent duplicates.
    on_load_fired: bool,
    /// Tracks if the onShow JavaScript event has been fired. Reset on hide to allow re-entry.
    on_show_fired: bool,
    /// Tracks if the onReady JavaScript event has been fired to prevent duplicates.
    on_ready_fired: bool,
    // Navigation bar state
    pub(crate) navbar_state: NavigationBarState,
    // Query parameters
    pub(crate) query: serde_json::Value,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum PageRenderStatus {
    Unstarted,
    Started,
    Finished,
}

/// Navigation type for page navigation within LxApp
/// This enum defines the different types of navigation actions that can be performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationType {
    /// Launch navigation - open entry page (clear stack)
    Launch = 0,
    /// Forward navigation - push new page
    Forward = 1,
    /// Backward navigation - pop to previous page
    Backward = 2,
    /// Replace navigation - replace current page
    Replace = 3,
    /// Switch tab navigation - switch between tab pages
    SwitchTab = 4,
}

impl NavigationType {
    /// Convert navigation type to an appropriate animation type for platform runtimes
    pub fn to_animation(self) -> AnimationType {
        match self {
            NavigationType::Forward => AnimationType::Forward,
            NavigationType::Backward => AnimationType::Backward,
            _ => AnimationType::None,
        }
    }
}

/// Represents a single page in a mini app
#[derive(Clone)]
pub struct Page {
    // Use Arc to share the inner state across threads
    inner: Arc<PageInner>,
}

impl Page {
    /// Build PageState from JSON config
    fn build_page_state(lxapp: &lxapp::LxApp, path: &str) -> PageState {
        PageState {
            event: PageLifecycleEvent::Unknown,
            render_status: PageRenderStatus::Unstarted,
            show_requested: false,
            on_load_fired: false,
            on_show_fired: false,
            on_ready_fired: false,
            navbar_state: NavigationBarState::from_json(lxapp, path),
            query: serde_json::Value::Null,
        }
    }

    /// Create a new page in pending state (WebView creation in progress)
    pub(crate) fn new<F>(appid: String, path: String, lxapp: &LxApp, setup_callback: F) -> Self
    where
        F: Fn(&Page) + Send + 'static,
    {
        // Build page state from LxApp configuration
        let page_state = Self::build_page_state(lxapp, &path);
        let inner = Arc::new(PageInner {
            appid: appid.clone(),
            path: path.clone(),
            last_active_time: Arc::new(Mutex::new(Instant::now())),
            state: Arc::new(Mutex::new(page_state)),
            webview: Arc::new(Mutex::new(None)),
        });

        let page = Self { inner };

        // Create channel for WebView creation notification
        let (sender, receiver) = mpsc::channel();

        // Initiate WebView creation asynchronously
        let webtag = WebTag::new(&appid, &path);
        create_webview(&webtag, sender);

        // Spawn task to wait for WebView creation completion
        // Keep a strong reference to ensure page stays alive during WebView creation
        let page_for_task = page.clone();
        let appid_clone = appid.clone();
        let path_clone = path.clone();

        if let Err(e) = service_executor::spawn_blocking(move || {
            match receiver.recv() {
                Ok(Ok(webview_controller)) => {
                    // First attach WebView to page
                    page_for_task.attach_webview(webview_controller.clone());

                    // Then set the page as the WebView delegate
                    // Create a new Arc to avoid potential circular references
                    webview_controller.set_delegate(Arc::new(page_for_task.clone()));

                    // Call setup callback - let external code handle the rest
                    setup_callback(&page_for_task);
                }
                Ok(Err(e)) => {
                    error!("Failed to create WebView: {}", e)
                        .with_appid(appid_clone)
                        .with_path(path_clone);
                }
                Err(_) => {
                    error!("WebView creation channel closed")
                        .with_appid(appid_clone)
                        .with_path(path_clone);
                }
            }
        }) {
            error!("Failed to spawn blocking task for WebView creation: {}", e)
                .with_appid(appid.clone())
                .with_path(path.clone());
        }

        page
    }

    /// Attach WebView to this page (called when WebView is ready)
    fn attach_webview(&self, webview: Arc<WebView>) {
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            *webview_guard = Some(webview);
        }
    }

    /// Get complete page state
    pub fn get_page_state(&self) -> Option<PageState> {
        self.inner.state.lock().ok().map(|state| state.clone())
    }

    /// Set page reander status
    fn set_render_status(&self, status: PageRenderStatus) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.render_status = status;
        }
    }

    /// Returns whether the underlying WebView has started or finished rendering
    /// for this Page instance. Used to decide whether we should manually trigger
    /// onLoad for re-navigation (existing WebView) versus relying on the initial
    /// WebView/LXPort ready path for first-time creation.
    pub(crate) fn has_render_started(&self) -> bool {
        self.inner
            .state
            .lock()
            .map(|s| !matches!(s.render_status, PageRenderStatus::Unstarted))
            .unwrap_or(false)
    }

    pub(crate) fn dispatch_lifecycle_event(&self, event: PageLifecycleEvent) {
        // Central lifecycle state machine for a single WebView-backed Page.
        // Sources of events:
        // - First-time creation: WebView/LXPort ready triggers onLoad (AppService side)
        // - Re-navigation with new query (navigateTo): native manually triggers onLoad
        // - Render completion: WebView delegate triggers onReady
        // - Visibility changes: native triggers onShow/onHide
        // Goals (Weixin semantics adapted to a single WebView instance):
        // - onLoad carries query and may occur multiple times across logical navigations
        //   (first-time + each navigateTo with new params)
        // - onReady fires once for each logical navigation after render has finished
        // - onShow fires each time the page becomes visible (after a hide), without query

        // A collection of events to fire after the lock is released.
        let mut events_to_fire: Vec<(PageLifecycleEvent, Option<String>)> = Vec::new();

        // acquire lock, update state, determine events to fire
        // The lock must be released before calling the executor to avoid deadlocks,
        // in case the JS code calls back into Rust and needs to access page state.
        {
            let mut state = self.inner.state.lock().unwrap();

            // OnHide and OnUnload are handled exclusively and do not trigger the main event cascade.
            if event == PageLifecycleEvent::OnHide || event == PageLifecycleEvent::OnUnload {
                if state.event != event {
                    events_to_fire.push((event, None));
                    state.event = event;
                    // Reset on_show_fired when the page is hidden, to allow onShow to fire again on re-entry.
                    state.on_show_fired = false;
                }
            } else {
                // This logic handles the Load -> Show -> Ready cascade.

                // Update raw status based on the incoming event.
                if event == PageLifecycleEvent::OnShow {
                    state.show_requested = true;
                }

                // Handle onLoad (can occur multiple times for navigateTo with new params)
                if event == PageLifecycleEvent::OnLoad {
                    let query = serde_json::to_string(&state.query).ok();
                    events_to_fire.push((PageLifecycleEvent::OnLoad, query));
                    state.on_load_fired = true;
                    state.on_show_fired = false;
                    state.on_ready_fired = false;
                }

                // Desired order (Weixin semantics): Load -> Show -> Ready (first load)
                // 1) Ready: after Load and render finished; only once per lifecycle
                if state.on_load_fired
                    && state.render_status == PageRenderStatus::Finished
                    && !state.on_ready_fired
                {
                    events_to_fire.push((PageLifecycleEvent::OnReady, None));
                    state.on_ready_fired = true;
                }

                // 2) Show: each time the page becomes visible after hide
                // Do not require Ready; allow Show before Ready on first load.
                if state.on_load_fired && state.show_requested && !state.on_show_fired {
                    events_to_fire.push((PageLifecycleEvent::OnShow, None));
                    state.on_show_fired = true;
                    state.event = PageLifecycleEvent::OnShow;
                }
            }
        }

        //  Fire the collected events outside of the lock to prevent deadlocks.
        if !events_to_fire.is_empty() {
            let lxapp = lxapp::get(self.inner.appid.clone());
            let appid = self.appid();
            let path = self.path();

            for (event, query) in events_to_fire {
                let page_event = match event {
                    PageLifecycleEvent::OnLoad => crate::event::PageServiceEvent::OnLoad,
                    PageLifecycleEvent::OnShow => crate::event::PageServiceEvent::OnShow,
                    PageLifecycleEvent::OnReady => crate::event::PageServiceEvent::OnReady,
                    PageLifecycleEvent::OnHide => crate::event::PageServiceEvent::OnHide,
                    PageLifecycleEvent::OnUnload => crate::event::PageServiceEvent::OnUnload,
                    PageLifecycleEvent::Unknown => {
                        // Skip unknown
                        continue;
                    }
                };

                if let Err(e) = lxapp.executor.call_page_service_event(
                    appid.clone(),
                    path.clone(),
                    page_event,
                    query,
                ) {
                    error!("Failed to call {}: {}", String::from(event), e)
                        .with_appid(appid.clone())
                        .with_path(path.clone());
                }
            }
        }
    }

    /// Get navbar state (read-only)
    pub fn get_navbar_state(&self) -> Option<NavigationBarState> {
        self.inner
            .state
            .lock()
            .ok()
            .map(|state| state.navbar_state.clone())
    }

    /// Get navbar state with mutable access (internal use)
    pub(crate) fn get_navbar_state_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut NavigationBarState) -> R,
    {
        self.inner
            .state
            .lock()
            .ok()
            .map(|mut state| f(&mut state.navbar_state))
    }

    /// Get WebView if available
    pub(crate) fn webview(&self) -> Option<Arc<WebView>> {
        if let Ok(webview_guard) = self.inner.webview.lock() {
            webview_guard.clone()
        } else {
            None
        }
    }

    /// Get the WebView controller for this page (returns None if not ready)
    pub(crate) fn webview_controller(&self) -> Option<Arc<dyn WebViewController>> {
        if let Some(webview) = self.webview() {
            Some(webview as Arc<dyn WebViewController>)
        } else {
            None
        }
    }

    /// Load HTML content into this page's WebView
    pub(crate) fn load_html(&self) -> Result<(), LxAppError> {
        let lxapp = lxapp::get(self.appid());
        let path = self.path();
        let html_data = lxapp.generate_page_html(&path);
        let base_url = self.base_url();

        if let Some(controller) = self.webview_controller() {
            controller
                .load_data(
                    String::from_utf8_lossy(&html_data).to_string(),
                    base_url,
                    None,
                )
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }

    /// Returns the appid of this page
    pub fn path(&self) -> String {
        self.inner.path.clone()
    }

    /// Returns the appid of this page
    pub fn appid(&self) -> String {
        self.inner.appid.clone()
    }

    /// Returns the base URL used when loading this page's HTML (lx://appid/<path>)
    pub fn base_url(&self) -> String {
        format!("lx://{}/{}", self.appid(), self.path())
    }

    /// Update the last active time to now
    pub(crate) fn mark_active(&self) {
        if let Ok(mut time) = self.inner.last_active_time.lock() {
            *time = Instant::now();
        }
    }

    /// Get the last active time for LRU eviction
    pub(crate) fn get_last_active_time(&self) -> Option<Instant> {
        self.inner.last_active_time.lock().ok().map(|time| *time)
    }

    /// Set one-shot sender to be fired when bridge (LXPort) is ready.
    pub(crate) fn set_bridge_ready_sender(&self, tx: mpsc::Sender<()>) {
        if let Ok(mut guard) = self.inner.bridge_ready_tx.lock() {
            *guard = Some(tx);
        }
    }

    /// Notify the waiting thread that bridge is ready (called from PageSvc.handle_lxport_ready).
    pub(crate) fn notify_bridge_ready(&self) {
        if let Ok(mut guard) = self.inner.bridge_ready_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
    }

    /// Check if this page is a TabBar page
    pub fn is_tabbar_page(&self) -> bool {
        let lxapp = lxapp::get(self.inner.appid.clone());
        match lxapp.get_tabbar() {
            Some(tab_bar) => tab_bar.is_tabbar_page(&self.inner.path),
            None => false,
        }
    }

    pub fn navigate_to(
        &self,
        target_page: Page,
        nav_type: NavigationType,
    ) -> Result<Page, LxAppError> {
        let lxapp = lxapp::get(self.appid());

        let path = target_page.path();

        // 2. Handle UI state based on navigation type (TabBar, NavBar)
        let is_tab_switch = nav_type == NavigationType::SwitchTab;
        lxapp.with_tabbar_mut(|t| t.set_visible(is_tab_switch));
        if is_tab_switch {
            if let Some(Some(index)) = lxapp.with_tabbar_mut(|t| t.find_index_by_path(&path)) {
                lxapp.with_tabbar_mut(|t| {
                    t.set_selected_index(index as i32);
                });
            }
            lxapp.with_navbar_mut(&path, |navbar| navbar.set_back_button_visibility(false));
        }

        // 3. Handle page stack modifications
        match nav_type {
            NavigationType::Launch | NavigationType::SwitchTab => {
                lxapp.clear_page_stack()?;
            }
            NavigationType::Replace => {
                lxapp.pop_from_page_stack();
            }
            NavigationType::Forward => {
                if lxapp.is_page_stack_full() {
                    info!("Page stack is full, cannot navigate forward.");
                    return Ok(target_page);
                }
            }
            NavigationType::Backward => {
                return Err(LxAppError::UnsupportedOperation(
                    "should use navigate_back".to_string(),
                ));
            }
        }

        lxapp.push_to_page_stack(&path)?;

        // Set navbar state AFTER page creation to avoid being overwritten
        if nav_type == NavigationType::Forward {
            target_page.get_navbar_state_mut(|navbar| navbar.set_back_button_visibility(true));
        }

        // 5. Dispatch lifecycle events for current and target pages
        if nav_type == NavigationType::Replace {
            self.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
        } else {
            self.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
        }

        // IMPORTANT: Manual onLoad trigger is intentional.
        // Rationale:
        // - The WebView (via LXPort ready) can trigger onLoad for the first-time creation path.
        // - For navigateTo with new query while reusing the same WebView, the WebView itself
        //   will NOT re-emit a bridge-ready event; therefore we deliberately trigger onLoad
        //   here for the existing instance so the page receives the new query in onLoad.
        // Filtering & correctness:
        // - For the very first navigation (render not started), we do NOT trigger onLoad here;
        //   the WebView-side handshake will deliver onLoad at the correct time.
        // - For subsequent navigations (already rendered), we trigger onLoad here and rely on
        //   dispatch_lifecycle_event to enforce the correct ordering (onLoad -> onReady -> onShow)
        //   for this logical navigation instance.
        if target_page.has_render_started() {
            target_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);
        }

        // 6. Perform the native navigation
        (*lxapp.runtime)
            .navigate(self.appid(), path, nav_type.to_animation())
            .map_err(LxAppError::from)?;

        // Do not dispatch OnReady here. WebViewDelegate::on_page_finished() will do it.

        Ok(target_page)
    }

    pub fn navigate_back(&self, delta: u32) -> Result<(), LxAppError> {
        let lxapp = lxapp::get(self.appid());
        let stack_size = lxapp.get_page_stack_size();

        // Ensure at least one page remains
        if stack_size <= 1 {
            return Ok(());
        }

        let mut pages_to_pop = delta;
        // Prevent popping all pages
        if pages_to_pop as usize >= stack_size {
            pages_to_pop = (stack_size - 1) as u32;
        }

        if pages_to_pop == 0 {
            return Ok(());
        }

        for _ in 0..pages_to_pop {
            if let Some(path) = lxapp.pop_from_page_stack() {
                if let Some(page) = lxapp.get_page(path.as_str()) {
                    page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
                }
            }
        }

        if let Some(path) = lxapp.peek_current_page() {
            // Update UI for the destination page
            // Check if destination is a tabbar page without holding any locks
            let is_tabbar_page = lxapp
                .get_tabbar()
                .map_or(false, |tabbar| tabbar.is_tabbar_page(&path));
            lxapp.with_tabbar_mut(|t| t.set_visible(is_tabbar_page));

            // Update NavBar back button visibility based on the new stack size
            let new_stack_size = lxapp.get_page_stack_size();
            lxapp.with_navbar_mut(&path, |navbar| {
                navbar.set_back_button_visibility(new_stack_size > 1);
            });

            (*lxapp.runtime).navigate(
                self.appid(),
                path,
                NavigationType::Backward.to_animation(),
            )?;
            Ok(())
        } else {
            Err(LxAppError::UnsupportedOperation(
                "Page stack is empty after pop".to_string(),
            ))
        }
    }

    pub(crate) fn set_query(&self, query_str: String) {
        if let Ok(query_value) = parse_query_string(&query_str) {
            self.inner.state.lock().unwrap().query = query_value;
        }
    }

    /// Call a JavaScript function in the page's logic service
    ///
    /// # Arguments
    /// * `name` - Function name to call
    /// * `arg` - JSON string containing function arguments
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(LxAppError)` if execution fails
    pub fn call_js(&self, name: String, arg: String) -> Result<(), LxAppError> {
        let lxapp = lxapp::get(self.appid());
        lxapp
            .executor
            .call_page_service(self.appid(), self.path(), name, Some(arg))
    }
}

impl WebViewDelegate for Page {
    /// Called when the page starts loading
    fn on_page_started(&self) {
        self.set_render_status(PageRenderStatus::Started);
    }

    /// Called when the page finishes loading
    fn on_page_finished(&self) {
        self.set_render_status(PageRenderStatus::Finished);
        self.dispatch_lifecycle_event(PageLifecycleEvent::OnReady);
    }

    /// Called when scroll position changes
    fn on_page_scroll_changed(
        &self,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    ) {
        // Safe division to avoid division by zero
        let scroll_percent_x = if max_scroll_x > 0 {
            (scroll_x as f64 / max_scroll_x as f64 * 100.0) as i32
        } else {
            0
        };

        let scroll_percent_y = if max_scroll_y > 0 {
            (scroll_y as f64 / max_scroll_y as f64 * 100.0) as i32
        } else {
            0
        };

        info!(
            "Scroll: x={}/{} ({}%), y={}/{} ({}%)",
            scroll_x, max_scroll_x, scroll_percent_x, scroll_y, max_scroll_y, scroll_percent_y
        );
    }

    /// Handles a postMessage from the WebView
    fn handle_post_message(&self, msg: String) {
        // Parse the message and forward to executor safely
        match IncomingMessage::from_json_str(&msg) {
            Ok(incoming) => {
                let lxapp = lxapp::get(self.inner.appid.clone());
                if let Err(e) = lxapp.executor.handle_view_message(
                    self.inner.appid.clone(),
                    self.inner.path.clone(),
                    Arc::new(incoming),
                ) {
                    error!("Failed to handle view message: {}", e)
                        .with_appid(self.inner.appid.clone());
                }
            }
            Err(e) => {
                error!("Invalid postMessage JSON: {}", e)
                    .with_appid(self.inner.appid.clone())
                    .with_path(self.inner.path.clone());
            }
        }
    }

    /// Handles an HTTP request from the WebView
    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<WebResourceResponse> {
        // Get LxApp and delegate to its request handler
        let lxapp = lxapp::get(self.inner.appid.clone());

        // Use the LxApp's request handling logic
        let uri = req.uri();
        let scheme = uri.scheme_str().unwrap_or("");

        // Use pattern matching for different URI schemes
        match scheme {
            // HTTPS requests - check domain whitelist and static resource types
            "https" => lxapp.https_handler(req),

            // Lingxia scheme for internal app assets
            "lx" => lxapp.lingxia_handler(self, req),

            // Reject all other schemes with 400 Bad Request
            _ => Some(lxapp.create_error_response(
                StatusCode::BAD_REQUEST,
                "Unsupported Scheme",
                &format!("Unsupported scheme: {}", scheme),
            )),
        }
    }

    /// Receive log from WebView
    fn log(&self, level: LogLevel, message: &str) {
        // Convert lingxia_webview::LogLevel to crate::log::LogLevel
        let log_level = match level {
            LogLevel::Error => crate::log::LogLevel::Error,
            LogLevel::Warn => crate::log::LogLevel::Warn,
            LogLevel::Info => crate::log::LogLevel::Info,
            LogLevel::Debug => crate::log::LogLevel::Debug,
            LogLevel::Verbose => crate::log::LogLevel::Debug, // Map Verbose to Debug
        };

        crate::log::LogBuilder::new(crate::log::LogTag::WebViewConsole, message)
            .with_level(log_level)
            .with_path(&self.inner.path)
            .with_appid(self.inner.appid.clone());
    }
}

impl Drop for PageInner {
    fn drop(&mut self) {
        // Terminate page service
        let lxapp = crate::lxapp::get(self.appid.clone());
        if let Err(e) = lxapp
            .executor
            .terminate_page_svc(self.appid.clone(), self.path.clone())
        {
            error!("Failed to terminate page service during drop: {}", e)
                .with_appid(self.appid.clone())
                .with_path(self.path.clone());
        }

        // Destroy WebView if it exists
        if let Ok(mut webview) = self.webview.lock() {
            if let Some(_webview_controller) = webview.take() {
                // WebView will be automatically destroyed when controller is dropped
                info!("WebView destroyed for page")
                    .with_appid(self.appid.clone())
                    .with_path(self.path.clone());
            }
        }
    }
}
