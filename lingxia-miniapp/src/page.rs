use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
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
        self.pages.push_back(path);
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

    fn previous_page(&self) -> Option<&str> {
        if self.pages.len() > 1 {
            self.pages.get(self.pages.len() - 2).map(|s| s.as_str())
        } else {
            None
        }
    }
}

pub struct PageManager {
    controllers: HashMap<String, (Arc<dyn PageController>, Instant)>,
    stacks: HashMap<String, PageStack>, // tab_path -> PageStack
    current_tab: Option<String>,        // Current active tab path
    max_pages: usize,
}

impl PageManager {
    pub(crate) fn new(max_pages: Option<usize>) -> Self {
        Self {
            controllers: HashMap::new(),
            stacks: HashMap::new(),
            current_tab: None,
            max_pages: max_pages.unwrap_or(10),
        }
    }

    /// Finds a PageController by path
    pub(crate) fn find_page_controller(&self, path: &str) -> Option<Arc<dyn PageController>> {
        self.controllers.get(path).map(|(c, _)| c.clone())
    }

    /// Pushes a new page controller
    pub(crate) fn push_page_controller(
        &mut self,
        path: String,
        is_tab_page: bool,
        controller: Arc<dyn PageController>,
    ) {
        if self.controllers.len() >= self.max_pages {
            self.destroy_least_active();
        }

        if is_tab_page {
            // Create new stack for tab page
            self.stacks
                .insert(path.clone(), PageStack::new(path.clone()));
            self.current_tab = Some(path.clone());
        } else if let Some(current_tab) = &self.current_tab {
            // Add to current tab's stack
            if let Some(stack) = self.stacks.get_mut(current_tab) {
                stack.push_page(path.clone());
            }
        }

        self.controllers.insert(path, (controller, Instant::now()));
    }

    /// Updates the last active time for a page
    pub(crate) fn mark_active(&mut self, path: &str) {
        if let Some((_, time)) = self.controllers.get_mut(path) {
            *time = Instant::now();

            // If this is a tab page, update current_tab
            if self.stacks.contains_key(path) {
                self.current_tab = Some(path.to_string());
            }
        }
    }

    /// Gets the previous page in the current stack
    pub(crate) fn get_previous_page(&self, current_path: &str) -> Option<String> {
        // Find which stack contains this page
        for stack in self.stacks.values() {
            if stack.current_page() == Some(current_path) {
                return stack.previous_page().map(String::from);
            }
        }
        None
    }

    /// Pops the current page from its stack
    pub(crate) fn pop_page_controller(&mut self, path: &str) -> Option<Arc<dyn PageController>> {
        // Never pop a tab page
        if self.stacks.contains_key(path) {
            return None;
        }

        // Remove from stack first
        if let Some(current_tab) = &self.current_tab {
            if let Some(stack) = self.stacks.get_mut(current_tab) {
                stack.pop_page();
            }
        }

        // Then remove the controller
        self.controllers.remove(path).map(|(c, _)| c)
    }

    /// Destroys the least active page that isn't a tab page
    fn destroy_least_active(&mut self) {
        let mut oldest_time = Instant::now();
        let mut oldest_path = None;

        // Find the oldest non-tab page
        for (path, (_, time)) in &self.controllers {
            if !self.stacks.contains_key(path) && *time < oldest_time {
                oldest_time = *time;
                oldest_path = Some(path.clone());
            }
        }

        // Remove it if found
        if let Some(path) = oldest_path {
            self.pop_page_controller(&path);
        }
    }
}

impl Drop for PageManager {
    fn drop(&mut self) {
        self.controllers.clear();
    }
}

/// Trait for controlling page operations from Rust
pub trait PageController: Send + Sync + Any {
    /// Loads the specified URL in the page
    /// Returns true if the URL was successfully loaded
    fn load_url(&self, url: String) -> bool;

    // configure UserAgent for the page
    fn setup_ua(&self, ua: &str);

    /// Evaluates JavaScript in the page context
    /// Returns Ok(()) if successful, Err with the error if failed
    fn evaluate_javascript(&self, js: &str) -> Result<(), Box<dyn std::error::Error>>;

    /// Clears the page's cache and history
    fn clear_browsing_data(&self);

    /// Enable or disable WebView debugging
    /// Returns true if the operation was successful
    fn set_devtools(&self, enabled: bool) -> bool;

    /// Get the Any trait object for downcasting
    fn as_any(&self) -> &dyn Any;
}
