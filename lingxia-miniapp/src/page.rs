use crate::{AppController, ControllerCmd, MiniAppError, WebViewCmd};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, mpsc};
use std::time::Instant;

/// A page stack represents a group of pages starting with a tab page
struct PageStack {
    pages: VecDeque<String>, // First page is always the tab page
}

impl PageStack {
    fn new(tab_page: String) -> Self {
        let mut pages = VecDeque::new();
        pages.push_back(tab_page);
        Self { pages }
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
            None // Never pop the tab page
        }
    }

    fn current_page(&self) -> Option<&str> {
        self.pages.back().map(|s| s.as_str())
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
pub struct Pages {
    /// Map of path to Page
    pages: HashMap<String, Page>,
    /// Tab path to PageStack mapping
    stacks: HashMap<String, PageStack>,
    /// Path of the currently active tab
    current_tab: Option<String>,
    /// Maximum number of pages to keep in memory
    max_pages: usize,
    /// Whether the app has a tabbar, if not, only one pagestack is used
    has_tabbar: bool,
}

impl Pages {
    pub(crate) fn new(max_pages: Option<usize>, has_tabbar: bool) -> Self {
        Self {
            pages: HashMap::new(),
            stacks: HashMap::new(),
            current_tab: None,
            max_pages: max_pages.unwrap_or(5),
            has_tabbar,
        }
    }

    /// Finds a page by path
    pub(crate) fn find_page(&self, path: &str) -> Option<&Page> {
        self.pages.get(path)
    }

    /// Gets or creates the root page stack
    /// In case of no tabbar, uses a fixed "root" as the key for the root page stack
    fn get_or_create_root_stack(&mut self, path: String) -> &mut PageStack {
        if !self.has_tabbar {
            // For apps without tabbar, use a fixed "root" key for the single pagestack
            let key = "root".to_string();
            let stack = self
                .stacks
                .entry(key.clone())
                .or_insert_with(|| PageStack::new(path.clone()));
            if self.current_tab.is_none() {
                self.current_tab = Some(key);
            }
            stack
        } else {
            // For apps with tabbar, use the path as the key for the page stack
            self.stacks
                .entry(path.clone())
                .or_insert_with(|| PageStack::new(path.clone()))
        }
    }

    /// Pushes a new page
    pub(crate) fn push_page(
        &mut self,
        appid: String,
        path: String,
        is_tab_page: bool,
        controller: Arc<dyn AppController>,
    ) {
        if self.pages.len() >= self.max_pages {
            self.destroy_least_active();
        }

        if self.has_tabbar {
            // For apps with tabbar
            if is_tab_page {
                // Create new tabbar page stack or get existing one
                let stack = self
                    .stacks
                    .entry(path.clone())
                    .or_insert_with(|| PageStack::new(path.clone()));
                // Ensure path is in the stack (might be creating or switching)
                stack.push_page(path.clone());
                self.current_tab = Some(path.clone());
            } else if let Some(current_tab) = &self.current_tab {
                // Add to current tab's stack
                if let Some(stack) = self.stacks.get_mut(current_tab) {
                    stack.push_page(path.clone());
                }
            }
        } else {
            // For apps without tabbar, all pages are in a single stack
            let root_key = "root".to_string();

            // Ensure root stack is created
            if !self.stacks.contains_key(&root_key) {
                self.stacks
                    .insert(root_key.clone(), PageStack::new(path.clone()));
                self.current_tab = Some(root_key.clone());
            }

            // Add page to the root stack
            if let Some(stack) = self.stacks.get_mut(&root_key) {
                stack.push_page(path.clone());
            }
        }

        // Create and insert new page
        let page = Page::new(controller, appid, path.clone());
        self.pages.insert(path, page);
    }

    /// Updates the last active time for a page
    pub(crate) fn mark_active(&mut self, path: &str) {
        if let Some(page) = self.pages.get_mut(path) {
            page.last_active_time = Instant::now();
        }
    }

    /// Sets the current active tab
    /// This method should be called when the user switches tabs
    pub(crate) fn set_current_tab(&mut self, tab_path: &str) -> Result<(), MiniAppError> {
        // Ensure this is a valid tab path
        if self.has_tabbar {
            if !self.stacks.contains_key(tab_path) {
                return Err(MiniAppError::InvalidParameter(format!(
                    "Tab '{}'",
                    tab_path
                )));
            }
            self.current_tab = Some(tab_path.to_string());
            Ok(())
        } else {
            // For non-tabbar applications
            Err(MiniAppError::UnsupportedOperation(
                "Setting tab in non-tabbar app".to_string(),
            ))
        }
    }

    /// Pops the current page from the current tab's stack if possible.
    /// Returns the path of the page to navigate back *to* if successful.
    /// Also destroys the page that was popped.
    /// Returns None if the current page cannot be popped (e.g., it's the tab root).
    pub fn pop_from_current_stack(&mut self) -> Option<String> {
        let current_tab_path = match &self.current_tab {
            Some(tab) => tab,
            None => return None, // No current tab to pop from
        };

        // Get the current page before mutably borrowing the stack
        let current_page = {
            let stack = self.stacks.get(current_tab_path)?;
            match stack.current_page() {
                Some(p) => p.to_string(),
                None => return None, // Stack is empty? Error state.
            }
        };

        // Check if the current page is a tab page
        if self.has_tabbar && self.stacks.contains_key(&current_page) {
            return None; // Cannot pop a tab root page
        }

        // Now get a mutable reference to the stack
        let stack = self.stacks.get_mut(current_tab_path)?;

        // For apps without tabbar, cannot pop if stack has only one page
        if !self.has_tabbar && stack.pages.len() <= 1 {
            return None;
        }

        // Attempt to pop from the stack structure
        if stack.pop_page().is_some() {
            // Successfully popped from the stack structure.
            // Now, remove the page for the popped path.
            self.pages.remove(&current_page); // Page is dropped here

            // Return the path of the *new* current page in the stack
            stack.current_page().map(String::from)
        } else {
            // pop_page failed (e.g., already at root, though we checked)
            None
        }
    }

    /// Destroys the least recently used page to maintain memory limits
    fn destroy_least_active(&mut self) {
        if self.has_tabbar {
            // For apps with tabbar
            // First try to remove the most recently pushed page from non-current tabs
            if let Some(current_tab) = &self.current_tab {
                // Check all non-current tabs
                for (tab_path, stack) in self.stacks.iter_mut() {
                    if tab_path == current_tab {
                        continue; // Skip current tab
                    }

                    // If this tab has more than one page (besides the root page)
                    // remove the most recently pushed page (top of stack)
                    if stack.pages.len() > 1 {
                        if let Some(last_page) = stack.pop_page() {
                            self.pages.remove(&last_page);
                            return;
                        }
                    }
                }
            }

            // If no pages can be removed from non-current tabs, try the current tab
            // but preserve the root page and at least one direct child page
            if let Some(current_tab) = &self.current_tab {
                if let Some(stack) = self.stacks.get_mut(current_tab) {
                    if stack.pages.len() > 2 {
                        // Keep root page and at least one child page
                        // Remove the most recently pushed page (top of stack)
                        if let Some(last_page) = stack.pop_page() {
                            self.pages.remove(&last_page);
                            return;
                        }
                    }
                }
            }
        } else {
            // For apps without tabbar - only one page stack
            let root_key = "root".to_string();
            if let Some(stack) = self.stacks.get_mut(&root_key) {
                // Ensure we keep at least one page
                if stack.pages.len() > 1 {
                    // Remove the most recently pushed page (top of stack)
                    if let Some(last_page) = stack.pop_page() {
                        self.pages.remove(&last_page);
                        return;
                    }
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
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::LoadUrl {
            appid: self.appid.clone(),
            path: self.path.clone(),
            url: url.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }

    fn evaluate_javascript(&self, js: &str) -> Result<(), MiniAppError> {
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::EvaluateJavascript {
            appid: self.appid.clone(),
            path: self.path.clone(),
            script: js.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }

    fn post_message(&self, message: &str) -> Result<(), MiniAppError> {
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::PostMessage {
            appid: self.appid.clone(),
            path: self.path.clone(),
            message: message.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::SetDevtools {
            appid: self.appid.clone(),
            enabled,
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::ClearBrowsingData {
            appid: self.appid.clone(),
            path: self.path.clone(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }

    fn set_user_agent(&self, ua: &str) -> Result<(), MiniAppError> {
        let (responder, _) = mpsc::channel();

        let cmd = WebViewCmd::SetUserAgent {
            appid: self.appid.clone(),
            ua: ua.to_string(),
            responder,
        };

        self.controller.send_cmd(ControllerCmd::WebView(cmd))
    }
}
