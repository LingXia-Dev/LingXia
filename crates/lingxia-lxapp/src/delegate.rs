use crate::PageLifecycleEvent;
use crate::lifecycle::AppServiceEvent;
use crate::lxapp::LxAppSessionStatus;
use crate::page::NavigationType;
use crate::update::UpdateManager;
use crate::{LxApp, error, info, lxapp, warn};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::pull_to_refresh::PullToRefresh;
use std::sync::Arc;
use std::time::Instant;

/// lxapp-scoped UI event types (page/app runtime events).
#[derive(Debug, Clone, PartialEq)]
pub enum LxAppUiEventType {
    /// TabBar item clicked
    TabBarClick = 0,
    /// Capsule button clicked (close, minimize, more)
    CapsuleClick = 1,
    /// Navigation bar button clicked (back, home, title)
    NavigationClick = 2,
    /// System back button pressed
    BackPress = 3,
    /// Pull-to-refresh triggered by user
    PullDownRefresh = 4,
}

pub trait LxAppDelegate {
    /// Called when lxapp is opened
    /// Returns the resolved path that should be used
    fn on_lxapp_opened(self: Arc<Self>, path: String, session_id: u64) -> String;

    /// Called when lxapp is closed
    fn on_lxapp_closed(self: &Arc<Self>, session_id: u64);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    /// Handle UI events
    /// Returns true if the event was handled, false to allow default behavior
    fn on_lxapp_event(self: &Arc<Self>, event_type: LxAppUiEventType, data: String) -> bool;
}

impl LxAppDelegate for LxApp {
    fn on_lxapp_opened(self: Arc<Self>, path: String, session_id: u64) -> String {
        let current_session = self.session_id();
        if session_id != current_session {
            return String::new();
        }

        let previous_appid = lxapp::get_current_lxapp().0;

        let raw_url = if path.is_empty() {
            self.config.get_initial_route()
        } else {
            path
        };

        let resolved = crate::route::resolve_route(&self, &raw_url).unwrap_or_else(|e| {
            error!("Failed to resolve page url '{}': {}", raw_url, e)
                .with_appid(self.appid.clone());
            crate::route::ResolvedRoute {
                original: raw_url.clone(),
                query: None,
                target: crate::route::RouteTarget::Normal {
                    path: raw_url.clone(),
                },
            }
        });

        let resolved_path = resolved.internal_path();
        let was_already_opened = self.is_opened();

        // When switching to this app, hide the previously active app (if any).
        if !previous_appid.is_empty()
            && previous_appid != self.appid
            && let Some(previous) = lxapp::try_get(&previous_appid)
        {
            let args = crate::lifecycle::AppServiceEventArgs {
                source: crate::lifecycle::AppServiceEventSource::Lxapp,
                reason: crate::lifecycle::AppServiceEventReason::SwitchAway,
            }
            .to_json_string();
            let _ = previous.appservice_notify(AppServiceEvent::OnHide, Some(args));
        }

        // Move this app to the top of the navigation stack.
        if let Some(manager) = lxapp::get_lxapps_manager() {
            manager.remove_from_stack(&self.appid);
            manager.push_lxapp_stack(self.appid.clone());
        }

        if !was_already_opened {
            let page = self.get_or_create_page(&resolved_path);
            if let Some(query) = resolved.query.clone() {
                page.set_query(query);
            }
            if page.is_tabbar_page() {
                // Ensure TabBar is visible and selected index matches the resolved path.
                self.with_tabbar_mut(|t| {
                    t.set_visible(true);
                    if let Some(index) = t.find_index_by_path(&resolved_path) {
                        t.set_selected_index(index);
                    }
                });
            }
            let _ = self.push_to_page_stack(&resolved_path);
            // Pre-create tab pages (synchronously enqueue); FIFO ordering ensures CreateAppSvc precedes these.
            if let Some(tab_pages) = self.get_tabbar().map(|t| t.get_tabbar_pages()) {
                for tab_path in tab_pages {
                    if tab_path == resolved_path {
                        continue;
                    }
                    let _ = self.get_or_create_page(&tab_path);
                }
            }
            self.set_status(LxAppSessionStatus::Opening);
            if let Err(e) = self.appservice_notify(AppServiceEvent::OnLaunch, None) {
                error!("Failed to trigger onLaunch service: {}", e).with_appid(self.appid.clone());
            }
            self.set_status(LxAppSessionStatus::Opened);

            // Update last_open_at in metadata for this installed app
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or_default();
            let _ = lxapp::metadata::touch_last_open(&self.appid, self.release_type, now);
        }

        // Ensure status reflects opened (both first open and reopen)
        self.set_status(LxAppSessionStatus::Opened);

        // Cancel any pending delayed-destroy now that the app is reopened.
        if let Some(manager) = lxapp::get_lxapps_manager() {
            manager.cancel_delayed_destroy(&self.appid);
        }

        // App-level onShow still fires here (app layer), independent of page service readiness.
        let options = self.state.lock().unwrap().startup_options.clone();
        let mut args = serde_json::to_value(&options).unwrap_or_else(|_| serde_json::json!({}));
        if let serde_json::Value::Object(map) = &mut args {
            map.insert(
                "source".to_string(),
                serde_json::to_value(crate::lifecycle::AppServiceEventSource::Lxapp)
                    .unwrap_or_else(|_| serde_json::Value::String("lxapp".to_string())),
            );
            map.insert(
                "reason".to_string(),
                serde_json::to_value(if was_already_opened {
                    crate::lifecycle::AppServiceEventReason::SwitchBack
                } else {
                    crate::lifecycle::AppServiceEventReason::Open
                })
                .unwrap_or_else(|_| serde_json::Value::String("unknown".to_string())),
            );
        }
        let args_str = serde_json::to_string(&args).ok();
        let _ = self.appservice_notify(AppServiceEvent::OnShow, args_str);
        self.trigger_home_update_check_once();

        if self.has_pending_restart_request()
            && let Err(e) = self.restart()
        {
            error!("Deferred restart after open failed: {}", e).with_appid(self.appid.clone());
        }

        #[cfg(target_os = "windows")]
        self.sync_windows_shell_layout();

        resolved_path
    }

