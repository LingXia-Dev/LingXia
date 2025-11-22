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
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{oneshot, watch};

type WebviewReadyReceiver = Arc<Mutex<watch::Receiver<Option<Result<(), String>>>>>;

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

    // notify when WebView wiring is ready (delegate set & setup ran)
    webview_ready_tx: watch::Sender<Option<Result<(), String>>>,
    webview_ready_rx: WebviewReadyReceiver,
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
    pub(crate) fn new<F, Fut>(appid: String, path: String, lxapp: &LxApp, setup_callback: F) -> Self
    where
        F: Fn(&Page) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        // Build page state from LxApp configuration
        let page_state = Self::build_page_state(lxapp, &path);
        let (ready_tx, ready_rx) = watch::channel(None);
        let inner = Arc::new(PageInner {
            appid: appid.clone(),
            path: path.clone(),
            last_active_time: Arc::new(Mutex::new(Instant::now())),
            state: Arc::new(Mutex::new(page_state)),
            webview: Arc::new(Mutex::new(None)),
            webview_ready_tx: ready_tx,
            webview_ready_rx: Arc::new(Mutex::new(ready_rx)),
        });

        let page = Self { inner };

        // Initiate WebView creation asynchronously
        let webtag = WebTag::new(&appid, &path, Some(lxapp.session.id));
        let (ready_tx, ready_rx) = oneshot::channel();
        create_webview(&webtag, ready_tx);

        // Spawn task to wait for WebView creation completion
        // Keep a strong reference to ensure page stays alive during WebView creation
        let page_for_task = page.clone();
        let appid_clone = appid.clone();
        let path_clone = path.clone();

        if let Err(e) = service_executor::spawn_async(async move {
            match ready_rx.await {
                Ok(Ok(webview_controller)) => {
                    // First attach WebView to page
                    page_for_task.attach_webview(webview_controller.clone());

                    // Then set the page as the WebView delegate
                    // Create a new Arc to avoid potential circular references
                    webview_controller.set_delegate(Arc::new(page_for_task.clone()));

                    // Call setup callback - let external code handle the rest
                    let result = setup_callback(&page_for_task).await;

                    // Mark ready after setup completes so waiters are released only once page is usable.
                    page_for_task.mark_webview_ready(result);
                }
                Ok(Err(e)) => {
                    error!("Failed to create WebView: {}", e)
                        .with_appid(appid_clone)
                        .with_path(path_clone);
                    page_for_task.mark_webview_ready(Err(e.to_string()));
                }
                Err(e) => {
                    error!("WebView ready signal failed: {}", e)
                        .with_appid(appid_clone)
                        .with_path(path_clone);
                    page_for_task.mark_webview_ready(Err(e.to_string()));
                }
            }
        }) {
            error!("Failed to spawn async task for WebView creation: {}", e)
                .with_appid(appid.clone())
                .with_path(path.clone());
            page.mark_webview_ready(Err(e));
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

                // Guard: only honor a manual OnLoad after render has started.
                // Bridge-ready path typically arrives after on_page_started, so it passes.
                if event == PageLifecycleEvent::OnLoad
                    && matches!(state.render_status, PageRenderStatus::Unstarted)
                {
                    // Ignore early OnLoad until WebView render actually begins.
                    // Caller can invoke again later; bridge-ready path will also dispatch.
                    return;
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
                    lxapp.clone(),
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

    fn mark_webview_ready(&self, result: Result<(), String>) {
        // Ignore errors; receiver will handle missing updates.
        let _ = self.inner.webview_ready_tx.send(Some(result));
    }

    pub async fn wait_webview_ready(&self) -> Result<(), String> {
        let rx = {
            // Clone receiver so concurrent waiters don't block each other.
            self.inner
                .webview_ready_rx
                .lock()
                .map(|r| r.clone())
                .map_err(|_| "webview ready receiver poisoned".to_string())?
        };

        // Fast-path: already has a value.
        if let Some(res) = rx.borrow().clone() {
            return res;
        }

        let mut rx = rx;
        while rx.changed().await.is_ok() {
            if let Some(res) = rx.borrow().clone() {
                return res;
            }
        }

        Err("webview ready channel closed before result".to_string())
    }

    /// Detach and drop the WebView held by this page.
    /// This breaks Page -> WebView strong reference and triggers platform Drop when
    /// combined with registry removal.
    pub(crate) fn detach_webview(&self) {
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            if let Some(wv) = webview_guard.as_ref() {
                // Break potential cycle by removing delegate first
                wv.remove_delegate();
            }
            // Drop the Arc by taking it out
            let _ = webview_guard.take();
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
                    t.set_selected_index(index);
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

        // Request onLoad for the target page; dispatch_lifecycle_event will gate:
        // - If first-time render hasn't started yet, the early OnLoad is ignored; bridge-ready path will dispatch.
        // - If the WebView has rendered before (re-navigation), OnLoad will be accepted and ordered correctly.
        target_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);

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
            if let Some(path) = lxapp.pop_from_page_stack()
                && let Some(page) = lxapp.get_page(path.as_str())
            {
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
            }
        }

        if let Some(path) = lxapp.peek_current_page() {
            // Update UI for the destination page
            // Check if destination is a tabbar page without holding any locks
            let is_tabbar_page = lxapp
                .get_tabbar()
                .is_some_and(|tabbar| tabbar.is_tabbar_page(&path));
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
            .call_page_service(lxapp.clone(), self.path(), name, Some(arg))
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
                    lxapp.clone(),
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
        // Destroy WebView if it exists
        if let Ok(mut webview) = self.webview.lock() {
            if let Some(wv) = webview.as_ref() {
                // Break potential cycle by removing delegate first
                wv.remove_delegate();
            }
            if let Some(_webview_controller) = webview.take() {
                // WebView will be automatically destroyed when controller is dropped
                info!("WebView destroyed for page")
                    .with_appid(self.appid.clone())
                    .with_path(self.path.clone());
            }
        }
    }
}
