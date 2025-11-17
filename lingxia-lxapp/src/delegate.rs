use crate::PageLifecycleEvent;
use crate::event::AppServiceEvent;
use crate::lxapp::LxAppStatus;
use crate::page::NavigationType;
use crate::{LxApp, error, info, lxapp};
use lingxia_platform::AppRuntime;
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

            let page = self.get_or_create_page(&resolved_path);
            if page.is_tabbar_page() {
                self.with_tabbar_mut(|t| t.set_visible(true));
            }
            let _ = self.push_to_page_stack(&resolved_path);
            // Pre-create tab pages (synchronously enqueue); FIFO ordering ensures CreateAppSvc precedes these.
            if !was_already_opened {
                if let Some(tab_pages) = self.get_tabbar().map(|t| t.get_tabbar_pages()) {
                    for tab_path in tab_pages {
                        if tab_path == resolved_path {
                            continue;
                        }
                        let _ = self.get_or_create_page(&tab_path);
                    }
                }
            }
            self.set_status(LxAppStatus::Opening);
            if let Err(e) = self.appservice_notify(AppServiceEvent::OnLaunch, None) {
                error!("Failed to trigger onLaunch service: {}", e).with_appid(self.appid.clone());
            }
            self.set_status(LxAppStatus::Opened);

            // Update last_open_at in metadata for this installed app
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or_default();
            let _ = lxapp::metadata::touch_last_open(&self.appid, self.release_type, now);
        }

        // Ensure status reflects opened (both first open and reopen)
        self.set_status(LxAppStatus::Opened);

        // App-level onShow still fires here (app layer), independent of page service readiness.
        let options = self.state.lock().unwrap().startup_options.clone();
        let options_str = serde_json::to_string(&options).ok();
        let _ = self.appservice_notify(AppServiceEvent::OnShow, options_str);

        resolved_path
    }

    fn on_lxapp_closed(self: &Arc<Self>) {
        self.set_status(LxAppStatus::Closed);

        // Update last active time
        self.state.lock().unwrap().last_active_time = Instant::now();

        // Remove this LxApp from the navigation stack
        if let Some(manager) = lxapp::get_lxapps_manager() {
            manager.remove_from_stack(&self.appid);
        }

        // Trigger onHide
        if let Err(e) = self.appservice_notify(AppServiceEvent::OnHide, None) {
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
                        if let Some(target_page) = self.get_page(tab_path) {
                            if page
                                .navigate_to(target_page, NavigationType::SwitchTab)
                                .is_ok()
                            {
                                return true;
                            }
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
                        if let Some(target_page) = self.get_page(&home_route) {
                            let _ = page.navigate_to(target_page, navigate_type);
                        }
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
