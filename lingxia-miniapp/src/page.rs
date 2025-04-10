use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

pub struct PageManager {
    // path -> (PageController, last active time)
    controllers: HashMap<String, (Arc<dyn PageController>, Instant)>,
    active_order: VecDeque<String>, // Tracks the order of active pages
    max_pages: usize,               // Maximum number of pages allowed
}

impl PageManager {
    pub(crate) fn new(max_pages: Option<usize>) -> Self {
        Self {
            controllers: HashMap::new(),
            active_order: VecDeque::new(),
            max_pages: max_pages.unwrap_or(10),
        }
    }

    /// Finds a PageController by path
    pub(crate) fn find_page_controller(&self, path: &str) -> Option<Arc<dyn PageController>> {
        self.controllers.get(path).map(|(c, _)| c.clone())
    }

    /// Pushes a new PageController for the given path
    pub(crate) fn push_page_controller(
        &mut self,
        path: String,
        controller: Arc<dyn PageController>,
    ) {
        if self.controllers.len() >= self.max_pages {
            self.destroy_least_active();
        }

        self.controllers
            .insert(path.clone(), (controller, Instant::now()));
        self.active_order.push_back(path); // Add to the end of the active order
    }

    /// Pops the PageController for the given path
    pub(crate) fn pop_page_controller(&mut self, path: &str) -> Option<Arc<dyn PageController>> {
        if let Some((controller, _)) = self.controllers.remove(path) {
            // Remove the path from the active order
            self.active_order.retain(|p| p != path);
            Some(controller)
        } else {
            None
        }
    }

    /// Returns the current PageController for the given path
    pub(crate) fn current_page_controller(&self, path: &str) -> Option<Arc<dyn PageController>> {
        self.controllers.get(path).map(|(c, _)| c.clone())
    }

    /// Destroys the least active page
    pub(crate) fn destroy_least_active(&mut self) {
        if let Some(path) = self.active_order.pop_front() {
            self.controllers.remove(&path);
            println!("Destroyed least active page: {}", path);
        }
    }

    /// Updates the last active time for a page
    pub(crate) fn mark_active(&mut self, path: &str) {
        if let Some((_controller, _)) = self.controllers.get_mut(path) {
            // Move the path to the end of the active order
            self.active_order.retain(|p| p != path);
            self.active_order.push_back(path.to_string());
        }
    }
}

impl Drop for PageManager {
    fn drop(&mut self) {
        self.controllers.clear();
        self.active_order.clear();
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
