use crate::{AppController, ControllerCmd, MiniAppError, WebViewCmd};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, mpsc};
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
pub trait WebViewController {
    /// Load a URL in the WebView
    fn load_url(&self, url: &str) -> Result<(), MiniAppError>;

    /// Evaluate JavaScript in the WebView
    fn evaluate_javascript(&self, js: &str) -> Result<(), MiniAppError>;

    /// Post a message to the JavaScript context
    fn post_message(&self, message: &str) -> Result<(), MiniAppError>;

    /// Enable or disable developer tools
    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError>;

    /// Clear browsing data from the WebView
    fn clear_browsing_data(&self) -> Result<(), MiniAppError>;

    /// Set the user agent string for the WebView
    fn set_user_agent(&self, ua: &str) -> Result<(), MiniAppError>;
}

/// Manages a collection of pages for a single miniapp
pub(crate) struct Pages {
    /// Map of path to Page
    pages: HashMap<String, Page>,
    /// Tab stacks in the same order as tab_paths
    stacks: Vec<PageStack>,
    /// Index of the currently active tab stack
    current_index: usize,
    /// Maximum number of pages to keep in memory
    max_pages: usize,
    /// Ordered tab paths for index-based access (empty means no tab bar)
    tab_paths: Vec<String>,
}

impl Pages {
    pub(crate) fn new() -> Self {
        Self {
            pages: HashMap::new(),
            stacks: Vec::new(),
            current_index: 0,
            max_pages: 5,
            tab_paths: Vec::new(),
        }
    }

    /// Set tab bar items with ordered paths and initialize stacks
    ///
    /// # Arguments
    /// * `tab_paths` - Ordered list of tab page paths
    pub fn set_tabbar_items(&mut self, tab_paths: Vec<String>) {
        self.tab_paths = tab_paths;

        // Reset stacks and preallocate for the known number of tabs
        self.stacks = Vec::with_capacity(self.tab_paths.len());

        // If we have no tabs, make sure we have at least one default stack
        if self.tab_paths.is_empty() {
            // For non-tabbar apps, create one default stack
            self.stacks.push(PageStack::new());
        } else {
            // Create stacks for each tab path
            for _ in &self.tab_paths {
                self.stacks.push(PageStack::new());
            }
        }

        // Reset current index to 0
        self.current_index = 0;
    }

    /// Check if the app has a tab bar
    fn has_tabbar(&self) -> bool {
        !self.tab_paths.is_empty()
    }

    /// Creates a new page and adds it to the pages collection
    /// Returns a reference to the newly created page
    pub fn create_page(
        &mut self,
        appid: String,
        path: String,
        controller: Arc<dyn AppController>,
    ) -> &Page {
        if self.pages.len() >= self.max_pages {
            self.destroy_least_active();
        }

        // Create and insert new page
        let page = Page::new(controller, appid, path.clone());
        self.pages.insert(path.clone(), page);

        // Return reference to the newly created page
        self.pages.get(&path).unwrap()
    }

    /// Navigates to a page by updating the current stack and marking the page as active
    pub fn navigate_to_page(&mut self, path: String) {
        // Handle tab page navigation
        if self.has_tabbar() && self.tab_paths.contains(&path) {
            if let Some(index) = self.tab_paths.iter().position(|p| p == &path) {
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
        if let Some(page) = self.pages.get_mut(&path) {
            page.last_active_time = Instant::now();
        }
    }

    /// Pops the current page from the current stack if possible.
    /// Returns the path of the page to navigate back *to* if successful.
    /// Also destroys the page that was popped.
    /// Returns None if the current page cannot be popped (e.g., it's the tab root).
    pub fn pop_from_current_stack(&mut self) -> Option<String> {
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
        if self.has_tabbar() && self.tab_paths.contains(&current_page) {
            return None; // Cannot pop a tab root page
        }

        // For any app, cannot pop if stack has only one page
        if self.stacks[self.current_index].pages.len() <= 1 {
            return None;
        }

        // Attempt to pop from the stack structure
        if let Some(popped_page) = self.stacks[self.current_index].pop_page() {
            // Successfully popped from the stack structure.
            // Now, remove the page for the popped path.
            self.pages.remove(&popped_page);

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
        if self.has_tabbar() {
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
                        self.pages.remove(&last_page);
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
                    self.pages.remove(&last_page);
                }
            }
        } else {
            // For apps without tabbar - only one stack
            // Ensure we keep at least one page
            if self.stacks[0].pages.len() > 1 {
                // Remove the most recently pushed page (top of stack)
                if let Some(last_page) = self.stacks[0].pop_page() {
                    self.pages.remove(&last_page);
                }
            }
        }
    }
}

/// Represents a single page in a mini app
pub(crate) struct Page {
    appid: String,
    path: String,

    // Reference to the app controller
    controller: Arc<dyn AppController>,

    // Time when this page was last active
    last_active_time: Instant,
}

impl Page {
    pub(crate) fn new(controller: Arc<dyn AppController>, appid: String, path: String) -> Self {
        Self {
            controller,
            appid,
            path,
            last_active_time: Instant::now(),
        }
    }
}

impl WebViewController for Page {
    fn load_url(&self, url: &str) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::LoadUrl {
            appid: self.appid.clone(),
            path: self.path.clone(),
            url: url.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView("WebView command 'LoadUrl' failed: channel closed".to_string())
        })?
    }

    fn evaluate_javascript(&self, js: &str) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::EvaluateJavascript {
            appid: self.appid.clone(),
            path: self.path.clone(),
            script: js.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'EvaluateJavascript' failed: channel closed".to_string(),
            )
        })?
    }

    fn post_message(&self, message: &str) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::PostMessage {
            appid: self.appid.clone(),
            path: self.path.clone(),
            message: message.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'PostMessage' failed: channel closed".to_string(),
            )
        })?
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::SetDevtools {
            appid: self.appid.clone(),
            enabled,
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'SetDevtools' failed: channel closed".to_string(),
            )
        })?
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::ClearBrowsingData {
            appid: self.appid.clone(),
            path: self.path.clone(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'ClearBrowsingData' failed: channel closed".to_string(),
            )
        })?
    }

    fn set_user_agent(&self, ua: &str) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();

        let cmd = WebViewCmd::SetUserAgent {
            appid: self.appid.clone(),
            ua: ua.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))?;

        // Wait for the response
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'SetUserAgent' failed: channel closed".to_string(),
            )
        })?
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        // Just send drop webview command without waiting for response
        let _ = self
            .controller
            .send_cmd(ControllerCmd::WebView(WebViewCmd::DropWebView {
                appid: self.appid.clone(),
                path: self.path.clone(),
            }));
    }
}
