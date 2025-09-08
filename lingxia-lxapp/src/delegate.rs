use crate::executor::LxAppExecutor;
use crate::{LxApp, error, info, lxapp};
use lingxia_platform::{AppRuntime, NavigationType};
use std::sync::Arc;
use std::time::Instant;

pub trait LxAppDelegate {
    /// Called when lxapp is opened
    fn on_lxapp_opened(self: Arc<Self>, path: String);

    /// Called when lxapp is closed
    fn on_lxapp_closed(self: &Arc<Self>);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    /// Handle back button press
    /// Return true to indicate the back press had been handled
    fn on_back_pressed(self: &Arc<Self>) -> bool;
}

impl LxAppDelegate for LxApp {
    fn on_lxapp_opened(self: Arc<Self>, path: String) {
        let was_already_opened = self.is_opened();

        info!("LxApp opened (already_opened: {})", was_already_opened)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        if !was_already_opened {
            // Push to navigation stack if not home app and not already opened
            if !self.is_home_lxapp {
                if let Some(manager) = lxapp::get_lxapps_manager() {
                    manager.push_lxapp_stack(self.appid.clone());
                }
            }

            // First-time launch logic
            if let Err(e) = self.executor.create_app_svc(self.clone()) {
                error!("Failed to trigger app service: {}", e).with_appid(self.appid.clone());
            }
            if let Err(e) =
                self.executor
                    .call_app_service(self.appid.clone(), "onLaunch".to_string(), None)
            {
                error!("Failed to trigger onLaunch service: {}", e).with_appid(self.appid.clone());
            }
            self.state.lock().unwrap().opened = true;
        }

        // Create or get the initial page first
        if self.get_or_create_page(&path).is_none() {
            error!("Failed to create initial page")
                .with_appid(self.appid.clone())
                .with_path(path.clone());
            return;
        }

        if let Err(e) =
            self.executor
                .call_app_service(self.appid.clone(), "onShow".to_string(), None)
        {
            error!("Failed to trigger onShow service: {}", e).with_appid(self.appid.clone());
        }

        // Call onShow for the page itself
        if let Err(e) = self.executor.call_page_service(
            self.appid.clone(),
            path.clone(),
            "onShow".to_string(),
            None,
        ) {
            error!("Failed to call onShow: {}", e)
                .with_appid(self.appid.clone())
                .with_path(path.clone());
        }

        // Pre-create all tab pages in background (only on first open)
        if !was_already_opened && self.config.has_tab_bar() {
            let tab_pages = self.config.get_tab_pages();
            let initial_path = path.clone();
            let lxapp_clone = self.clone();

            LxAppExecutor::spawn_task(move || {
                info!("Pre-creating tab pages...").with_appid(lxapp_clone.appid.clone());
                for tab_path in tab_pages {
                    if tab_path == initial_path {
                        continue; // Skip the initial page we already created
                    }
                    if lxapp_clone.get_or_create_page(&tab_path).is_none() {
                        error!("Failed to pre-create tab page: {}", tab_path)
                            .with_appid(lxapp_clone.appid.clone());
                    }
                }
            });
        }
    }

    fn on_lxapp_closed(self: &Arc<Self>) {
        self.state.lock().unwrap().opened = false;

        // Update last active time
        self.state.lock().unwrap().last_active_time = Instant::now();

        // Trigger onHide
        if let Err(e) =
            self.executor
                .call_app_service(self.appid.clone(), "onHide".to_string(), None)
        {
            error!("Failed to trigger onHide service: {}", e).with_appid(self.appid.clone());
        }

        // Log the app closing event
        info!("LxApp closed").with_appid(self.appid.clone());
    }

    fn on_page_show(self: &Arc<Self>, path: String) {
        // Get the existing page - it should already exist when show is called
        let page = match self.get_page(&path) {
            Some(page) => page,
            None => {
                error!("Page not found when showing: {}", path)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
                return;
            }
        };

        // Push to page stack - this is where all pages get pushed, regardless of type
        if let Err(e) = self.push_to_page_stack(&path) {
            error!("Failed to push page to stack: {}", e)
                .with_appid(self.appid.clone())
                .with_path(path.clone());
        }

        // Mark the page as active for LRU tracking
        page.mark_active();
    }

    fn on_back_pressed(self: &Arc<Self>) -> bool {
        // Only handle back press if there are pages to go back to
        if self.get_page_stack_size() <= 1 {
            return false; // Let the system handle it (e.g., close app)
        }

        // Pop the current page from the stack
        self.pop_from_page_stack();

        // Get the new top page to navigate to
        let state = self.state.lock().unwrap();
        if let Some(prev_path) = state.page_stack.lock().unwrap().back() {
            if let Err(e) = self.runtime.navigate(
                self.appid.clone(),
                prev_path.clone(),
                NavigationType::Backward,
            ) {
                error!("Failed to navigate back to page {}: {}", prev_path, e)
                    .with_appid(self.appid.clone());
                return true; // We tried to handle it, but failed
            }
            true // We handled the back press
        } else {
            false // Should not happen if stack size > 1, but as safeguard
        }
    }
}
