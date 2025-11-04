use crate::page::{NavigationType, PageLifecycleEvent};
use crate::{LxApp, error, info, lxapp};
use lingxia_platform::AppRuntime;
use rong::service_executor;
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
    /// Returns the resolved path that should be used
    fn on_lxapp_opened(self: Arc<Self>, path: String) -> String;

    /// Called when lxapp is closed
    fn on_lxapp_closed(self: &Arc<Self>);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    /// Handle UI events
    /// Returns true if the event was handled, false to allow default behavior
    fn on_ui_event(self: &Arc<Self>, event_type: UiEventType, data: String) -> bool;
}

impl LxAppDelegate for LxApp {
    fn on_lxapp_opened(self: Arc<Self>, path: String) -> String {
        // Resolve the actual path to use - if empty, use initial route
        let resolved_path = if path.is_empty() {
            self.config.get_initial_route()
        } else {
            path
        };

        let was_already_opened = self.is_opened();

        info!("LxApp opened (already_opened: {})", was_already_opened)
            .with_appid(self.appid.clone())
            .with_path(resolved_path.clone());

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

        // Create or get the page first for launch page
        let page = self.get_or_create_page(&resolved_path);
        if page.is_tabbar_page() {
            self.with_tabbar_mut(|t| t.set_visible(true));
        }

        if let Err(e) = self.push_to_page_stack(&resolved_path) {
            error!("Failed to initialize page stack: {}", e)
                .with_appid(self.appid.clone())
                .with_path(resolved_path.clone());
        }

        let options = self.state.lock().unwrap().startup_options.clone();
        let options_str = serde_json::to_string(&options).ok();

        if let Err(e) =
            self.executor
                .call_app_service(self.appid.clone(), "onShow".to_string(), options_str)
        {
            error!("Failed to trigger onShow service: {}", e).with_appid(self.appid.clone());
        }

        // Pre-create all tab pages in background (only on first open)
        let tab_pages = self
            .get_tabbar()
            .map(|t| t.get_tabbar_pages())
            .unwrap_or_else(Vec::new);
        if !was_already_opened && !tab_pages.is_empty() {
            let initial_path = resolved_path.clone();
            let lxapp_clone = self.clone();

            if let Err(e) = service_executor::spawn_blocking(move || {
                info!("Pre-creating tab pages...").with_appid(lxapp_clone.appid.clone());
                for tab_path in tab_pages {
                    if tab_path == initial_path {
                        continue; // Skip the initial page we already created
                    }
                    let _ = lxapp_clone.get_or_create_page(&tab_path);
                }
            }) {
                error!("Failed to spawn background task for tab pre-create: {}", e)
                    .with_appid(self.appid.clone());
            }
        }

        resolved_path
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

        page.dispatch_lifecycle_event(PageLifecycleEvent::OnShow);

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

            if let Some(tabbar) = self.get_tabbar() {
                if tabbar.get_selected_index() == index as i32 {
                    return true; // Already selected, do nothing
                }
            }

            // Don't set the index here - let page.navigate handle it
            let tab_pages = self
                .get_tabbar()
                .map(|t| t.get_tabbar_pages())
                .unwrap_or_else(Vec::new);
            if let Some(tab_path) = tab_pages.get(index) {
                if let Some(current_page_path) = self.peek_current_page() {
                    if let Some(page) = self.get_page(&current_page_path) {
                        let target_page = self.get_or_create_page(tab_path);
                        if page
                            .navigate_to(target_page, NavigationType::SwitchTab)
                            .is_ok()
                        {
                            return true;
                        }
                    }
                }
                // Fallback or error handling if no current page
                error!("Could not get current page to perform navigation")
                    .with_appid(self.appid.clone());
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
            "back" => {
                if let Some(path) = self.peek_current_page() {
                    if let Some(page) = self.get_page(path.as_str()) {
                        let _ = page.navigate_back(1);
                        return true;
                    }
                }
                false
            }
            "home" => {
                // Navigate to home page using Launch
                let home_route = self.config.get_initial_route();
                let navigate_type = if let Some(tabbar) = self.get_tabbar() {
                    if tabbar.is_tabbar_page(&home_route) {
                        NavigationType::SwitchTab
                    } else {
                        NavigationType::Launch
                    }
                } else {
                    NavigationType::Launch
                };

                if let Some(path) = self.peek_current_page() {
                    if let Some(page) = self.get_page(&path) {
                        let target_page = self.get_or_create_page(&home_route);
                        let _ = page.navigate_to(target_page, navigate_type);
                    }
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
        let stack_size = self.get_page_stack_size();
        info!("BackPress trigered, page stack size: {}", stack_size).with_appid(self.appid.clone());

        if stack_size <= 1 {
            // if it's last page, clsoe this lxapp
            if !self.is_home_lxapp {
                let _ = self.runtime.close_lxapp(self.appid.clone());
            }
            return true;
        }

        if let Some(path) = self.peek_current_page() {
            if let Some(page) = self.get_page(path.as_str()) {
                let _ = page.navigate_back(1);
                return true;
            }
        }
        false
    }
}
