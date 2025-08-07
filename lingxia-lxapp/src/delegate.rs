use http::{Response, StatusCode};
use std::sync::Arc;
use std::time::Instant;

use crate::log::{self, LogLevel, LogTag};
use crate::page::PageState;
use crate::{LxApp, appservice, error, info};

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
        if let Err(e) = state.pages.get_or_create_page(
            self.appid.clone(),
            path.clone(),
            self.runtime.clone(),
            self.executor.clone(),
        ) {
            error!("Failed to create page for initial_route: {}", e)
                .with_appid(self.appid.clone())
                .with_path(path.clone());
        }
        state.opened = true;
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
    }

    fn on_page_show(self: &Arc<Self>, path: String) {
        info!("on_page_show called for path: {}", path)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        // Get the page (should already exist)
        let page = {
            let state = self.state.lock().unwrap();
            state.pages.get_page(&path).cloned()
        };

        let page = match page {
            Some(page) => page,
            None => {
                error!("Page not found: {}", path)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
                return;
            }
        };

        // Setup page if it hasn't been setup yet
        self.setup_page(&page, &path);

        // Navigate to the new page and get the previous page if there was a switch
        let previous_page = self
            .state
            .lock()
            .unwrap()
            .pages
            .navigate_to_page(path.clone());

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

        // precreate webviews for other tab pages
        let current_state = page.get_page_state();
        let should_precreate =
            self.config.is_initial_route(&path) && current_state == PageState::Loaded;

        if should_precreate {
            let tab_pages = self.config.get_tab_pages();
            for p in tab_pages {
                if p == path {
                    continue;
                }

                // Create new page and setup content
                let mut state = self.state.lock().unwrap();
                if let Ok(page) = state.pages.create_page(
                    self.appid.clone(),
                    p.clone(),
                    self.runtime.clone(),
                    self.executor.clone(),
                ) {
                    drop(state); // Release lock before setup

                    // On HarmonyOS, setup_page might fail if WebView isn't ready yet
                    // This is OK - the page will be setup when onPageShow is called for that tab
                    self.setup_page(&page, &p);
                }
            }
        }

        // Mark page as shown (at the end of on_page_show)
        page.set_page_state(PageState::Showed);
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
        if let Some(previous_page) = self.state.lock().unwrap().pages.pop_from_current_stack() {
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
