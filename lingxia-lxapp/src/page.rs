use crate::appservice::bridge::IncomingMessage;
use crate::executor::LxAppExecutor;
use crate::lxapp::{self, navbar::NavigationBarState};
use crate::{LxApp, LxAppError, error, info};
use lingxia_platform::{AnimationType, AppRuntime};
use lingxia_webview::{
    LogLevel, WebTag, WebView, WebViewController, WebViewDelegate, create_webview,
};

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

#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) enum PageLifecycleEvent {
    OnLoad,
    OnReady,
    OnShow,
    OnHide,
    OnUnload,
    Unknown,
}

impl From<PageLifecycleEvent> for String {
    fn from(event: PageLifecycleEvent) -> Self {
        match event {
            PageLifecycleEvent::OnLoad => "onLoad".to_string(),
            PageLifecycleEvent::OnReady => "onReady".to_string(),
            PageLifecycleEvent::OnShow => "onShow".to_string(),
            PageLifecycleEvent::OnHide => "onHide".to_string(),
            PageLifecycleEvent::OnUnload => "onUnload".to_string(),
            PageLifecycleEvent::Unknown => "unknown".to_string(),
        }
    }
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

        LxAppExecutor::spawn_task(move || {
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
        });

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
        // This function acts as a state machine to ensure the JS lifecycle events
        // fire in the correct order (onLoad -> onShow -> onReady), regardless of
        // when the underlying events (manual calls, UI visibility, webview delegates) arrive.

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

                // Check for onLoad: Can fire if page has started loading and hasn't been fired.
                if event == PageLifecycleEvent::OnLoad
                    && state.render_status != PageRenderStatus::Unstarted
                {
                    let query = serde_json::to_string(&state.query).ok();
                    events_to_fire.push((PageLifecycleEvent::OnLoad, query));
                    state.on_load_fired = true;
                    state.on_show_fired = false;
                    state.on_ready_fired = false;
                }

                // Check for onShow: Can fire if onLoad has fired, UI has requested show, and it hasn't been fired.
                if state.on_load_fired && state.show_requested && !state.on_show_fired {
                    events_to_fire.push((PageLifecycleEvent::OnShow, None));
                    state.on_show_fired = true;
                    state.event = PageLifecycleEvent::OnShow;
                }

                // Check for onReady: Can fire if onShow has fired, page has finished loading, and it hasn't been fired.
                if state.on_show_fired
                    && state.render_status == PageRenderStatus::Finished
                    && !state.on_ready_fired
                {
                    events_to_fire.push((PageLifecycleEvent::OnReady, None));
                    state.on_ready_fired = true;
                }
            }
        }

        //  Fire the collected events outside of the lock to prevent deadlocks.
        if !events_to_fire.is_empty() {
            let lxapp = lxapp::get(self.inner.appid.clone());
            let appid = self.appid();
            let path = self.path();

            for (event, query) in events_to_fire {
                if let Err(e) = lxapp.executor.call_page_service(
                    appid.clone(),
                    path.clone(),
                    event.into(),
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
        let base_url = format!("lx://{}/{}", self.appid(), path);

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
        lxapp.config.is_tab_page(&self.inner.path)
    }

    pub fn navigate(&self, url: &str, nav_type: NavigationType) -> Result<(), LxAppError> {
        let lxapp = lxapp::get(self.appid());

        // 1. Parse URL to get path and query
        let (path, query) = if nav_type == NavigationType::SwitchTab {
            (url.to_string(), serde_json::Value::Null)
        } else {
            let (p, q_str) = if let Some(idx) = url.find('?') {
                (url[..idx].to_string(), &url[idx + 1..])
            } else {
                (url.to_string(), "")
            };

            let q = if q_str.is_empty() {
                serde_json::Value::Null
            } else {
                let mut query_map = serde_json::Map::new();
                for pair in q_str.split('&') {
                    if let Some(eq_pos) = pair.find('=') {
                        let key = &pair[..eq_pos];
                        let value = &pair[eq_pos + 1..];
                        let decoded_value = urlencoding::decode(value)
                            .unwrap_or_else(|_| std::borrow::Cow::Borrowed(value));
                        query_map.insert(
                            key.to_string(),
                            serde_json::Value::String(decoded_value.to_string()),
                        );
                    } else {
                        query_map
                            .insert(pair.to_string(), serde_json::Value::String("".to_string()));
                    }
                }
                serde_json::Value::Object(query_map)
            };
            (p, q)
        };

        // 2. Handle UI state based on navigation type (TabBar, NavBar)
        let is_tab_switch = nav_type == NavigationType::SwitchTab;
        lxapp.with_tabbar_mut(|t| t.set_visible(is_tab_switch));
        if is_tab_switch {
            if let Some(Some(index)) = lxapp.with_tabbar_mut(|t| t.find_tab_index_by_path(&path)) {
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
                    return Ok(());
                }
                // Special case: if navigating forward to the same page, force push.
                if self.path() == path {
                    lxapp.push_to_page_stack(&path, true)?;
                }
                lxapp.with_navbar_mut(&path, |navbar| navbar.set_back_button_visibility(true));
            }
            NavigationType::Backward => {
                // Backward is handled by navigate_back, so this is a no-op here.
            }
        }

        // 4. Get or create the target page and set its query
        let target_page = lxapp.get_or_create_page(&path).ok_or_else(|| {
            LxAppError::UnsupportedOperation("Failed to get or create page".to_string())
        })?;
        if query != serde_json::Value::Null {
            target_page.set_query(query);
        }

        // 5. Dispatch lifecycle events for current and target pages
        if nav_type == NavigationType::Replace {
            self.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
        } else {
            self.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
        }
        target_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);

        // 6. Perform the native navigation
        (*lxapp.runtime)
            .navigate(self.appid(), path, nav_type.to_animation())
            .map_err(LxAppError::from)?;

        // 7. Dispatch final lifecycle event for the target page
        target_page.dispatch_lifecycle_event(PageLifecycleEvent::OnReady);

        Ok(())
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
            if let Some(dest_page) = lxapp.get_page(&path) {
                // Update TabBar visibility based on whether the destination is a tab page
                lxapp.with_tabbar_mut(|t| t.set_visible(dest_page.is_tabbar_page()));
            }

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

    pub(crate) fn set_query(&self, query: serde_json::Value) {
        self.inner.state.lock().unwrap().query = query;
    }
}

impl WebViewDelegate for Page {
    /// Called when the page starts loading
    fn on_page_started(&self) {
        self.set_render_status(PageRenderStatus::Started);
        self.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);
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
        // Parse the message and forward to executor
        let incoming = IncomingMessage::from_json_str(&msg).unwrap();
        let lxapp = lxapp::get(self.inner.appid.clone());

        if let Err(e) = lxapp.executor.handle_view_message(
            self.inner.appid.clone(),
            self.inner.path.clone(),
            Arc::new(incoming),
        ) {
            error!("Failed to handle view message: {}", e).with_appid(self.inner.appid.clone());
        }
    }

    /// Handles an HTTP request from the WebView
    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>> {
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
            "lx" => lxapp.lingxia_handler(req),

            // Reject all other schemes with 400 Bad Request
            _ => Some(
                http::Response::builder()
                    .status(400)
                    .header("Content-Type", "text/plain")
                    .body(format!("Unsupported scheme: {}", scheme).into_bytes())
                    .unwrap(),
            ),
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