    fn on_lxapp_closed(self: &Arc<Self>, session_id: u64) {
        let current_session = self.session_id();
        if session_id != current_session {
            return;
        }

        self.set_status(LxAppSessionStatus::Closed);
        self.clear_transient_files();

        // Update last active time. Recover from poisoned mutex instead of panicking.
        self.state
            .lock()
            .unwrap_or_else(|e| {
                warn!("Recovered poisoned lxapp state mutex during close")
                    .with_appid(self.appid.clone());
                e.into_inner()
            })
            .last_active_time = Instant::now();

        // Remove this LxApp from the navigation stack
        if let Some(manager) = lxapp::get_lxapps_manager() {
            manager.remove_from_stack(&self.appid);
            manager.schedule_delayed_destroy(self.appid.clone());
        }

        // Trigger onHide with reason so JS can distinguish lxapp close vs host background.
        let args = crate::lifecycle::AppServiceEventArgs {
            source: crate::lifecycle::AppServiceEventSource::Lxapp,
            reason: crate::lifecycle::AppServiceEventReason::Close,
        }
        .to_json_string();
        if let Err(e) = self.appservice_notify(AppServiceEvent::OnHide, Some(args)) {
            error!("Failed to trigger onHide service: {}", e).with_appid(self.appid.clone());
        }
    }

    fn on_page_show(self: &Arc<Self>, path: String) {
        // Get the existing page - it should already exist when show is called
        let page = match self.get_page(&path) {
            Some(page) => page,
            None => {
                error!("PageInstance not found when showing: {}", path)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
                return;
            }
        };

        page.dispatch_lifecycle_event(PageLifecycleEvent::OnShow);

        // Mark the page as active for LRU tracking
        page.mark_active();

        #[cfg(target_os = "windows")]
        self.sync_windows_shell_layout();
    }

    fn on_lxapp_event(self: &Arc<Self>, event_type: LxAppUiEventType, data: String) -> bool {
        info!("UI event received: {:?}, data: {}", event_type, data).with_appid(self.appid.clone());

        let handled = match event_type {
            LxAppUiEventType::TabBarClick => self.handle_tabbar_click(data),
            LxAppUiEventType::CapsuleClick => self.handle_capsule_click(data),
            LxAppUiEventType::NavigationClick => self.handle_navigation_click(data),
            LxAppUiEventType::BackPress => self.handle_back_press(),
            LxAppUiEventType::PullDownRefresh => self.handle_pull_down_refresh(data),
        };

        #[cfg(target_os = "windows")]
        self.sync_windows_shell_layout();

        handled
    }
}

