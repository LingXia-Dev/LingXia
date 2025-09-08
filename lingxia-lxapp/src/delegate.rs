use crate::executor::LxAppExecutor;
use crate::{LxApp, error, info, lxapp};
use lingxia_platform::{AppRuntime, NavigationType};
use std::sync::Arc;
use std::time::Instant;

/// UI event types for unified event handling
#[derive(Debug, Clone, PartialEq)]
pub enum UiEventType {
    /// TabBar item clicked
    TabBarClick = 0,
    /// Capsule button clicked (close, minimize, more)
    CapsuleClick = 1,
    /// Navigation bar button clicked (back, home, title)
    NavigationClick = 2,
    /// System back button pressed
    BackPress = 3,
}

pub trait LxAppDelegate {
    /// Called when lxapp is opened
    fn on_lxapp_opened(self: Arc<Self>, path: String);

    /// Called when lxapp is closed
    fn on_lxapp_closed(self: &Arc<Self>);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    /// Handle UI events
    /// Returns true if the event was handled, false to allow default behavior
    fn on_ui_event(self: &Arc<Self>, event_type: UiEventType, data: String) -> bool;
}

impl LxAppDelegate for LxApp {
    fn on_lxapp_opened(self: Arc<Self>, path: String) {
        let was_already_opened = self.is_opened();

        info!("LxApp opened (already_opened: {})", was_already_opened)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        if !was_already_opened {
            // Push to lxapp navigation stack
            if let Some(manager) = lxapp::get_lxapps_manager() {
                manager.push_lxapp_stack(self.appid.clone());
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

        // Remove this LxApp from the navigation stack
        if let Some(manager) = lxapp::get_lxapps_manager() {
            manager.remove_from_stack(&self.appid);
        }

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

    fn on_ui_event(self: &Arc<Self>, event_type: UiEventType, data: String) -> bool {
        info!("UI event received: {:?}, data: {}", event_type, data).with_appid(self.appid.clone());

        match event_type {
            UiEventType::TabBarClick => self.handle_tabbar_click(data),
            UiEventType::CapsuleClick => self.handle_capsule_click(data),
            UiEventType::NavigationClick => self.handle_navigation_click(data),
            UiEventType::BackPress => self.handle_back_press(),
        }
    }
}

impl LxApp {
    /// Handle TabBar item click
    fn handle_tabbar_click(self: &Arc<Self>, data: String) -> bool {
        if let Ok(index) = data.parse::<usize>() {
            info!("TabBar item {} clicked", index).with_appid(self.appid.clone());

            // Get tab pages from config
            let tab_pages = self.config.get_tab_pages();
            if let Some(tab_path) = tab_pages.get(index) {
                // Clear page stack when switching tabs (tab pages are root pages)
                if let Err(e) = self.clear_page_stack() {
                    error!("Failed to clear page stack: {}", e).with_appid(self.appid.clone());
                }

                // Navigate to the selected tab page
                if let Err(e) = self.runtime.navigate(
                    self.appid.clone(),
                    tab_path.clone(),
                    NavigationType::SwitchTab,
                ) {
                    error!("Failed to switch to tab {}: {}", tab_path, e)
                        .with_appid(self.appid.clone());
                    return false;
                }
                return true;
            } else {
                error!("Invalid tab index: {}", index).with_appid(self.appid.clone());
            }
        } else {
            error!("Invalid tab index format: {}", data).with_appid(self.appid.clone());
        }
        false
    }

    /// Handle capsule button click
    fn handle_capsule_click(self: &Arc<Self>, data: String) -> bool {
        info!("Capsule button '{}' clicked", data).with_appid(self.appid.clone());

        match data.as_str() {
            "close" => {
                // Clear page stack when closing app
                if let Err(e) = self.clear_page_stack() {
                    error!("Failed to clear page stack: {}", e).with_appid(self.appid.clone());
                }

                // after SDK close it, SDK should call get_current_lxapp to show another lxapp
                let _ = self.runtime.close_lxapp(self.appid.clone());
                return true;
            }
            "minimize" => {
                // Minimize the app (platform-specific behavior)
                info!("LxApp minimize requested").with_appid(self.appid.clone());
                return true;
            }
            "more" => {
                // Show more options menu
                info!("More options requested").with_appid(self.appid.clone());
                return true;
            }
            _ => {
                error!("Unknown capsule action: {}", data).with_appid(self.appid.clone());
            }
        }
        false
    }

    /// Handle navigation bar button click
    fn handle_navigation_click(self: &Arc<Self>, data: String) -> bool {
        info!("Navigation button '{}' clicked", data).with_appid(self.appid.clone());

        match data.as_str() {
            "back" => self.handle_back_press(),
            "home" => {
                // Clear page stack when navigating to home
                if let Err(e) = self.clear_page_stack() {
                    error!("Failed to clear page stack: {}", e).with_appid(self.appid.clone());
                }

                // Navigate to home page using Launch
                let home_route = self.config.get_initial_route();
                if let Err(e) =
                    self.runtime
                        .navigate(self.appid.clone(), home_route, NavigationType::Launch)
                {
                    error!("Failed to navigate to home: {}", e).with_appid(self.appid.clone());
                    return false;
                }
                true
            }
            _ => {
                error!("Unknown navigation action: {}", data).with_appid(self.appid.clone());
                false
            }
        }
    }

    /// Handle back button press (system or navigation)
    fn handle_back_press(self: &Arc<Self>) -> bool {
        info!("BackPress trigered").with_appid(self.appid.clone());

        if self.get_page_stack_size() <= 1 {
            // if it's last page, clsoe this lxapp
            let _ = self.runtime.close_lxapp(self.appid.clone());
            return true;
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
