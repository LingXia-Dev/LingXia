use crate::executor::LxAppExecutor;
use crate::lxapp::navbar::NavigationBarState;
use crate::{AppRuntime, LxAppError, error};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A page stack represents a group of pages starting with a tab page
struct PageStack {
    pages: VecDeque<String>,
}

impl PageStack {
    fn new() -> Self {
        Self {
            pages: VecDeque::new(),
        }
    }

    fn push_page(&mut self, path: String) {
        // Avoid pushing duplicates if already the last page
        if self.pages.back() != Some(&path) {
            self.pages.push_back(path);
        }
    }

    fn pop_page(&mut self) -> Option<String> {
        if self.pages.len() > 1 {
            self.pages.pop_back()
        } else {
            None // Preserve at least one page in stack if it exists
        }
    }

    fn current_page(&self) -> Option<&str> {
        self.pages.back().map(|s| s.as_str())
    }

    fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

/// Interface for controlling WebView
pub trait WebViewController: Send + Sync {
    /// Load a URL in the WebView
    fn load_url(&self, url: String) -> Result<(), LxAppError>;

    /// Load HTML data into the WebView
    ///
    /// # Arguments
    /// * `data` - The HTML content to load
    /// * `base_url` - Base URL for resolving relative paths in the HTML
    /// * `history_url` - Optional URL to use for history (defaults to base_url if None)
    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), LxAppError>;

    /// Evaluate JavaScript in the WebView
    fn evaluate_javascript(&self, js: String) -> Result<(), LxAppError>;

    /// Post a message to the JavaScript context
    fn post_message(&self, message: String) -> Result<(), LxAppError> {
        // Escape the JSON message for safe JavaScript injection
        // Since message is already JSON, we need to escape it properly for JS string literal
        let escaped_message = message
            .replace('\\', "\\\\") // Escape backslashes first
            .replace('"', "\\\"") // Escape double quotes
            .replace('\n', "\\n") // Escape newlines
            .replace('\r', "\\r") // Escape carriage returns
            .replace('\t', "\\t"); // Escape tabs

        // Call the global receiver function defined in webview-bridge.js
        let js_code = format!(
            "if (typeof window.__LingXiaRecvMessage === 'function') {{ \
                window.__LingXiaRecvMessage(\"{}\"); \
            }} else {{ \
                console.warn('[LingXia] __LingXiaRecvMessage not available'); \
            }}",
            escaped_message
        );

        // Use evaluateJavaScript to send the message to the WebView
        self.evaluate_javascript(js_code)
    }

    /// Clear browsing data from the WebView
    fn clear_browsing_data(&self) -> Result<(), LxAppError>;

    /// Set the user agent string for the WebView
    fn set_user_agent(&self, ua: String) -> Result<(), LxAppError>;

    /// Enable or disable scroll event listener with optional throttle time
    /// When enabled, scroll events will be sent to the native layer
    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), LxAppError>;
}

/// Manages a collection of pages for a single lxapp
pub(crate) struct Pages {
    /// Map of path to Page
    pages: HashMap<String, Page>,
    /// Tab stacks (number of stacks matches number of tabs, or 1 for non-tabbar apps)
    stacks: Vec<PageStack>,
    /// Index of the currently active tab stack
    current_index: usize,
    /// Maximum number of pages to keep in memory
    max_pages: usize,
}

impl Pages {
    pub(crate) fn new() -> Self {
        Self {
            pages: HashMap::new(),
            stacks: vec![PageStack::new()], // Default: one stack for non-tabbar apps
            current_index: 0,
            max_pages: 5,
        }
    }

    /// Get a reference to a page by path
    pub fn get_page(&self, path: &str) -> Option<&Page> {
        self.pages.get(path)
    }

    /// Ensure page stacks match TabBar configuration
    /// Should be called when TabBar is initialized or changed
    pub fn ensure_stacks_for_tabbar(&mut self, tabbar: Option<&crate::lxapp::tabbar::TabBar>) {
        let tab_count = tabbar.map(|tb| tb.list.len()).unwrap_or(1);

        // Adjust stack count to match tabs (or 1 for non-tabbar apps)
        if self.stacks.len() != tab_count {
            self.stacks.clear();
            for _ in 0..tab_count {
                self.stacks.push(PageStack::new());
            }
            self.current_index = 0;
        }
    }

