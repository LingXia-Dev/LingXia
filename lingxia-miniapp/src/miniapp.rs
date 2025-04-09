use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use crate::page::{self, PageController};

// Global instance of MiniApp
static MINI_APP: OnceLock<Mutex<MiniApp>> = OnceLock::new();

/// Initializes the MiniApp with the given AssetReader, cache directory, and data directory.
/// If MiniApp is already initialized, this function does nothing.
///
/// # Arguments
/// * `asset_reader` - The asset reader implementation
/// * `cache_dir` - Path to the cache directory of App
/// * `data_dir` - Path to the data directory of App
pub fn init(asset_reader: Box<dyn AssetReader + Send + Sync>, cache_dir: String, data_dir: String) {
    MINI_APP.get_or_init(|| {
        Mutex::new(MiniApp {
            asset_reader,
            cache_dir,
            data_dir,
            apps: HashMap::new(),
            last_active_times: HashMap::new(),
            max_apps: 5,
        })
    });
}

/// Returns a reference to the initialized MiniApp.
/// Panics if MiniApp has not been initialized.
pub fn get() -> &'static Mutex<MiniApp> {
    MINI_APP.get().expect("MiniApp has not been initialized")
}

pub struct MiniApp {
    asset_reader: Box<dyn AssetReader + Send + Sync>,
    cache_dir: String,
    data_dir: String,
    apps: HashMap<String, Arc<Mutex<page::PageManager>>>, // appid -> PageManager
    last_active_times: HashMap<String, Instant>,          // appid -> last active time
    max_apps: usize,                                      // Maximum number of apps allowed
}

impl MiniApp {
    /// Returns a reference to the PageManager for the given appid
    pub fn get_page_manager(&self, appid: &str) -> Option<&Arc<Mutex<page::PageManager>>> {
        self.apps.get(appid)
    }

    pub fn on_miniapp_loaded(&mut self, appid: String) {
        // If the app is already loaded, just update its active time
        if self.apps.contains_key(&appid) {
            self.last_active_times.insert(appid, Instant::now());
            return;
        }

        // If we've reached the maximum number of apps, destroy the least active one
        if self.apps.len() >= self.max_apps {
            self.destroy_least_active_miniapp();
        }

        // Create a new PageManager for this app
        self.apps.insert(
            appid.clone(),
            Arc::new(Mutex::new(page::PageManager::new(None))),
        );
        self.last_active_times.insert(appid, Instant::now());
    }

    pub fn on_miniapp_hidden(&mut self, appid: String) {
        // Only update the time if the app exists
        if self.apps.contains_key(&appid) {
            self.last_active_times.insert(appid, Instant::now());
        }
    }

    /// Handles low memory event (global, no appid needed)
    pub fn on_low_memory(&mut self) {
        // Destroy the least active app
        self.destroy_least_active_miniapp();
    }

    /// Called when a new page is created for the given appid and path
    pub fn on_page_created(&mut self, appid: String, path: String) {
        let page_manager = self
            .apps
            .entry(appid.clone())
            .or_insert_with(|| Arc::new(Mutex::new(page::PageManager::new(None))));

        // Initialize or update the page for the given path
        // update: on_page_show, on page show: page finsihed, reload(from java)
        let mut page_manager = page_manager.lock().unwrap();
        page_manager.mark_active(&path);
    }

    /// Finds a PageController by appid and path
    pub fn find_page_controller(&self, appid: &str, path: &str) -> Option<Arc<dyn PageController>> {
        if let Some(page_manager) = self.apps.get(appid) {
            let page_manager = page_manager.lock().unwrap();
            return page_manager.find_page_controller(path);
        }
        None
    }

    /// Determines whether to override URL loading in the page.
    ///
    /// # Arguments
    /// * `appid` - The identifier of the mini application
    /// * `path` - The current path of mini app
    /// * `url` - The URL being requested
    ///
    /// # Returns
    /// * `true` - To intercept and handle the URL loading
    /// * `false` - To allow the page to continue loading the URL
    pub fn should_override_url_loading(&self, _appid: String, _path: String, _url: String) -> bool {
        // Default implementation allows all URLs to load
        false
    }

    /// Handles a postMessage from the page's JavaScript context
    pub fn handle_post_message(&self, _appid: String, _path: String, _msg: String) {
        // ... implementation ...
    }

    /// Handles an HTTP request from the page
    pub fn handle_request(
        &self,
        _appid: String,
        _req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>> {
        // ... implementation ...
        None
    }

    /// Called when the page starts loading
    pub fn on_page_started(&self, _appid: String, _path: String) {
        // ... implementation ...
    }

    /// Called when the page finishes loading
    pub fn on_page_finished(&self, _appid: String, _path: String) {
        // ... implementation ...
    }

    /// Called when the page showed in the view
    pub fn on_page_show(&self, _appid: String, _path: String) {
        // ... implementation ...
    }
}

pub trait AssetReader: Send + Sync {
    fn read_asset(&self, path: &str) -> Vec<u8>;
}

impl MiniApp {
    /// Destroys the least active app
    fn destroy_least_active_miniapp(&mut self) {
        let least_active_appid = self
            .last_active_times
            .iter()
            .min_by_key(|(_, time)| *time)
            .map(|(appid, _)| appid.clone());

        if let Some(appid) = least_active_appid {
            // Remove from both maps - PageManager's Drop trait will handle cleanup
            self.apps.remove(&appid);
            self.last_active_times.remove(&appid);
        }
    }
}
