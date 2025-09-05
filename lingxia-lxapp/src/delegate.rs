use http::{Response, StatusCode};
use std::sync::Arc;
use std::time::Instant;

use crate::executor::LxAppExecutor;
use crate::log::{self, LogLevel, LogTag};
use crate::page::{Page, PageLoadState};
use crate::{LxApp, appservice, error, info};
use lingxia_platform::AppRuntime;

pub trait LxAppDelegate {
    /// Called when mini app is opened
    fn on_lxapp_opened(self: Arc<Self>, path: String);

    /// Called when mini app is closed
    fn on_lxapp_closed(self: &Arc<Self>);

    /// Called when the page starts loading
    fn on_page_started(self: &Arc<Self>, path: String);

    /// Called when the page finishes loading
    fn on_page_finished(self: &Arc<Self>, path: String);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    // Called when Scroll changed
    fn on_page_scroll_changed(
        self: &Arc<Self>,
        path: String,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    );

    /// Handle back button press
    /// Return true to indicate the back press had been handled
    fn on_back_pressed(self: &Arc<Self>) -> bool;

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(self: &Arc<Self>, path: String, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(
        self: &Arc<Self>,
        req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>>;

    /// Receive log from WebView
    fn log(self: &Arc<Self>, path: &str, level: LogLevel, message: &str);
}

impl LxAppDelegate for LxApp {
    fn on_lxapp_opened(self: Arc<Self>, path: String) {
        let was_already_opened = self.is_opened();

        info!("Mini app opened (already_opened: {})", was_already_opened)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        if was_already_opened {
            return;
        }

        // Use the Arc<Self> directly instead of looking it up in the global manager
        if let Err(e) = self.executor.create_app_svc(self.clone()) {
            error!("Failed to trigger app service: {}", e).with_appid(self.appid.clone());
        }
        if let Err(e) =
            self.executor
                .call_app_service(self.appid.clone(), "onLaunch".to_string(), None)
        {
            error!("Failed to trigger onLaunch service: {}", e).with_appid(self.appid.clone());
        }

        // Create the page for the given path if it doesn't exist
        // This path is typically the initial_route.
        let mut state = self.state.lock().unwrap();
        let self_for_setup = self.clone();

        if state.pages.get_page(&path).is_none() {
            // Build PageState from JSON config
            let page_state = Page::build_page_state(&*self, &path);

            if let Err(e) = state.pages.create_page(
                self.appid.clone(),
                path.clone(),
                page_state,
                self.executor.clone(),
                move |page, path| {
                    self_for_setup.setup_page(page, path);

                    // Create page service
                    if let Err(e) = self_for_setup
                        .executor
                        .create_page_svc(self_for_setup.appid.clone(), path.to_string())
                    {
                        error!("Failed to request page service creation: {}", e)
                            .with_appid(self_for_setup.appid.clone())
                            .with_path(path.to_string());
                    }
                },
            ) {
                error!("Failed to create page for path: {}", e)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
            }
        }
        state.opened = true;

        // Precreate tab pages in background (only for first time opening)
        if self.config.has_tab_bar() {
            let self_clone = self.clone();

            LxAppExecutor::spawn_task(move || {
                info!("Starting tab pages precreation").with_appid(self_clone.appid.clone());

                let tab_pages = self_clone.config.get_tab_pages();
                for tab_path in tab_pages {
                    if path == tab_path {
                        continue;
                    }

                    // Check if page already exists
                    {
                        let state = self_clone.state.lock().unwrap();
                        if state.pages.get_page(&tab_path).is_some() {
                            info!("Tab page already exists, skipping: {}", tab_path)
                                .with_appid(self_clone.appid.clone())
                                .with_path(tab_path.clone());
                            continue;
                        }
                    }

                    info!("Precreating tab page: {}", tab_path)
                        .with_appid(self_clone.appid.clone())
                        .with_path(tab_path.clone());

                    // Create page in background
                    let mut state = self_clone.state.lock().unwrap();
                    let self_for_setup = self_clone.clone();

                    // Build PageState from JSON config
                    let page_state = Page::build_page_state(&*self_clone, &tab_path);

                    let _ = state.pages.create_page(
                        self_clone.appid.clone(),
                        tab_path.clone(),
                        page_state,
                        self_clone.executor.clone(),
                        move |page, path| {
                            // Setup page content (load HTML)
                            self_for_setup.setup_page(page, path);

                            // Create page service
                            if let Err(e) = self_for_setup
                                .executor
                                .create_page_svc(self_for_setup.appid.clone(), path.to_string())
                            {
                                error!("Failed to request page service creation: {}", e)
                                    .with_appid(self_for_setup.appid.clone())
                                    .with_path(path.to_string());
                            }
                        },
                    );
                }

                info!("Tab pages precreation completed").with_appid(self_clone.appid.clone());
            });
        }
    }

    fn on_lxapp_closed(self: &Arc<Self>) {
        self.state.lock().unwrap().opened = false;

        // Update last active time
        self.state.lock().unwrap().last_active_time = Instant::now();

        // Log the app closing event
        info!("Mini app closed").with_appid(self.appid.clone());
    }

    fn on_page_started(self: &Arc<Self>, path: String) {
        let _ = self.executor.call_page_service(
            self.appid.clone(),
            path.clone(),
            "onLoad".to_string(),
            None,
        );
    }

    fn on_page_finished(self: &Arc<Self>, path: String) {
        let _ = self.executor.call_page_service(
            self.appid.clone(),
            path.clone(),
            "onReady".to_string(),
            None,
        );

        let state = self.state.lock().unwrap();
        if let Some(page) = state.pages.get_page(&path) {
            page.set_load_state(PageLoadState::Loaded);
        }
    }

    fn on_page_show(self: &Arc<Self>, path: String) {
        info!("on_page_show called for path: {}", path)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        // Navigate to the new page and get the previous page if there was a switch
        let previous_page = {
            let mut state = self.state.lock().unwrap();
            // Clone tabbar to avoid borrow checker issues
            let tabbar = state.tabbar.clone();
            state.pages.navigate_to_page(path.clone(), tabbar.as_ref())
        };

        // Call onHide for the previous page if there was a page switch
        if let Some(prev_path) = previous_page {
            if let Err(e) = self.executor.call_page_service(
                self.appid.clone(),
                prev_path.clone(),
                "onHide".to_string(),
                None,
            ) {
                error!("Failed to call onHide for page {}: {}", prev_path, e)
                    .with_appid(self.appid.clone());
            }
        }

        // Call onShow for the new page
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
    }

    fn on_page_scroll_changed(
        self: &Arc<Self>,
        _path: String,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    ) {
        // safe division to avoid division by zero
        let scroll_percent_x = if max_scroll_x > 0 {
            (scroll_x as f64 / max_scroll_x as f64 * 100.0) as i32
        } else {
            0
        };

        let scroll_percent_y = if max_scroll_y > 0 {
            (scroll_y as f64 / max_scroll_y as f64 * 100.0) as i32
        } else {
            0
        };

        info!(
            "Scroll: x={}/{} ({}%), y={}/{} ({}%)",
            scroll_x, max_scroll_x, scroll_percent_x, scroll_y, max_scroll_y, scroll_percent_y
        );
    }

    fn on_back_pressed(self: &Arc<Self>) -> bool {
        info!("Backbutton pressed").with_appid(self.appid.clone());

        // Try to pop the current page from the stack
        let previous_page = {
            let mut state = self.state.lock().unwrap();
            // Clone tabbar to avoid borrow checker issues
            let tabbar = state.tabbar.clone();
            state.pages.pop_from_current_stack(tabbar.as_ref())
        };

        if let Some(previous_page) = previous_page {
            // it's at top tab page
            if self.config.is_initial_route(&previous_page)
                || self.config.is_tab_page(&previous_page)
            {
                return false;
            }

            info!("Popped page, switching back to: {}", previous_page)
                .with_appid(self.appid.clone());

            // Request to switch to the previous page
            if let Err(e) = self
                .runtime
                .switch_page(self.appid.clone(), previous_page.clone())
            {
                error!("Failed to switch to page {}: {}", previous_page, e)
                    .with_appid(self.appid.clone());
            }

            // Return true to indicate we handled the back press
            true
        } else {
            // No page to pop, return false to allow default back behavior
            false
        }
    }

    fn handle_post_message(self: &Arc<Self>, path: String, msg: String) {
        let incoming = appservice::bridge::IncomingMessage::from_json_str(&msg).unwrap();

        if let Err(e) =
            self.executor
                .handle_view_message(self.appid.clone(), path, Arc::new(incoming))
        {
            error!("Failed to create app service: {}", e).with_appid(self.appid.clone());
        }
    }

    fn handle_request(
        self: &Arc<Self>,
        req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>> {
        let uri = req.uri();
        let scheme = uri.scheme_str().unwrap_or("");

        // Use pattern matching for different URI schemes
        match scheme {
            // HTTPS requests - check domain whitelist and static resource types
            "https" => self.https_handler(req),

            // Lingxia scheme for internal app assets
            "lx" => self.lingxia_handler(req),

            // Reject all other schemes with 400 Bad Request
            _ => Some(
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("Content-Type", "text/plain")
                    .body(format!("Unsupported scheme: {}", scheme).into_bytes())
                    .unwrap(),
            ),
        }
    }

    fn log(self: &Arc<Self>, path: &str, level: LogLevel, message: &str) {
        log::LogBuilder::new(LogTag::WebViewConsole, message)
            .with_level(level)
            .with_path(path)
            .with_appid(self.appid.clone());
    }
}