    /// Get tab paths from TabBar
    fn get_tab_paths(&self, tabbar: Option<&crate::lxapp::tabbar::TabBar>) -> Vec<String> {
        tabbar
            .map(|tb| tb.list.iter().map(|item| item.pagePath.clone()).collect())
            .unwrap_or_default()
    }

    /// Creates a new page placeholder and initiates WebView creation asynchronously
    /// Returns the page immediately, WebView will be attached when ready
    pub fn create_page<F>(
        &mut self,
        appid: String,
        path: String,
        page_state: PageState,
        controller: Arc<dyn AppRuntime>,
        executor: Arc<LxAppExecutor>,
        setup_callback: F,
    ) -> Result<Page, LxAppError>
    where
        F: Fn(&Page, &str) + Send + 'static,
    {
        if self.pages.len() >= self.max_pages {
            self.destroy_least_active();
        }

        // Create page without WebView first
        let page = Page::new(appid.clone(), path.clone(), page_state, executor.clone());

        // Insert the page into the hashmap
        self.pages.insert(path.clone(), page.clone());

        // Create channel for WebView creation notification
        let (sender, receiver) = std::sync::mpsc::channel();

        // Initiate WebView creation asynchronously
        controller.create_webview(appid.clone(), path.clone(), sender);

        // Spawn task to wait for WebView creation completion
        let page_clone = page.clone();
        let appid_clone = appid.clone();
        let path_clone = path.clone();

        LxAppExecutor::spawn_task(move || {
            match receiver.recv() {
                Ok(Ok(webview_controller)) => {
                    // Attach WebView to page
                    page_clone.attach_webview(webview_controller);

                    // Call setup callback - let external code handle the rest
                    setup_callback(&page_clone, &path_clone);
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

        Ok(page)
    }

    /// Navigates to a page by updating the current stack and marking the page as active
    /// Returns the previous page path if there was a page switch that should trigger onHide
    pub fn navigate_to_page(
        &mut self,
        path: String,
        tabbar: Option<&crate::lxapp::tabbar::TabBar>,
    ) -> Option<String> {
        // Ensure stacks match TabBar configuration
        self.ensure_stacks_for_tabbar(tabbar);

        // Get the current page before navigation
        let previous_page = self.stacks[self.current_index]
            .current_page()
            .map(String::from);

        // Handle tab page navigation
        let tab_paths = self.get_tab_paths(tabbar);
        if tabbar.is_some() && tab_paths.contains(&path) {
            if let Some(index) = tab_paths.iter().position(|p| p == &path) {
                self.current_index = index;

                // If this stack is empty, add the tab page as its first page
                if self.stacks[index].is_empty() {
                    self.stacks[index].push_page(path.clone());
                }
            }
        } else {
            // Non-tab page or no tabbar - add to current stack
            let stack = &mut self.stacks[self.current_index];
            stack.push_page(path.clone());
        }

        // Update the last active time for this page
        if let Some(page) = self.pages.get(&path) {
            page.mark_active();
        }

        // Return previous page if it's different from the new page
        if let Some(prev_path) = previous_page {
            if prev_path != path {
                return Some(prev_path);
            }
        }

        None
    }

    /// Pops the current page from the current stack if possible.
    /// Returns the path of the page to navigate back *to* if successful.
    /// Also destroys the page that was popped.
    /// Returns None if the current page cannot be popped (e.g., it's the tab root).
    pub fn pop_from_current_stack(
        &mut self,
        tabbar: Option<&crate::lxapp::tabbar::TabBar>,
    ) -> Option<String> {
        // Make sure we have stacks and current stack isn't empty
        if self.stacks.is_empty() || self.stacks[self.current_index].is_empty() {
            return None;
        }

        // Get the current page before popping
        let current_page = match self.stacks[self.current_index].current_page() {
            Some(p) => p.to_string(),
            None => return None, // Stack is empty? Error state.
        };

        // Check if the current page is a tab page (if we have tabbar)
        let tab_paths = self.get_tab_paths(tabbar);
        if tabbar.is_some() && tab_paths.contains(&current_page) {
            return None; // Cannot pop a tab root page
        }

        // For any app, cannot pop if stack has only one page
        if self.stacks[self.current_index].pages.len() <= 1 {
            return None;
        }

        // Attempt to pop from the stack structure
        if let Some(popped_page) = self.stacks[self.current_index].pop_page() {
            // Successfully popped from the stack structure.
            // Now remove the page and terminate its services
            if let Some(page) = self.pages.remove(&popped_page) {
                let _ = page.terminate_page_service();
            }

            // Return the path of the *new* current page in the stack
            self.stacks[self.current_index]
                .current_page()
                .map(String::from)
        } else {
            // pop_page failed (e.g., already at root, though we checked)
            None
        }
    }

    /// Destroys the least recently used page to maintain memory limits
    fn destroy_least_active(&mut self) {
        // Check if we have multiple stacks (indicating TabBar app)
        if self.stacks.len() > 1 {
            // Try to remove pages from non-current stacks first
            for i in 0..self.stacks.len() {
                if i == self.current_index {
                    continue; // Skip current stack
                }

                let stack = &mut self.stacks[i];
                if stack.pages.len() > 1 {
                    // If this stack has more than one page (besides the root page)
                    // remove the most recently pushed page (top of stack)
                    if let Some(last_page) = stack.pop_page() {
                        if let Some(page) = self.pages.remove(&last_page) {
                            let _ = page.terminate_page_service();
                        }
                        return;
                    }
                }
            }

            // If no pages can be removed from non-current stacks, try the current stack
            // but preserve the root page and at least one direct child page
            let current_stack = &mut self.stacks[self.current_index];
            if current_stack.pages.len() > 2 {
                // Keep root page and at least one child page
                // Remove the most recently pushed page (top of stack)
                if let Some(last_page) = current_stack.pop_page() {
                    if let Some(page) = self.pages.remove(&last_page) {
                        let _ = page.terminate_page_service();
                    }
                }
            }
        } else {
            // For apps without tabbar - only one stack
            // Ensure we keep at least one page
            if self.stacks[0].pages.len() > 1 {
                // Remove the most recently pushed page (top of stack)
                if let Some(last_page) = self.stacks[0].pop_page() {
                    if let Some(page) = self.pages.remove(&last_page) {
                        let _ = page.terminate_page_service();
                    }
                }
            }
        }
    }
}

/// Inner state of a page that can be shared across threads
#[derive(Clone)]
pub(crate) struct PageInner {
    appid: String,
    path: String,

    // Reference to the WebView controller (optional, set when WebView is ready)
    webview_controller: Arc<Mutex<Option<Arc<dyn WebViewController>>>>,
    // Reference to the executor
    executor: Arc<LxAppExecutor>,

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
    pub navbar_state: NavigationBarState,
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
    pub(crate) fn build_page_state(lxapp: &crate::lxapp::LxApp, path: &str) -> PageState {
        PageState {
            load_state: PageLoadState::Pending,
            navbar_state: NavigationBarState::from_json(lxapp, path),
        }
    }

    /// Create a new page in pending state (WebView creation in progress)
    fn new(
        appid: String,
        path: String,
        page_state: PageState,
        executor: Arc<LxAppExecutor>,
    ) -> Self {
        let inner = Arc::new(PageInner {
            appid,
            path,
            executor,
            last_active_time: Arc::new(Mutex::new(Instant::now())),
            state: Arc::new(Mutex::new(page_state)),
            webview_controller: Arc::new(Mutex::new(None)),
        });

        Self { inner }
    }

    /// Attach WebView controller to this page (called when WebView is ready)
    fn attach_webview(&self, webview_controller: Arc<dyn WebViewController>) {
        if let Ok(mut controller_guard) = self.inner.webview_controller.lock() {
            *controller_guard = Some(webview_controller);
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

    /// Get the WebView controller for this page (returns None if not ready)
    pub(crate) fn webview_controller(&self) -> Option<Arc<dyn WebViewController>> {
        self.inner
            .webview_controller
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Load HTML content into this page's WebView
    ///
    /// # Arguments
    /// * `html_data` - The HTML content to load
    /// * `base_url` - Base URL for resolving relative paths in the HTML
    pub(crate) fn load_html(&self, html_data: String, base_url: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.webview_controller() {
            controller.load_data(html_data, base_url, None)
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
    fn mark_active(&self) {
        if let Ok(mut time) = self.inner.last_active_time.lock() {
            *time = Instant::now();
        }
    }

    fn terminate_page_service(&self) -> Result<(), LxAppError> {
        self.inner
            .executor
            .terminate_page_svc(self.inner.appid.clone(), self.inner.path.clone())?;
        Ok(())
    }
}
