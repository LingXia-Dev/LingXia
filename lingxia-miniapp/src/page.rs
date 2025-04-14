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
            // Create new stack for tab page or get existing
            let stack = self
                .stacks
                .entry(path.clone())
                .or_insert_with(|| PageStack::new(path.clone()));
            // Ensure the path is in the stack (might be creating or just switching to)
            stack.push_page(path.clone());
            self.current_tab = Some(path.clone());
        } else if let Some(current_tab) = &self.current_tab {
            // Add to current tab's stack
            if let Some(stack) = self.stacks.get_mut(current_tab) {
                stack.push_page(path.clone());
            }
        }

        // Insert or update controller - ensures controller is present even if page existed in stack
        self.controllers.insert(path, (controller, Instant::now()));
    }

    /// Updates the last active time for a page and sets current tab if it's a tab page
    pub(crate) fn mark_active(&mut self, path: &str) {
        if let Some((_, time)) = self.controllers.get_mut(path) {
            *time = Instant::now();
            // If this is a tab page, update current_tab
            if self.stacks.contains_key(path) {
                self.current_tab = Some(path.to_string());
            }
            // Note: We don't update the specific page stack here, only the current *tab*
        }
    }

    /// Pops the current page from the current tab's stack if possible.
    /// Returns the path of the page to navigate back *to* if successful.
    /// Also destroys the controller of the popped page.
    /// Returns None if the current page cannot be popped (e.g., it's the tab root).
    pub fn pop_from_current_stack(&mut self) -> Option<String> {
        let current_tab_path = match &self.current_tab {
            Some(tab) => tab,
            None => return None, // No current tab to pop from
        };

        // Use ? for cleaner handling if stack doesn't exist for current_tab_path
        let stack = self.stacks.get_mut(current_tab_path)?;

        let page_to_pop = match stack.current_page() {
            Some(p) => p.to_string(),
            None => return None, // Stack is empty? Error state.
        };

        // Check if the page to pop is the tab root itself
        if page_to_pop == *current_tab_path {
            return None; // Cannot pop the root page of the stack via back press
        }

        // Attempt to pop from the stack structure
        if stack.pop_page().is_some() {
            // Successfully popped from the stack structure.
            // Now, remove the controller for the popped page.
            self.controllers.remove(&page_to_pop); // Controller is dropped here

            // Return the path of the *new* current page in the stack
            stack.current_page().map(String::from)
        } else {
            // pop_page failed (e.g., already at root, though we checked)
            None
        }
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
            // We need to remove from the stack it belongs to as well
            if let Some(current_tab) = &self.current_tab {
                if let Some(stack) = self.stacks.get_mut(current_tab) {
                    // Only retain pages that are NOT the one we want to remove
                    stack.pages.retain(|p| p != &path);
                }
            }
            // TODO: If not found in current tab's stack, should we search others?

            self.controllers.remove(&path);
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

    /// Post a message to the page
    /// Returns Ok(()) if successful, Err with the error if failed
    fn post_message(&self, message: &str) -> Result<(), Box<dyn std::error::Error>>;

    /// Get the Any trait object for downcasting
    fn as_any(&self) -> &dyn Any;
}
