use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};

use lxapp::{AppRuntime, AssetFileEntry, DeviceInfo, LxAppError, WebViewController};

use crate::App;

/// Global runtime instance
static RUNTIME: OnceLock<Arc<PlatformAppRuntime>> = OnceLock::new();

/// Platform-specific AppRuntime implementation
/// This handles asset/resource access, device info, and platform capabilities
pub struct PlatformAppRuntime {
    app: App,
}

impl PlatformAppRuntime {
    /// Initialize the global runtime instance
    pub fn init(app: App) -> Arc<PlatformAppRuntime> {
        let runtime = Arc::new(PlatformAppRuntime { app });

        // Set global runtime, ignore error if already initialized
        let _ = RUNTIME.set(runtime.clone());
        runtime
    }
}

impl AppRuntime for PlatformAppRuntime {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, LxAppError> {
        // Use the original App's read_asset method directly
        self.app.read_asset(path)
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, LxAppError>> + 'a> {
        // Use the original App's asset_dir_iter method directly
        self.app.asset_dir_iter(asset_dir)
    }

    fn app_data_dir(&self) -> PathBuf {
        self.app.app_data_dir()
    }

    fn app_cache_dir(&self) -> PathBuf {
        self.app.app_cache_dir()
    }

    fn device_info(&self) -> DeviceInfo {
        self.app.device_info()
    }

    fn create_webview(
        &self,
        appid: String,
        path: String,
        sender: Sender<Result<Arc<dyn WebViewController>, LxAppError>>,
    ) {
        // Delegate to lingxia-webview's WebViewManager with channel sender
        lingxia_webview::create_webview(appid, path, sender)
    }

    fn open_lxapp(&self, appid: String, path: String) -> Result<(), LxAppError> {
        self.app.open_lxapp(&appid, &path)
    }

    fn close_lxapp(&self, appid: String) -> Result<(), LxAppError> {
        self.app.close_lxapp(&appid)
    }

    fn switch_page(&self, appid: String, path: String) -> Result<(), LxAppError> {
        self.app.switch_page(&appid, &path)
    }
}