impl LxApp {
    /// Handle TabBar item click
    fn handle_tabbar_click(self: &Arc<Self>, data: String) -> bool {
        if let Ok(index) = data.parse::<usize>() {
            info!("TabBar item {} clicked", index).with_appid(self.appid.clone());

            if let Some(tabbar) = self.get_tabbar()
                && tabbar.get_selected_index() == index as i32
            {
                return true; // Already selected, do nothing
            }

            // Let page.navigate own the committed selection; Windows may mirror it early
            // to keep native chrome responsive while the target WebView finishes loading.
            let tab_pages = self
                .get_tabbar()
                .map(|t| t.get_tabbar_pages())
                .unwrap_or_default();
            if let Some(tab_path) = tab_pages.get(index) {
                if let Some(current_page_path) = self.peek_current_page() {
                    let current_page = self
                        .get_page(&current_page_path)
                        .unwrap_or_else(|| self.get_or_create_page(&current_page_path));
                    let target_page = self.get_or_create_page(tab_path);

                    if current_page
                        .navigate_to(target_page, NavigationType::SwitchTab)
                        .is_ok()
                    {
                        return true;
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

                // after SDK hides it, SDK should call get_current_lxapp to show another lxapp
                let _ = self
                    .runtime
                    .hide_lxapp(self.appid.clone(), self.session_id());
                return true;
            }
            "minimize" => {
                // Minimize the app (platform-specific behavior)
                info!("LxApp minimize requested").with_appid(self.appid.clone());
                return true;
            }
            "clean_cache_restart" => {
                // Clear cache directory and restart the LxApp
                info!("Clean cache & restart requested").with_appid(self.appid.clone());

                // Clear and recreate cache directory
                let cache_dir = &self.user_cache_dir;
                if cache_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(cache_dir) {
                        error!("Failed to remove cache directory: {}", e)
                            .with_appid(self.appid.clone());
                    } else {
                        info!("Cache directory cleared: {}", cache_dir.display())
                            .with_appid(self.appid.clone());
                    }
                }
                // Recreate the cache directory
                if let Err(e) = std::fs::create_dir_all(cache_dir) {
                    error!("Failed to recreate cache directory: {}", e)
                        .with_appid(self.appid.clone());
                }

                if let Err(e) = self.restart() {
                    error!("Failed to restart app after cache cleanup: {}", e)
                        .with_appid(self.appid.clone());
                    return false;
                }
                return true;
            }
            "restart" => {
                info!("Restart requested").with_appid(self.appid.clone());
                if let Err(e) = self.restart() {
                    error!("Failed to restart app: {}", e).with_appid(self.appid.clone());
                    return false;
                }
                return true;
            }
            "uninstall" => {
                info!("Uninstall requested").with_appid(self.appid.clone());

                // Fully shutdown first so uninstall precondition (`!is_lxapp_open`) is satisfied.
                if let Err(e) = self.shutdown() {
                    error!("Failed to shutdown app before uninstall: {}", e)
                        .with_appid(self.appid.clone());
                    return false;
                }

                let appid = self.appid.clone();
                let lxapp = self.clone();
                std::mem::drop(crate::executor::spawn(async move {
                    let updater = UpdateManager::new(lxapp);
                    if let Err(e) = updater.uninstall_all(&appid) {
                        error!("Failed to uninstall app: {}", e).with_appid(appid);
                    }
                }));
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
                if let Some(path) = self.peek_current_page()
                    && let Some(page) = self.get_page(path.as_str())
                {
                    let _ = page.navigate_back(1);
                    return true;
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
                    let page = self
                        .get_page(&path)
                        .unwrap_or_else(|| self.get_or_create_page(&path));
                    let target_page = self.get_or_create_page(&home_route);
                    let _ = page.navigate_to(target_page, navigate_type);
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
            // If it's the last page, hide this LxApp (except home app)
            if !self.is_home_lxapp {
                let _ = self
                    .runtime
                    .hide_lxapp(self.appid.clone(), self.session_id());
            }
            return true;
        }

        if let Some(path) = self.peek_current_page()
            && let Some(page) = self.get_page(path.as_str())
        {
            let _ = page.navigate_back(1);
            return true;
        }
        false
    }

    /// Handle pull-to-refresh event
    /// data: page path
    fn handle_pull_down_refresh(self: &Arc<Self>, data: String) -> bool {
        let path = if data.is_empty() {
            match self.peek_current_page() {
                Some(p) => p,
                None => return false,
            }
        } else {
            data
        };

        if !self.is_pull_down_refresh_enabled(&path) {
            if let Err(e) = self.runtime.stop_pull_down_refresh(&self.appid, &path) {
                error!("Failed to stop pull-to-refresh: {}", e).with_appid(self.appid.clone());
            }
            return false;
        }

        if let Some(page) = self.get_page(&path) {
            page.dispatch_lifecycle_event(PageLifecycleEvent::OnPullDownRefresh);
            true
        } else {
            error!("PageInstance not found for pull-to-refresh: {}", path)
                .with_appid(self.appid.clone());
            if let Err(e) = self.runtime.stop_pull_down_refresh(&self.appid, &path) {
                error!("Failed to stop pull-to-refresh: {}", e).with_appid(self.appid.clone());
            }
            false
        }
    }
}
