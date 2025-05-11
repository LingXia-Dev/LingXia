use miniapp::{AppRuntime, MiniAppError};
use std::io::Read;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

mod controller;

#[cfg(target_os = "android")]
pub use android::{App, WebView};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::{App, WebView};

/// Asset file entry for iterator-based asset access
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

/// Platform host operations for mini apps
///
/// This trait defines the platform-specific capabilities that must be implemented by
/// the host platform (Android, iOS, etc) to support mini-app functionality. It extends
/// the core AppRuntime with UI and lifecycle operations.
trait PlatformHost: AppRuntime {
    /// Open a mini app in the platform-specific UI
    ///
    /// # Arguments
    /// * `appid` - Identifier of the mini application to open
    /// * `path` - Initial path to navigate to within the app
    ///
    /// # Returns
    /// * `Result<(), MiniAppError>` - Success or error response
    fn open_miniapp(&self, appid: &str, path: &str) -> Result<(), MiniAppError>;

    /// Switch to a different page within a mini app
    ///
    /// # Arguments
    /// * `appid` - Identifier of the mini application
    /// * `path` - Path to navigate to within the app
    ///
    /// # Returns
    /// * `Result<(), MiniAppError>` - Success or error response
    fn switch_page(&self, appid: &str, path: &str) -> Result<(), MiniAppError>;
}
