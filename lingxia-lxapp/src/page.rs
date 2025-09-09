use crate::appservice::bridge::IncomingMessage;
use crate::executor::LxAppExecutor;
use crate::lxapp::{self, navbar::NavigationBarState};
use crate::{LxApp, LxAppError, error, info};
use lingxia_platform::{AppRuntime, NavigationType};
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
    // Page loading state
    pub(crate) load_state: PageLoadState,
    // Navigation bar state
    pub(crate) navbar_state: NavigationBarState,
    // Query parameters
    pub(crate) query: serde_json::Value,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PageLoadState {
    Pending, // Page created, WebView creation in progress
    Created, // Page created and WebView attached, but no HTML loaded
    Loading, // HTML loading into page
    Loaded,  // HTML loaded into page
    Unknown, // Unknown state
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
            load_state: PageLoadState::Pending,
            navbar_state: NavigationBarState::from_json(lxapp, path),
            query: serde_json::Value::Null,
        }
    }

    /// Create a new page in pending state (WebView creation in progress)
    pub(crate) fn new<F>(appid: String, path: String, lxapp: &LxApp, setup_callback: F) -> Self
    where
        F: Fn(&Page, &str) + Send + 'static,
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
                    setup_callback(&page_for_task, &path_clone);
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
            // Update load state to Created when WebView is attached
            self.set_load_state(PageLoadState::Created);
        }
    }

    /// Get complete page state
    pub fn get_page_state(&self) -> Option<PageState> {
        self.inner.state.lock().ok().map(|state| state.clone())
    }

    /// Get load state
    pub fn get_load_state(&self) -> PageLoadState {
        self.inner
            .state
            .lock()
            .map(|state| state.load_state)
            .unwrap_or(PageLoadState::Unknown)
    }

    /// Set load state
    pub fn set_load_state(&self, load_state: PageLoadState) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.load_state = load_state;
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
    ///
    /// # Arguments
    /// * `html_data` - The HTML content to load
    /// * `base_url` - Base URL for resolving relative paths in the HTML
    pub(crate) fn load_html(&self, html_data: String, base_url: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.webview_controller() {
            controller
                .load_data(html_data, base_url, None)
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
        let lxapp = crate::lxapp::get(self.inner.appid.clone());
        lxapp.config.is_tab_page(&self.inner.path)
    }

    pub fn navigate(&self, url: &str, nav_type: NavigationType) -> Result<(), LxAppError> {
        let lxapp = lxapp::get(self.appid());

        let (path, query) = if nav_type == NavigationType::SwitchTab {
            (url.to_string(), serde_json::Value::Null)
        } else {
            let (p, q_str) = if let Some(idx) = url.find('?') {
                (url[..idx].to_string(), &url[idx + 1..])
            } else {
                (url.to_string(), "")
            };
            let q = serde_json::from_str(q_str)
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
            (p, q)
        };

        match nav_type {
            NavigationType::Launch => {
                lxapp.clear_page_stack()?;
            }
            NavigationType::SwitchTab => {
                lxapp.clear_page_stack()?;
                lxapp.with_tabbar_mut(|t| t.set_visible(true));
            }
            NavigationType::Replace => {
                lxapp.pop_from_page_stack();
            }
            NavigationType::Forward => {
                if lxapp.is_page_stack_full() {
                    info!("Page stack is full, cannot navigate forward.");
                    return Ok(());
                }
            }
            _ => {}
        }

        if let Some(page) = lxapp.get_or_create_page(&path) {
            if query != serde_json::Value::Null {
                page.set_query(query);
            }
        }

        (*lxapp.runtime)
            .navigate(self.appid(), path, nav_type)
            .map_err(Into::into)
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
            lxapp.pop_from_page_stack();
        }

        if let Some(path) = lxapp.peek_current_page() {
            (*lxapp.runtime).navigate(self.appid(), path, NavigationType::Backward)?;
            Ok(())
        } else {
            Err(LxAppError::UnsupportedOperation(
                "Page stack is empty after pop".to_string(),
            ))
        }
    }

    fn get_query(&self) -> serde_json::Value {
        self.inner.state.lock().unwrap().query.clone()
    }

    pub(crate) fn set_query(&self, query: serde_json::Value) {
        self.inner.state.lock().unwrap().query = query;
    }
}

impl WebViewDelegate for Page {
    /// Called when the page starts loading
    fn on_page_started(&self) {
        let query_str = serde_json::to_string(&self.get_query()).ok();

        // Get LxApp and call page service
        let lxapp = lxapp::get(self.inner.appid.clone());
        let _ = lxapp.executor.call_page_service(
            self.inner.appid.clone(),
            self.inner.path.clone(),
            "onLoad".to_string(),
            query_str,
        );
    }

    /// Called when the page finishes loading
    fn on_page_finished(&self) {
        // Get LxApp and call page service
        let lxapp = lxapp::get(self.inner.appid.clone());
        let _ = lxapp.executor.call_page_service(
            self.inner.appid.clone(),
            self.inner.path.clone(),
            "onReady".to_string(),
            None,
        );

        // Update page load state
        self.set_load_state(PageLoadState::Loaded);
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
